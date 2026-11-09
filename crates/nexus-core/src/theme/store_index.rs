//! Prüft die Index-Datei `nexus-themes.json` eines Store-Repos: ein JSON-Array von Einträgen
//! `{ id, name, version, sdkVersion, downloadUrl, sha256, preview?, description? }`. Ungültige
//! Einträge werden ausgelassen und mit Meldung genannt, Einträge mit unpassender `sdkVersion`
//! bleiben gelistet und werden nur markiert.

use std::collections::HashSet;

use serde::Serialize;
use serde_json::{Map, Value};
use url::Url;

use crate::error::HarnessError;
use crate::themes::is_valid_theme_id;

/// Aktuelle Version des Theme-SDK; muss zu `SDK_VERSION` in packages/theme-sdk passen
pub const THEME_SDK_VERSION: &str = "1.0.0";

/// [`THEME_SDK_VERSION`] als semver-Version
pub fn theme_sdk_version() -> semver::Version {
    semver::Version::parse(THEME_SDK_VERSION).expect("SDK-Version ist semver")
}

/// Höchstgröße einer Index-Datei: 1 MiB
pub const MAX_INDEX_BYTES: usize = 1 << 20;

/// Höchstzahl an Einträgen einer Index-Datei
pub const MAX_INDEX_ENTRIES: usize = 500;

/// Höchstlänge einer Beschreibung in Zeichen
pub const MAX_DESCRIPTION_CHARS: usize = 2000;

/// Höchstlänge einer Signatur (Inhalt der `.minisig`-Datei) in Bytes
pub const MAX_SIGNATURE_BYTES: usize = 4096;

/// Fehlercode: Zeitlimit überschritten
pub const STORE_INDEX_TIMEOUT: &str = "store-index-timeout";
/// Fehlercode: keine Verbindung (Verbindung, DNS)
pub const STORE_INDEX_UNREACHABLE: &str = "store-index-unreachable";
/// Fehlercode: HTTP-Status ≠ 2xx
pub const STORE_INDEX_HTTP: &str = "store-index-http";
/// Fehlercode: Index größer als [`MAX_INDEX_BYTES`]
pub const STORE_INDEX_TOO_LARGE: &str = "store-index-too-large";
/// Fehlercode: kein JSON, kein Array oder zu viele Einträge
pub const STORE_INDEX_INVALID: &str = "store-index-invalid";

/// Ein gültiger Eintrag eines Index
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StoreIndexEntry {
    pub id: String,
    pub name: String,
    pub version: String,
    pub sdk_version: String,
    pub download_url: String,
    pub sha256: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preview: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Inhalt der `.minisig`-Datei des Pakets (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
    /// Ob `sdkVersion` zur aktuellen SDK-Version passt
    pub compatible: bool,
}

/// Ein ausgelassener Eintrag
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkippedEntry {
    /// Stelle im Array
    pub index: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub message: String,
}

/// Ergebnis der Prüfung
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ParsedIndex {
    pub entries: Vec<StoreIndexEntry>,
    pub skipped: Vec<SkippedEntry>,
}

/// Prüft eine Index-Datei; kein JSON, kein Array oder zu viele Einträge →
/// `store-index-invalid`, zu groß → `store-index-too-large`
pub fn parse_index(bytes: &[u8], sdk: &semver::Version) -> Result<ParsedIndex, HarnessError> {
    if bytes.len() > MAX_INDEX_BYTES {
        return Err(HarnessError::new(
            STORE_INDEX_TOO_LARGE,
            "Die Index-Datei ist größer als 1 MiB.",
        ));
    }
    let value: Value = serde_json::from_slice(bytes)
        .map_err(|_| invalid("Die Index-Datei ist kein gültiges JSON."))?;
    let Value::Array(items) = value else {
        return Err(invalid("Die Index-Datei muss eine JSON-Liste sein."));
    };
    if items.len() > MAX_INDEX_ENTRIES {
        return Err(invalid("Die Index-Datei hat mehr als 500 Einträge."));
    }
    let mut parsed = ParsedIndex::default();
    let mut seen = HashSet::new();
    for (index, item) in items.iter().enumerate() {
        let id = item.get("id").and_then(Value::as_str).map(str::to_string);
        let skip = |message: String| SkippedEntry {
            index,
            id: id.clone(),
            message,
        };
        match check_entry(item, sdk) {
            Ok(entry) => {
                if seen.insert(entry.id.clone()) {
                    parsed.entries.push(entry);
                } else {
                    parsed
                        .skipped
                        .push(skip("Die ID kommt in diesem Repo doppelt vor.".into()));
                }
            }
            Err(message) => parsed.skipped.push(skip(message)),
        }
    }
    Ok(parsed)
}

fn check_entry(item: &Value, sdk: &semver::Version) -> Result<StoreIndexEntry, String> {
    let Value::Object(fields) = item else {
        return Err("Der Eintrag ist kein Objekt.".into());
    };
    let id = required(fields, "id")?;
    if !is_valid_theme_id(id) {
        return Err("Die ID muss 2–40 Zeichen aus a–z, 0–9 und - haben.".into());
    }
    let name = required(fields, "name")?;
    if name.trim().is_empty() {
        return Err("Der Name darf nicht leer sein.".into());
    }
    let version = required(fields, "version")?;
    if semver::Version::parse(version).is_err() {
        return Err("version ist keine gültige semver-Version (z. B. 1.2.3).".into());
    }
    let sdk_version = required(fields, "sdkVersion")?;
    let range = semver::VersionReq::parse(sdk_version)
        .ok()
        .filter(|_| !sdk_version.trim().is_empty())
        .ok_or("sdkVersion ist kein gültiger semver-Bereich (z. B. ^1.0).")?;
    let download_url = required(fields, "downloadUrl")?;
    check_url(download_url).map_err(|problem| format!("downloadUrl {problem}"))?;
    let sha256 = required(fields, "sha256")?;
    if sha256.len() != 64 || !sha256.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err("sha256 muss aus 64 Hex-Zeichen bestehen.".into());
    }
    let preview = optional(fields, "preview")?;
    if let Some(preview) = preview {
        check_url(preview).map_err(|problem| format!("preview {problem}"))?;
    }
    let description = optional(fields, "description")?;
    if description.is_some_and(|text| text.chars().count() > MAX_DESCRIPTION_CHARS) {
        return Err("description ist länger als 2000 Zeichen.".into());
    }
    let signature = optional(fields, "signature")?;
    if signature.is_some_and(|text| text.len() > MAX_SIGNATURE_BYTES) {
        return Err("signature ist länger als 4 KiB.".into());
    }
    Ok(StoreIndexEntry {
        id: id.into(),
        name: name.into(),
        version: version.into(),
        sdk_version: sdk_version.into(),
        download_url: download_url.into(),
        sha256: sha256.to_ascii_lowercase(),
        preview: preview.map(str::to_string),
        description: description.map(str::to_string),
        signature: signature.map(str::to_string),
        compatible: range.matches(sdk),
    })
}

fn required<'a>(fields: &'a Map<String, Value>, key: &str) -> Result<&'a str, String> {
    match fields.get(key) {
        None | Some(Value::Null) => Err(format!("Das Feld {key} fehlt.")),
        Some(Value::String(text)) => Ok(text),
        Some(_) => Err(format!("Das Feld {key} muss ein Text sein.")),
    }
}

fn optional<'a>(fields: &'a Map<String, Value>, key: &str) -> Result<Option<&'a str>, String> {
    match fields.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(text)) => Ok(Some(text)),
        Some(_) => Err(format!("Das Feld {key} muss ein Text sein.")),
    }
}

/// `https`, `http` nur für `localhost`/`127.0.0.1`, ohne Zugangsdaten
pub fn check_url(raw: &str) -> Result<(), &'static str> {
    let url = Url::parse(raw).map_err(|_| "ist keine gültige Adresse.")?;
    let host = url.host_str().unwrap_or_default();
    if host.is_empty() {
        return Err("hat keinen Host.");
    }
    let local = host == "localhost" || host == "127.0.0.1";
    match url.scheme() {
        "https" => {}
        "http" if local => {}
        _ => return Err("muss mit https:// beginnen."),
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err("darf keine Zugangsdaten enthalten.");
    }
    Ok(())
}

fn invalid(message: &str) -> HarnessError {
    HarnessError::new(STORE_INDEX_INVALID, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sdk() -> semver::Version {
        theme_sdk_version()
    }

    fn eintrag(id: &str) -> Value {
        json!({
            "id": id,
            "name": "Ozean",
            "version": "1.2.0",
            "sdkVersion": "^1.0",
            "downloadUrl": "https://example.com/ozean.nexus",
            "sha256": "AB".repeat(32),
            "preview": "https://example.com/ozean.png",
            "description": "Blau"
        })
    }

    fn parse(value: Value) -> ParsedIndex {
        parse_index(value.to_string().as_bytes(), &sdk()).unwrap()
    }

    #[test]
    fn gueltiger_index_ist_kompatibel() {
        let parsed = parse(json!([eintrag("ozean"), eintrag("wald")]));
        assert!(parsed.skipped.is_empty());
        assert_eq!(parsed.entries.len(), 2);
        assert!(parsed.entries.iter().all(|entry| entry.compatible));
        assert_eq!(parsed.entries[0].sha256, "ab".repeat(32));
    }

    #[test]
    fn unpassende_sdk_version_wird_markiert() {
        let mut entry = eintrag("ozean");
        entry["sdkVersion"] = json!("^2.0");
        let parsed = parse(json!([entry]));
        assert_eq!(parsed.entries.len(), 1);
        assert!(!parsed.entries[0].compatible);
    }

    #[test]
    fn ungueltige_eintraege_werden_ausgelassen() {
        let mut ohne_name = eintrag("a1");
        ohne_name.as_object_mut().unwrap().remove("name");
        let mut falsche_id = eintrag("x");
        falsche_id["id"] = json!("Gross");
        let mut kaputter_hash = eintrag("a3");
        kaputter_hash["sha256"] = json!("123");
        let mut fremdes_http = eintrag("a4");
        fremdes_http["downloadUrl"] = json!("http://example.com/a.nexus");
        let mut kein_semver = eintrag("a5");
        kein_semver["version"] = json!("eins");
        let mut lokales_http = eintrag("a6");
        lokales_http["downloadUrl"] = json!("http://127.0.0.1:9/a.nexus");
        let parsed = parse(json!([
            ohne_name,
            falsche_id,
            kaputter_hash,
            fremdes_http,
            kein_semver,
            lokales_http,
            "kein objekt"
        ]));
        assert_eq!(parsed.entries.len(), 1);
        assert_eq!(parsed.entries[0].id, "a6");
        let indizes: Vec<usize> = parsed.skipped.iter().map(|s| s.index).collect();
        assert_eq!(indizes, vec![0, 1, 2, 3, 4, 6]);
        assert!(parsed.skipped[0].message.contains("name"));
        assert_eq!(parsed.skipped[1].id.as_deref(), Some("Gross"));
        assert!(parsed.skipped[3].message.contains("https://"));
    }

    #[test]
    fn signatur_ist_optional_und_begrenzt() {
        let mut signiert = eintrag("ozean");
        signiert["signature"] = json!("untrusted comment: x\nRUQ=\n");
        let mut zu_lang = eintrag("wald");
        zu_lang["signature"] = json!("a".repeat(MAX_SIGNATURE_BYTES + 1));
        let parsed = parse(json!([signiert, zu_lang, eintrag("berg")]));
        assert_eq!(parsed.entries.len(), 2);
        assert!(parsed.entries[0].signature.is_some());
        assert_eq!(parsed.entries[1].signature, None);
        assert_eq!(parsed.skipped.len(), 1);
        assert!(parsed.skipped[0].message.contains("signature"));
    }

    #[test]
    fn doppelte_id_im_repo() {
        let parsed = parse(json!([eintrag("ozean"), eintrag("ozean")]));
        assert_eq!(parsed.entries.len(), 1);
        assert_eq!(parsed.skipped.len(), 1);
        assert_eq!(parsed.skipped[0].index, 1);
        assert!(parsed.skipped[0].message.contains("doppelt"));
    }

    #[test]
    fn kein_json_oder_kein_array() {
        for bytes in [&b"{kaputt"[..], br#"{"id":"x"}"#] {
            let error = parse_index(bytes, &sdk()).unwrap_err();
            assert_eq!(error.code, STORE_INDEX_INVALID);
        }
    }

    #[test]
    fn zu_viele_eintraege_oder_zu_gross() {
        let viele = Value::Array(vec![json!(1); MAX_INDEX_ENTRIES + 1]);
        let error = parse_index(viele.to_string().as_bytes(), &sdk()).unwrap_err();
        assert_eq!(error.code, STORE_INDEX_INVALID);
        let gross = vec![b' '; MAX_INDEX_BYTES + 1];
        let error = parse_index(&gross, &sdk()).unwrap_err();
        assert_eq!(error.code, STORE_INDEX_TOO_LARGE);
    }
}
