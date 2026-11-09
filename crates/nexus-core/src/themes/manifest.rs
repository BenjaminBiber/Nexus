//! Das Manifest (`manifest.json`) eines Theme-Pakets und seine Prüfung. Fehler werden je Feld
//! gesammelt (JSON-Feldname, bei Listen mit Index), damit die Oberfläche sie neben dem Feld
//! zeigen kann.

use std::collections::HashSet;
use std::fmt;
use std::sync::LazyLock;

use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use ts_rs::TS;

/// Erlaubte Zeichen einer Theme-ID
pub(crate) static THEME_ID_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[a-z0-9-]{2,40}$").unwrap());

/// Erlaubte Form eines Slot-Namens
static SLOT_NAME_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[a-z0-9][a-z0-9.-]*$").unwrap());

/// Erlaubte Form einer Domain; `*.` vorweg gibt alle Subdomains frei
static DOMAIN_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^(\*\.)?[a-z0-9-]+(\.[a-z0-9-]+)+$").unwrap());

/// Farbschema, für das ein Theme gebaut ist
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export, export_to = "themes.ts")]
pub enum ColorScheme {
    Light,
    Dark,
}

/// Rechte, die ein Theme anfordert
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "themes.ts")]
pub struct ThemePermissions {
    /// Domains, von denen das Theme Ressourcen über den Server-Proxy laden darf (ohne
    /// Schema, Port und Pfad; `*.example.com` gibt Subdomains frei)
    #[serde(default)]
    pub network: Vec<String>,
}

/// Inhalt der `manifest.json` eines Theme-Pakets
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "themes.ts")]
pub struct ThemeManifest {
    /// Kennung des Themes (`^[a-z0-9-]{2,40}$`); Ordnername unter `~/.nexus/themes/`
    pub id: String,
    pub name: String,
    /// Version des Themes (semver)
    pub version: String,
    pub author: String,
    /// Bereich der SDK-Versionen, mit denen das Theme läuft (semver-Bereich, z. B. `^1.0`)
    pub sdk_version: String,
    pub color_scheme: ColorScheme,
    pub start_color: Option<String>,
    /// Pfad der Vorschau im Paket
    pub preview: Option<String>,
    /// Namen der Slots, die das Theme ersetzt
    #[serde(default)]
    pub slots: Vec<String>,
    #[serde(default)]
    pub permissions: ThemePermissions,
}

/// Ein Fehler des Manifests an einem Feld
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "themes.ts")]
pub struct ManifestError {
    /// JSON-Feldname, bei Listen mit Index (`permissions.network[0]`); leer, wenn das
    /// Manifest als Ganzes unlesbar ist
    pub field: String,
    pub message: String,
}

/// Alle Fehler eines Manifests, gemeinsam gemeldet
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{}", join_messages(.0))]
pub struct ManifestErrors(pub Vec<ManifestError>);

fn join_messages(errors: &[ManifestError]) -> String {
    errors
        .iter()
        .map(|error| {
            if error.field.is_empty() {
                error.message.clone()
            } else {
                format!("{}: {}", error.field, error.message)
            }
        })
        .collect::<Vec<_>>()
        .join("; ")
}

impl ManifestErrors {
    /// Die Fehler, in der Reihenfolge der Felder im Manifest
    pub fn errors(&self) -> &[ManifestError] {
        &self.0
    }

    /// Erster Fehler zu einem Feld
    pub fn for_field(&self, field: &str) -> Option<&ManifestError> {
        self.0.iter().find(|error| error.field == field)
    }
}

impl fmt::Display for ManifestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.field, self.message)
    }
}

fn error(field: impl Into<String>, message: impl Into<String>) -> ManifestError {
    ManifestError {
        field: field.into(),
        message: message.into(),
    }
}

/// Die zu prüfenden Felder, als Verweise: gleich für ein fertiges Manifest und für rohes JSON
struct Fields<'a> {
    id: Option<&'a str>,
    name: Option<&'a str>,
    version: Option<&'a str>,
    author: Option<&'a str>,
    sdk_version: Option<&'a str>,
    /// Slot-Namen mit ihrem Index in der Liste
    slots: Vec<(usize, &'a str)>,
    /// Domains mit ihrem Index in der Liste
    network: Vec<(usize, &'a str)>,
}

fn check_fields(fields: &Fields<'_>, errors: &mut Vec<ManifestError>) {
    if fields.id.is_some_and(|id| !THEME_ID_PATTERN.is_match(id)) {
        errors.push(error(
            "id",
            "Die ID muss 2–40 Zeichen aus a–z, 0–9 und - haben.",
        ));
    }
    if fields.name.is_some_and(|name| name.trim().is_empty()) {
        errors.push(error("name", "Der Name darf nicht leer sein."));
    }
    if fields
        .version
        .is_some_and(|version| semver::Version::parse(version).is_err())
    {
        errors.push(error(
            "version",
            "Die Version ist keine gültige semver-Version (z. B. 1.2.3).",
        ));
    }
    if fields.author.is_some_and(|author| author.trim().is_empty()) {
        errors.push(error("author", "Der Autor darf nicht leer sein."));
    }
    if fields
        .sdk_version
        .is_some_and(|range| range.trim().is_empty() || semver::VersionReq::parse(range).is_err())
    {
        errors.push(error(
            "sdkVersion",
            "sdkVersion ist kein gültiger semver-Bereich (z. B. ^1.0).",
        ));
    }
    check_slots(&fields.slots, errors);
    for &(index, domain) in &fields.network {
        if let Some(message) = domain_problem(domain) {
            errors.push(error(format!("permissions.network[{index}]"), message));
        }
    }
}

fn check_slots(slots: &[(usize, &str)], errors: &mut Vec<ManifestError>) {
    let mut seen = HashSet::new();
    for &(index, name) in slots {
        let field = format!("slots[{index}]");
        if name.trim().is_empty() {
            errors.push(error(field, "Der Slot-Name darf nicht leer sein."));
        } else if !SLOT_NAME_PATTERN.is_match(name) {
            errors.push(error(
                field,
                format!(
                    "Der Slot-Name „{name}“ darf nur a–z, 0–9, . und - enthalten und muss mit a–z oder 0–9 beginnen."
                ),
            ));
        } else if !seen.insert(name) {
            errors.push(error(
                field,
                format!("Der Slot „{name}“ kommt mehrfach vor."),
            ));
        }
    }
}

/// Warum die Domain nicht erlaubt ist; `None`, wenn sie passt
fn domain_problem(domain: &str) -> Option<String> {
    if domain.is_empty() {
        return Some("Die Domain darf nicht leer sein.".to_string());
    }
    if domain.contains(|c: char| c.is_whitespace() || matches!(c, ':' | '/' | '?' | '#')) {
        return Some(format!(
            "Domain „{domain}“ darf kein Schema, keinen Port und keinen Pfad enthalten."
        ));
    }
    if !DOMAIN_PATTERN.is_match(domain) {
        return Some(format!(
            "Domain „{domain}“ ist kein gültiger Domainname (z. B. cdn.example.com oder *.example.com)."
        ));
    }
    None
}

/// Prüft ein fertiges Manifest; sammelt alle Fehler. Das Farbschema ist durch den Typ
/// gesichert; ein unbekanntes meldet [`parse_manifest`].
pub fn validate_manifest(manifest: &ThemeManifest) -> Result<(), ManifestErrors> {
    let fields = Fields {
        id: Some(&manifest.id),
        name: Some(&manifest.name),
        version: Some(&manifest.version),
        author: Some(&manifest.author),
        sdk_version: Some(&manifest.sdk_version),
        slots: manifest
            .slots
            .iter()
            .enumerate()
            .map(|(index, slot)| (index, slot.as_str()))
            .collect(),
        network: manifest
            .permissions
            .network
            .iter()
            .enumerate()
            .map(|(index, domain)| (index, domain.as_str()))
            .collect(),
    };
    let mut errors = Vec::new();
    check_fields(&fields, &mut errors);
    finish(errors)
}

fn finish(errors: Vec<ManifestError>) -> Result<(), ManifestErrors> {
    if errors.is_empty() {
        Ok(())
    } else {
        Err(ManifestErrors(errors))
    }
}

/// Liest und prüft den Text einer `manifest.json`. Alle Fehler kommen gemeinsam zurück, je
/// Feld mit JSON-Feldnamen – auch fehlende Felder und Felder mit falschem Typ.
pub fn parse_manifest(json: &str) -> Result<ThemeManifest, ManifestErrors> {
    let value: Value = serde_json::from_str(json).map_err(|e| {
        ManifestErrors(vec![error(
            "",
            format!("Das Manifest ist kein gültiges JSON: {e}"),
        )])
    })?;
    let Value::Object(object) = &value else {
        return Err(ManifestErrors(vec![error(
            "",
            "Das Manifest muss ein JSON-Objekt sein.",
        )]));
    };

    let mut errors = Vec::new();
    let id = required_text(object, "id", &mut errors);
    let name = required_text(object, "name", &mut errors);
    let version = required_text(object, "version", &mut errors);
    let author = required_text(object, "author", &mut errors);
    let sdk_version = required_text(object, "sdkVersion", &mut errors);
    let color_scheme = match required_text(object, "colorScheme", &mut errors) {
        Some("light") => Some(ColorScheme::Light),
        Some("dark") => Some(ColorScheme::Dark),
        Some(_) => {
            errors.push(error(
                "colorScheme",
                "colorScheme muss light oder dark sein.",
            ));
            None
        }
        None => None,
    };
    let start_color = optional_text(object, "startColor", &mut errors);
    let preview = optional_text(object, "preview", &mut errors);
    let slots = text_list(object.get("slots"), "slots", &mut errors);

    let mut network = Vec::new();
    match object.get("permissions") {
        None | Some(Value::Null) => {}
        Some(Value::Object(permissions)) => {
            network = text_list(
                permissions.get("network"),
                "permissions.network",
                &mut errors,
            );
        }
        Some(_) => errors.push(error("permissions", "permissions muss ein Objekt sein.")),
    }

    check_fields(
        &Fields {
            id,
            name,
            version,
            author,
            sdk_version,
            slots: slots.iter().map(|(i, s)| (*i, s.as_str())).collect(),
            network: network.iter().map(|(i, s)| (*i, s.as_str())).collect(),
        },
        &mut errors,
    );
    // Die Felder stehen nach Typ-/Muster-Fehlern in einer festen Reihenfolge
    errors.sort_by_key(|e| field_rank(&e.field));
    if !errors.is_empty() {
        return Err(ManifestErrors(errors));
    }

    match (id, name, version, author, sdk_version, color_scheme) {
        (Some(id), Some(name), Some(version), Some(author), Some(sdk_version), Some(scheme)) => {
            Ok(ThemeManifest {
                id: id.to_string(),
                name: name.to_string(),
                version: version.to_string(),
                author: author.to_string(),
                sdk_version: sdk_version.to_string(),
                color_scheme: scheme,
                start_color: start_color.map(str::to_string),
                preview: preview.map(str::to_string),
                slots: slots.into_iter().map(|(_, s)| s).collect(),
                permissions: ThemePermissions {
                    network: network.into_iter().map(|(_, s)| s).collect(),
                },
            })
        }
        // Fehlt ein Pflichtfeld, steht dazu schon ein Fehler in `errors`
        _ => Err(ManifestErrors(errors)),
    }
}

/// Stellung eines Feldes in der Fehlerliste: wie im Manifest, stabil innerhalb eines Feldes
fn field_rank(field: &str) -> usize {
    const ORDER: [&str; 10] = [
        "id",
        "name",
        "version",
        "author",
        "sdkVersion",
        "colorScheme",
        "startColor",
        "preview",
        "slots",
        "permissions",
    ];
    let base = field.split(['[', '.']).next().unwrap_or(field);
    ORDER.iter().position(|f| *f == base).unwrap_or(0)
}

fn required_text<'a>(
    object: &'a Map<String, Value>,
    key: &str,
    errors: &mut Vec<ManifestError>,
) -> Option<&'a str> {
    match object.get(key) {
        None | Some(Value::Null) => {
            errors.push(error(key, format!("Das Feld {key} fehlt.")));
            None
        }
        Some(Value::String(text)) => Some(text),
        Some(_) => {
            errors.push(error(key, format!("Das Feld {key} muss ein Text sein.")));
            None
        }
    }
}

fn optional_text<'a>(
    object: &'a Map<String, Value>,
    key: &str,
    errors: &mut Vec<ManifestError>,
) -> Option<&'a str> {
    match object.get(key) {
        None | Some(Value::Null) => None,
        Some(Value::String(text)) => Some(text),
        Some(_) => {
            errors.push(error(key, format!("Das Feld {key} muss ein Text sein.")));
            None
        }
    }
}

/// Liste von Texten mit ihrem Index; Einträge anderen Typs sind Fehler und fallen weg
fn text_list(
    value: Option<&Value>,
    field: &str,
    errors: &mut Vec<ManifestError>,
) -> Vec<(usize, String)> {
    match value {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::Array(items)) => items
            .iter()
            .enumerate()
            .filter_map(|(index, item)| match item {
                Value::String(text) => Some((index, text.clone())),
                _ => {
                    errors.push(error(
                        format!("{field}[{index}]"),
                        "Der Eintrag muss ein Text sein.",
                    ));
                    None
                }
            })
            .collect(),
        Some(_) => {
            errors.push(error(
                field,
                format!("{field} muss eine Liste von Texten sein."),
            ));
            Vec::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn voll() -> serde_json::Value {
        json!({
            "id": "nacht-blau",
            "name": "Nachtblau",
            "version": "1.2.3",
            "author": "Ada",
            "sdkVersion": "^1.0",
            "colorScheme": "dark",
            "startColor": "#101820",
            "preview": "assets/preview.png",
            "slots": ["sidebar", "board.card"],
            "permissions": { "network": ["cdn.example.com", "*.example.org"] }
        })
    }

    fn minimal() -> serde_json::Value {
        json!({
            "id": "ab",
            "name": "Schlicht",
            "version": "0.1.0",
            "author": "Ada",
            "sdkVersion": "^1",
            "colorScheme": "light"
        })
    }

    fn mit(base: serde_json::Value, feld: &str, wert: serde_json::Value) -> String {
        let mut value = base;
        value[feld] = wert;
        value.to_string()
    }

    fn felder(json: &str) -> Vec<String> {
        parse_manifest(json)
            .unwrap_err()
            .errors()
            .iter()
            .map(|e| e.field.clone())
            .collect()
    }

    #[test]
    fn gueltiges_manifest_voll() {
        let manifest = parse_manifest(&voll().to_string()).unwrap();
        assert_eq!(manifest.id, "nacht-blau");
        assert_eq!(manifest.color_scheme, ColorScheme::Dark);
        assert_eq!(manifest.slots, vec!["sidebar", "board.card"]);
        assert_eq!(manifest.permissions.network.len(), 2);
        assert_eq!(validate_manifest(&manifest), Ok(()));
    }

    #[test]
    fn gueltiges_manifest_minimal() {
        let manifest = parse_manifest(&minimal().to_string()).unwrap();
        assert_eq!(manifest.start_color, None);
        assert_eq!(manifest.preview, None);
        assert!(manifest.slots.is_empty());
        assert!(manifest.permissions.network.is_empty());
        assert_eq!(validate_manifest(&manifest), Ok(()));
    }

    #[test]
    fn roundtrip_in_camel_case() {
        let manifest = parse_manifest(&voll().to_string()).unwrap();
        let out = serde_json::to_value(&manifest).unwrap();
        assert_eq!(out["sdkVersion"], "^1.0");
        assert_eq!(out["colorScheme"], "dark");
        assert_eq!(out["startColor"], "#101820");
        let zurueck: ThemeManifest = serde_json::from_value(out).unwrap();
        assert_eq!(zurueck, manifest);
    }

    #[test]
    fn serde_liest_minimal_mit_vorgaben() {
        let manifest: ThemeManifest = serde_json::from_value(minimal()).unwrap();
        assert!(manifest.slots.is_empty());
        assert_eq!(manifest.permissions, ThemePermissions::default());
    }

    #[test]
    fn falsche_id() {
        for id in ["Gross", "a", &"a".repeat(41), "mit_strich", ""] {
            let json = mit(voll(), "id", json!(id));
            let fehler = parse_manifest(&json).unwrap_err();
            let eintrag = fehler.for_field("id").unwrap_or_else(|| panic!("{id:?}"));
            assert!(eintrag.message.contains("2–40 Zeichen"));
        }
        let json = mit(voll(), "id", json!("a".repeat(40)));
        assert!(parse_manifest(&json).is_ok());
    }

    #[test]
    fn version_kein_semver() {
        for version in ["1.0", "abc", "v1.0.0", ""] {
            let json = mit(voll(), "version", json!(version));
            assert_eq!(felder(&json), vec!["version"], "{version:?}");
        }
        let fehler = parse_manifest(&mit(voll(), "version", json!("1.0"))).unwrap_err();
        assert!(fehler.to_string().contains("semver"));
    }

    #[test]
    fn sdk_version_kein_semver_bereich() {
        for bereich in ["^x", "abc", ""] {
            let json = mit(voll(), "sdkVersion", json!(bereich));
            assert_eq!(felder(&json), vec!["sdkVersion"], "{bereich:?}");
        }
        for bereich in ["^1.0", ">=1.2, <2", "1", "*"] {
            assert!(parse_manifest(&mit(voll(), "sdkVersion", json!(bereich))).is_ok());
        }
    }

    #[test]
    fn unbekanntes_farbschema() {
        let fehler = parse_manifest(&mit(voll(), "colorScheme", json!("blue"))).unwrap_err();
        assert_eq!(fehler.errors().len(), 1);
        assert_eq!(
            fehler.for_field("colorScheme").unwrap().message,
            "colorScheme muss light oder dark sein."
        );
    }

    #[test]
    fn domains_mit_schema_port_oder_pfad() {
        for domain in [
            "https://a.de",
            "a.de/pfad",
            "a.de:8080",
            "a.de?x=1",
            "a b.de",
        ] {
            let json = mit(
                voll(),
                "permissions",
                json!({ "network": ["cdn.example.com", domain] }),
            );
            let fehler = parse_manifest(&json).unwrap_err();
            assert_eq!(fehler.errors().len(), 1, "{domain}");
            let eintrag = &fehler.errors()[0];
            assert_eq!(eintrag.field, "permissions.network[1]");
            assert!(
                eintrag
                    .message
                    .contains("kein Schema, keinen Port und keinen Pfad"),
                "{domain}"
            );
        }
    }

    #[test]
    fn domains_ohne_punkt_oder_mit_fremden_zeichen() {
        for domain in ["localhost", "", "a_b.de", "*.com.", "*a.de"] {
            let json = mit(voll(), "permissions", json!({ "network": [domain] }));
            assert_eq!(felder(&json), vec!["permissions.network[0]"], "{domain:?}");
        }
    }

    #[test]
    fn gueltige_domains() {
        for domain in [
            "cdn.example.com",
            "*.example.com",
            "CDN.Example.COM",
            "a-b.de",
        ] {
            let json = mit(voll(), "permissions", json!({ "network": [domain] }));
            assert!(parse_manifest(&json).is_ok(), "{domain}");
        }
    }

    #[test]
    fn leerer_und_doppelter_slot() {
        let json = mit(voll(), "slots", json!(["sidebar", "", "sidebar", "  "]));
        let fehler = parse_manifest(&json).unwrap_err();
        let felder: Vec<_> = fehler.errors().iter().map(|e| e.field.as_str()).collect();
        assert_eq!(felder, vec!["slots[1]", "slots[2]", "slots[3]"]);
        assert!(fehler.errors()[0].message.contains("nicht leer"));
        assert!(fehler.errors()[1].message.contains("mehrfach"));
    }

    #[test]
    fn slot_mit_ungueltigem_muster() {
        let json = mit(voll(), "slots", json!(["Sidebar", ".punkt", "ok"]));
        assert_eq!(felder(&json), vec!["slots[0]", "slots[1]"]);
    }

    #[test]
    fn name_und_autor_nicht_leer() {
        let mut manifest = voll();
        manifest["name"] = json!("  ");
        let json = mit(manifest, "author", json!(""));
        assert_eq!(felder(&json), vec!["name", "author"]);
    }

    #[test]
    fn mehrere_fehler_gemeinsam() {
        let json = json!({
            "id": "BAD",
            "name": "x",
            "version": "eins",
            "author": "y",
            "sdkVersion": "^x",
            "colorScheme": "blue",
            "slots": ["a", "a"],
            "permissions": { "network": ["https://a.de"] }
        })
        .to_string();
        let fehler = parse_manifest(&json).unwrap_err();
        assert_eq!(
            felder(&json),
            vec![
                "id",
                "version",
                "sdkVersion",
                "colorScheme",
                "slots[1]",
                "permissions.network[0]"
            ]
        );
        let text = fehler.to_string();
        assert!(text.contains("id: Die ID muss"));
        assert!(text.contains("; version: "));
        assert!(text.matches("; ").count() == 5);
    }

    #[test]
    fn validate_manifest_prueft_fertigen_typ() {
        let mut manifest = parse_manifest(&voll().to_string()).unwrap();
        manifest.id = "X".to_string();
        manifest.permissions.network = vec!["a.de/x".to_string()];
        let fehler = validate_manifest(&manifest).unwrap_err();
        let felder: Vec<_> = fehler.errors().iter().map(|e| e.field.as_str()).collect();
        assert_eq!(felder, vec!["id", "permissions.network[0]"]);
    }

    #[test]
    fn fehlende_und_falsch_getypte_felder() {
        let fehler = parse_manifest(r#"{"id": 5, "slots": "x", "permissions": []}"#).unwrap_err();
        assert_eq!(
            fehler.for_field("id").unwrap().message,
            "Das Feld id muss ein Text sein."
        );
        assert_eq!(
            fehler.for_field("name").unwrap().message,
            "Das Feld name fehlt."
        );
        assert!(fehler.for_field("slots").is_some());
        assert!(fehler.for_field("permissions").is_some());
    }

    #[test]
    fn kein_json_oder_kein_objekt() {
        let fehler = parse_manifest("{kaputt").unwrap_err();
        assert_eq!(fehler.errors()[0].field, "");
        assert!(fehler.errors()[0]
            .message
            .starts_with("Das Manifest ist kein gültiges JSON"));
        let fehler = parse_manifest("[]").unwrap_err();
        assert_eq!(
            fehler.errors()[0].message,
            "Das Manifest muss ein JSON-Objekt sein."
        );
    }
}
