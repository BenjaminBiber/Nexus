//! Speichert die vom Nutzer eingetragenen Store-Repos unter `~/.nexus/store/repos.json`: je
//! Repo die Adresse einer Index-Datei `nexus-themes.json`. Vorbelegte Repos gibt es nicht.

use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use url::Url;

use crate::clock::Clock;
use crate::error::{Error, HarnessError, Result};
use crate::locking::AtomicWriter;
use crate::registry::HarnessHome;

/// Höchstzahl eingetragener Repos
pub const MAX_STORE_REPOS: usize = 50;

/// Höchstlänge einer Adresse in Zeichen
pub const MAX_STORE_REPO_URL: usize = 2048;

/// Fehlercode für ungültige Eingaben (HTTP 400)
pub const STORE_REPO_INVALID: &str = "store-repo-invalid";

/// Fehlercode für eine schon eingetragene Adresse (HTTP 409)
pub const STORE_REPO_EXISTS: &str = "store-repo-exists";

/// Ein eingetragenes Repo
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoreRepo {
    pub id: String,
    pub url: String,
    pub added_at: String,
}

/// Liest und schreibt die Store-Repos
#[derive(Debug, Clone)]
pub struct StoreRepoStore {
    home: HarnessHome,
    clock: Clock,
    writer: AtomicWriter,
    /// Hält Lesen-Ändern-Schreiben von `repos.json` zusammen
    lock: Arc<Mutex<()>>,
}

impl StoreRepoStore {
    /// Repos im angegebenen Benutzerordner
    pub fn new(home: HarnessHome) -> Self {
        Self::with_clock(home, Clock::system())
    }

    /// Wie [`StoreRepoStore::new`], mit eigener Uhr
    pub fn with_clock(home: HarnessHome, clock: Clock) -> Self {
        Self {
            home,
            clock,
            writer: AtomicWriter::new(),
            lock: Arc::new(Mutex::new(())),
        }
    }

    /// Alle Repos, sortiert nach `addedAt`, dann `id`
    pub fn list(&self) -> Vec<StoreRepo> {
        self.read()
    }

    /// Trägt die Adresse ein; ungültig → `store-repo-invalid`, schon da → `store-repo-exists`
    pub fn add(&self, url: &str) -> Result<StoreRepo> {
        let url = normalize_url(url)?;
        let _guard = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        let mut repos = self.read();
        if repos.iter().any(|repo| repo.url == url) {
            return Err(
                HarnessError::new(STORE_REPO_EXISTS, "Dieses Repo ist schon eingetragen.").into(),
            );
        }
        if repos.len() >= MAX_STORE_REPOS {
            return Err(invalid("Höchstens 50 Repos.").into());
        }
        let repo = StoreRepo {
            id: repo_id(&url),
            url,
            added_at: self.clock.iso(),
        };
        repos.push(repo.clone());
        self.write(&repos)?;
        Ok(repo)
    }

    /// Entfernt das Repo; unbekannt → `store-repo-not-found`
    pub fn remove(&self, id: &str) -> Result<()> {
        let _guard = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        let mut repos = self.read();
        let before = repos.len();
        repos.retain(|repo| repo.id != id);
        if repos.len() == before {
            return Err(
                HarnessError::new("store-repo-not-found", "Dieses Repo gibt es nicht.").into(),
            );
        }
        self.write(&repos)
    }

    fn file(&self) -> PathBuf {
        self.home.store_dir().join("repos.json")
    }

    /// Einträge sortiert; fehlende oder beschädigte Datei ergibt keine Repos
    fn read(&self) -> Vec<StoreRepo> {
        let mut repos: Vec<StoreRepo> = fs::read_to_string(self.file())
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default();
        repos.sort_by(|a, b| a.added_at.cmp(&b.added_at).then_with(|| a.id.cmp(&b.id)));
        repos
    }

    fn write(&self, repos: &[StoreRepo]) -> Result<()> {
        fs::create_dir_all(self.home.store_dir()).map_err(Error::unexpected)?;
        let json = serde_json::to_string_pretty(repos).map_err(Error::unexpected)?;
        self.writer.write(&self.file(), &json)
    }
}

/// Prüft die Adresse und bringt sie in die gespeicherte Form (Rand ohne Leerzeichen, Schema
/// und Host klein). Erlaubt ist `https`, `http` nur für `localhost`/`127.0.0.1`
pub fn normalize_url(raw: &str) -> Result<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(invalid("Bitte eine Adresse angeben.").into());
    }
    if trimmed.chars().count() > MAX_STORE_REPO_URL {
        return Err(invalid("Die Adresse ist länger als 2048 Zeichen.").into());
    }
    if trimmed.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err(invalid("Die Adresse darf keine Leer- oder Steuerzeichen enthalten.").into());
    }
    let url = Url::parse(trimmed).map_err(|_| invalid("Das ist keine gültige Adresse."))?;
    let host = url.host_str().unwrap_or_default();
    if host.is_empty() {
        return Err(invalid("Der Adresse fehlt der Host.").into());
    }
    let local = host == "localhost" || host == "127.0.0.1";
    match url.scheme() {
        "https" => {}
        "http" if local => {}
        _ => return Err(invalid("Die Adresse muss mit https:// beginnen.").into()),
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(invalid("Die Adresse darf keine Zugangsdaten enthalten.").into());
    }
    Ok(url.to_string())
}

/// Erste 12 Hex-Zeichen von sha256 der normalisierten Adresse
fn repo_id(url: &str) -> String {
    Sha256::digest(url.as_bytes())
        .iter()
        .take(6)
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn invalid(message: impl Into<String>) -> HarnessError {
    HarnessError::new(STORE_REPO_INVALID, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> (StoreRepoStore, tempfile::TempDir) {
        let dir = tempfile::tempdir().expect("Temp-Ordner");
        (StoreRepoStore::new(HarnessHome::at(dir.path())), dir)
    }

    const INDEX: &str = "https://raw.githubusercontent.com/a/b/main/nexus-themes.json";

    #[test]
    fn traegt_ein_und_listet() {
        let (store, dir) = store();
        assert!(store.list().is_empty());
        let repo = store.add(INDEX).unwrap();
        assert_eq!(repo.url, INDEX);
        assert_eq!(repo.id.len(), 12);
        assert!(repo.id.chars().all(|c| c.is_ascii_hexdigit()));
        assert_eq!(store.list(), vec![repo]);
        assert!(dir.path().join("store").join("repos.json").exists());
    }

    #[test]
    fn lehnt_ungueltige_adressen_ab_und_speichert_nichts() {
        let (store, dir) = store();
        let long = format!("https://x.de/{}", "a".repeat(MAX_STORE_REPO_URL));
        for url in [
            "",
            "ftp://x.de/i.json",
            "nicht-url",
            "https://",
            "http://example.com/i.json",
            "https://u:p@host.de/x",
            "https://u@host.de/x",
            "https://x.de/a b",
            long.as_str(),
        ] {
            let error = store.add(url).unwrap_err();
            assert_eq!(error.code(), Some(STORE_REPO_INVALID), "{url}");
        }
        assert!(!dir.path().join("store").exists());
    }

    #[test]
    fn erlaubt_http_nur_lokal() {
        let (store, _dir) = store();
        store.add("http://localhost:8080/i.json").unwrap();
        store.add("http://127.0.0.1/i.json").unwrap();
    }

    #[test]
    fn erkennt_dubletten_nach_normalisierung() {
        let (store, _dir) = store();
        store.add("https://Example.COM/i.json").unwrap();
        let error = store.add("  HTTPS://example.com/i.json ").unwrap_err();
        assert_eq!(error.code(), Some(STORE_REPO_EXISTS));
        assert_eq!(store.list().len(), 1);
        assert_eq!(store.list()[0].url, "https://example.com/i.json");
    }

    #[test]
    fn begrenzt_die_anzahl() {
        let (store, _dir) = store();
        for i in 0..MAX_STORE_REPOS {
            store.add(&format!("https://x.de/{i}.json")).unwrap();
        }
        let error = store.add("https://x.de/zu-viel.json").unwrap_err();
        assert_eq!(error.code(), Some(STORE_REPO_INVALID));
    }

    #[test]
    fn entfernt_und_meldet_unbekannte() {
        let (store, _dir) = store();
        let repo = store.add(INDEX).unwrap();
        store.remove(&repo.id).unwrap();
        assert!(store.list().is_empty());
        assert_eq!(
            store.remove(&repo.id).unwrap_err().code(),
            Some("store-repo-not-found")
        );
    }

    #[test]
    fn neue_instanz_liest_die_gespeicherte_liste() {
        let (store, dir) = store();
        let repo = store.add(INDEX).unwrap();
        let again = StoreRepoStore::new(HarnessHome::at(dir.path()));
        assert_eq!(again.list(), vec![repo]);
    }
}
