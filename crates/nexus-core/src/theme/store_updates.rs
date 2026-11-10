//! Updates aus dem Store: vergleicht installierte Themes per semver mit den Einträgen der
//! Store-Indizes und prüft, ob ein neues Paket eine Bestätigung braucht (Code oder neue
//! Domains, D2). Alles läuft im Speicher; installiert wird über `themes::install_theme`.

use std::io::Cursor;

use semver::Version;
use serde::Serialize;

use super::store_index::StoreIndexEntry;
use super::store_install::read_manifest;
use crate::error::HarnessError;
use crate::themes::{parse_manifest, ThemeManifest};

/// Fehlercode: neues Paket ist kein lesbares Theme-Paket
pub const STORE_UPDATE_PACKAGE_INVALID: &str = "store-update-package-invalid";

/// Ein verfügbares Update
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemeUpdate {
    pub id: String,
    pub name: String,
    /// Repo, aus dem das Update kommt
    pub repo_id: String,
    pub installed_version: String,
    pub available_version: String,
}

/// Was ein Update mitbringt, das der Nutzer bestätigen muss
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateRisk {
    /// Das neue Paket enthält `main.js`
    pub has_code: bool,
    /// Domains aus `permissions.network`, die die installierte Version nicht hatte
    pub new_domains: Vec<String>,
}

impl UpdateRisk {
    /// Code oder neue Domains brauchen eine ausdrückliche Bestätigung
    pub fn needs_confirmation(&self) -> bool {
        self.has_code || !self.new_domains.is_empty()
    }
}

/// Ob `available` per semver streng neuer ist als `installed`; ungültige Versionen → `false`
pub fn is_newer(installed: &str, available: &str) -> bool {
    match (Version::parse(installed), Version::parse(available)) {
        (Ok(installed), Ok(available)) => available > installed,
        _ => false,
    }
}

/// Verfügbare Updates, nach id sortiert. Nur kompatible Einträge mit streng neuerer Version;
/// je Theme gewinnt die höchste Version, bei Gleichstand das zuerst genannte Repo
pub fn find_updates(
    installed: &[ThemeManifest],
    entries: &[(String, StoreIndexEntry)],
) -> Vec<ThemeUpdate> {
    let mut updates = Vec::new();
    for manifest in installed {
        let Ok(current) = Version::parse(&manifest.version) else {
            continue;
        };
        let mut best: Option<(Version, &String, &StoreIndexEntry)> = None;
        for (repo_id, entry) in entries {
            if entry.id != manifest.id || !entry.compatible {
                continue;
            }
            let Ok(version) = Version::parse(&entry.version) else {
                continue;
            };
            if version <= current {
                continue;
            }
            if best.as_ref().is_none_or(|(top, _, _)| version > *top) {
                best = Some((version, repo_id, entry));
            }
        }
        if let Some((_, repo_id, entry)) = best {
            updates.push(ThemeUpdate {
                id: manifest.id.clone(),
                name: manifest.name.clone(),
                repo_id: repo_id.clone(),
                installed_version: manifest.version.clone(),
                available_version: entry.version.clone(),
            });
        }
    }
    updates.sort_by(|a, b| a.id.cmp(&b.id));
    updates
}

/// Prüft das neue Paket gegen die installierte Version: enthält es `main.js`, und welche
/// Domains fordert es neu an (Vergleich klein geschrieben)
pub fn assess_update(old: &ThemeManifest, package: &[u8]) -> Result<UpdateRisk, HarnessError> {
    let invalid = || {
        HarnessError::new(
            STORE_UPDATE_PACKAGE_INVALID,
            "Das neue Paket ist kein lesbares Theme-Paket.",
        )
    };
    let mut archive = zip::ZipArchive::new(Cursor::new(package)).map_err(|_| invalid())?;
    let has_code = archive.by_name("main.js").is_ok();
    let text = read_manifest(package).ok_or_else(invalid)?;
    let manifest = parse_manifest(&text).map_err(|_| invalid())?;
    let known: Vec<String> = old
        .permissions
        .network
        .iter()
        .map(|domain| domain.to_lowercase())
        .collect();
    let mut new_domains: Vec<String> = Vec::new();
    for domain in &manifest.permissions.network {
        let domain = domain.to_lowercase();
        if !known.contains(&domain) && !new_domains.contains(&domain) {
            new_domains.push(domain);
        }
    }
    Ok(UpdateRisk {
        has_code,
        new_domains,
    })
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    fn manifest(id: &str, version: &str, network: &[&str]) -> ThemeManifest {
        let json = serde_json::json!({
            "id": id, "name": "Ozean", "version": version, "author": "Test",
            "sdkVersion": "^1.0", "colorScheme": "dark",
            "permissions": { "network": network }
        });
        parse_manifest(&json.to_string()).unwrap()
    }

    fn eintrag(id: &str, version: &str, compatible: bool) -> StoreIndexEntry {
        StoreIndexEntry {
            id: id.into(),
            name: "Ozean".into(),
            version: version.into(),
            sdk_version: "^1.0".into(),
            download_url: "https://example.com/p.nexus".into(),
            sha256: "0".repeat(64),
            preview: None,
            description: None,
            signature: None,
            compatible,
        }
    }

    fn paket(network: &[&str], mit_code: bool) -> Vec<u8> {
        let m = manifest("ozean", "1.1.0", network);
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default();
        zip.start_file("manifest.json", options).unwrap();
        zip.write_all(serde_json::to_string(&m).unwrap().as_bytes())
            .unwrap();
        zip.start_file("theme.css", options).unwrap();
        zip.write_all(b":root{}").unwrap();
        if mit_code {
            zip.start_file("main.js", options).unwrap();
            zip.write_all(b"export default {}").unwrap();
        }
        zip.finish().unwrap().into_inner()
    }

    fn updates(installiert: &str, eintraege: &[(&str, StoreIndexEntry)]) -> Vec<ThemeUpdate> {
        let entries: Vec<(String, StoreIndexEntry)> = eintraege
            .iter()
            .map(|(repo, entry)| (repo.to_string(), entry.clone()))
            .collect();
        find_updates(&[manifest("ozean", installiert, &[])], &entries)
    }

    #[test]
    fn store_updates_neuere_version_ist_update() {
        let found = updates("1.2.0", &[("r1", eintrag("ozean", "1.3.0", true))]);
        assert_eq!(
            found,
            vec![ThemeUpdate {
                id: "ozean".into(),
                name: "Ozean".into(),
                repo_id: "r1".into(),
                installed_version: "1.2.0".into(),
                available_version: "1.3.0".into(),
            }]
        );
    }

    #[test]
    fn store_updates_gleiche_und_aeltere_version_nicht() {
        assert!(updates("1.2.0", &[("r1", eintrag("ozean", "1.2.0", true))]).is_empty());
        assert!(updates("1.2.0", &[("r1", eintrag("ozean", "1.1.9", true))]).is_empty());
    }

    #[test]
    fn store_updates_vorabversion_ist_kleiner() {
        assert!(is_newer("2.0.0-beta", "2.0.0"));
        assert!(!is_newer("2.0.0", "2.0.0-beta"));
        assert!(updates("2.0.0", &[("r1", eintrag("ozean", "2.0.0-beta", true))]).is_empty());
    }

    #[test]
    fn store_updates_ungueltiges_semver_uebersprungen() {
        let mut kaputt = manifest("ozean", "1.0.0", &[]);
        kaputt.version = "eins".into();
        let entries = vec![("r1".to_string(), eintrag("ozean", "2.0.0", true))];
        assert!(find_updates(&[kaputt], &entries).is_empty());
        assert!(updates("1.0.0", &[("r1", eintrag("ozean", "zwei", true))]).is_empty());
        assert!(!is_newer("eins", "2.0.0"));
    }

    #[test]
    fn store_updates_nicht_installiert_und_inkompatibel_nicht() {
        assert!(updates("1.0.0", &[("r1", eintrag("wald", "2.0.0", true))]).is_empty());
        assert!(updates("1.0.0", &[("r1", eintrag("ozean", "2.0.0", false))]).is_empty());
    }

    #[test]
    fn store_updates_hoechste_version_gewinnt() {
        let found = updates(
            "1.0.0",
            &[
                ("r1", eintrag("ozean", "1.1.0", true)),
                ("r2", eintrag("ozean", "1.4.0", true)),
                ("r3", eintrag("ozean", "1.4.0", true)),
            ],
        );
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].repo_id, "r2");
        assert_eq!(found[0].available_version, "1.4.0");
    }

    #[test]
    fn store_updates_nur_css_braucht_keine_bestaetigung() {
        let alt = manifest("ozean", "1.0.0", &["fonts.example.com"]);
        let risk = assess_update(&alt, &paket(&["Fonts.Example.com"], false)).unwrap();
        assert_eq!(risk, UpdateRisk::default());
        assert!(!risk.needs_confirmation());
    }

    #[test]
    fn store_updates_code_braucht_bestaetigung() {
        let alt = manifest("ozean", "1.0.0", &[]);
        let risk = assess_update(&alt, &paket(&[], true)).unwrap();
        assert!(risk.has_code);
        assert!(risk.needs_confirmation());
    }

    #[test]
    fn store_updates_neue_domain_braucht_bestaetigung() {
        let alt = manifest("ozean", "1.0.0", &["a.example.com"]);
        let risk = assess_update(&alt, &paket(&["a.example.com", "b.example.com"], false)).unwrap();
        assert_eq!(risk.new_domains, vec!["b.example.com".to_string()]);
        assert!(risk.needs_confirmation());
    }

    #[test]
    fn store_updates_entfernte_domain_braucht_keine_bestaetigung() {
        let alt = manifest("ozean", "1.0.0", &["a.example.com", "b.example.com"]);
        let risk = assess_update(&alt, &paket(&["a.example.com"], false)).unwrap();
        assert!(!risk.needs_confirmation());
    }

    #[test]
    fn store_updates_kaputtes_paket() {
        let alt = manifest("ozean", "1.0.0", &[]);
        let error = assess_update(&alt, b"kein zip").unwrap_err();
        assert_eq!(error.code, STORE_UPDATE_PACKAGE_INVALID);
    }
}
