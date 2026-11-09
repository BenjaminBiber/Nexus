//! Speichert die CSS-Schnipsel des Nutzers unter `~/.nexus/snippets/`: je Schnipsel eine
//! Datei `<id>.css`, dazu `snippets.json` mit Name, Schalter, Reihenfolge und Änderungszeit.

use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

use crate::clock::Clock;
use crate::error::{Error, HarnessError, Result};
use crate::locking::AtomicWriter;
use crate::registry::HarnessHome;
use crate::theme::css_check::check_snippet_css;

/// Höchstgröße eines Schnipsels in Bytes (64 KiB)
pub const MAX_SNIPPET_BYTES: usize = 64 * 1024;

/// Fehlercode für ungültige Eingaben (HTTP 400)
pub const SNIPPET_INVALID: &str = "snippet-invalid";

/// Eintrag in `snippets.json`
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SnippetMeta {
    id: String,
    name: String,
    enabled: bool,
    order: i64,
    updated_at: String,
}

/// Ein Schnipsel mit Inhalt, wie die Oberfläche ihn bekommt
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snippet {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub order: i64,
    pub updated_at: String,
    pub css: String,
}

/// Liest und schreibt die Schnipsel
#[derive(Debug, Clone)]
pub struct SnippetStore {
    home: HarnessHome,
    clock: Clock,
    writer: AtomicWriter,
    /// Hält Lesen-Ändern-Schreiben von `snippets.json` zusammen
    lock: Arc<Mutex<()>>,
}

impl SnippetStore {
    /// Schnipsel im angegebenen Benutzerordner
    pub fn new(home: HarnessHome) -> Self {
        Self::with_clock(home, Clock::system())
    }

    /// Wie [`SnippetStore::new`], mit eigener Uhr
    pub fn with_clock(home: HarnessHome, clock: Clock) -> Self {
        Self {
            home,
            clock,
            writer: AtomicWriter::new(),
            lock: Arc::new(Mutex::new(())),
        }
    }

    /// Alle Schnipsel in Auslieferungsreihenfolge (`order`, dann `id`)
    pub fn list(&self) -> Vec<Snippet> {
        self.read_meta()
            .into_iter()
            .map(|meta| {
                let css = fs::read_to_string(self.css_file(&meta.id)).unwrap_or_default();
                Snippet {
                    id: meta.id,
                    name: meta.name,
                    enabled: meta.enabled,
                    order: meta.order,
                    updated_at: meta.updated_at,
                    css,
                }
            })
            .collect()
    }

    /// Legt an oder ändert; `true`, wenn der Schnipsel neu ist. Ohne `enabled` bleibt der
    /// Schalter, neue Schnipsel sind an
    pub fn put(
        &self,
        id: &str,
        name: &str,
        css: &str,
        enabled: Option<bool>,
    ) -> Result<(bool, Snippet)> {
        validate_id(id)?;
        if css.len() > MAX_SNIPPET_BYTES {
            return Err(invalid("Der Schnipsel ist größer als 64 KiB.").into());
        }
        check_snippet_css(css).map_err(|violation| invalid(violation.to_string()))?;
        let _guard = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        let mut metas = self.read_meta();
        let now = self.clock.iso();
        let created = !metas.iter().any(|meta| meta.id == id);
        if created {
            let order = metas.iter().map(|meta| meta.order).max().unwrap_or(0) + 1;
            metas.push(SnippetMeta {
                id: id.to_string(),
                name: name.to_string(),
                enabled: enabled.unwrap_or(true),
                order,
                updated_at: now,
            });
        } else if let Some(meta) = metas.iter_mut().find(|meta| meta.id == id) {
            meta.name = name.to_string();
            meta.enabled = enabled.unwrap_or(meta.enabled);
            meta.updated_at = now;
        }
        fs::create_dir_all(self.dir()).map_err(Error::unexpected)?;
        self.writer.write(&self.css_file(id), css)?;
        self.write_meta(&metas)?;
        let snippet = self.find(id)?;
        Ok((created, snippet))
    }

    /// Schaltet an oder aus
    pub fn set_enabled(&self, id: &str, enabled: bool) -> Result<Snippet> {
        validate_id(id)?;
        let _guard = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        let mut metas = self.read_meta();
        let meta = metas
            .iter_mut()
            .find(|meta| meta.id == id)
            .ok_or_else(not_found)?;
        meta.enabled = enabled;
        meta.updated_at = self.clock.iso();
        self.write_meta(&metas)?;
        self.find(id)
    }

    /// Löscht den Schnipsel samt Datei
    pub fn delete(&self, id: &str) -> Result<()> {
        validate_id(id)?;
        let _guard = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        let mut metas = self.read_meta();
        let before = metas.len();
        metas.retain(|meta| meta.id != id);
        if metas.len() == before {
            return Err(not_found().into());
        }
        self.write_meta(&metas)?;
        let file = self.css_file(id);
        if file.exists() {
            fs::remove_file(file).map_err(Error::unexpected)?;
        }
        Ok(())
    }

    /// Die aktiven Schnipsel als ein Stylesheet, je mit `/* snippet: <id> */` davor
    pub fn active_css(&self) -> String {
        self.list()
            .into_iter()
            .filter(|snippet| snippet.enabled)
            .map(|snippet| format!("/* snippet: {} */\n{}\n", snippet.id, snippet.css))
            .collect()
    }

    fn find(&self, id: &str) -> Result<Snippet> {
        self.list()
            .into_iter()
            .find(|snippet| snippet.id == id)
            .ok_or_else(|| not_found().into())
    }

    fn dir(&self) -> PathBuf {
        self.home.snippets_dir()
    }

    fn css_file(&self, id: &str) -> PathBuf {
        self.dir().join(format!("{id}.css"))
    }

    fn meta_file(&self) -> PathBuf {
        self.dir().join("snippets.json")
    }

    /// Einträge sortiert; fehlende oder beschädigte Datei ergibt keine Schnipsel
    fn read_meta(&self) -> Vec<SnippetMeta> {
        let mut metas: Vec<SnippetMeta> = fs::read_to_string(self.meta_file())
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default();
        metas.retain(|meta| is_valid_id(&meta.id));
        metas.sort_by(|a, b| a.order.cmp(&b.order).then_with(|| a.id.cmp(&b.id)));
        metas
    }

    fn write_meta(&self, metas: &[SnippetMeta]) -> Result<()> {
        fs::create_dir_all(self.dir()).map_err(Error::unexpected)?;
        let json = serde_json::to_string_pretty(metas).map_err(Error::unexpected)?;
        self.writer.write(&self.meta_file(), &json)
    }
}

/// `^[a-z0-9-]{2,40}$`, wie die ID im Manifest
pub fn is_valid_id(id: &str) -> bool {
    (2..=40).contains(&id.len())
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

fn validate_id(id: &str) -> Result<()> {
    match is_valid_id(id) {
        true => Ok(()),
        false => Err(invalid("Die ID muss aus 2 bis 40 Zeichen a-z, 0-9 oder - bestehen.").into()),
    }
}

fn invalid(message: impl Into<String>) -> HarnessError {
    HarnessError::new(SNIPPET_INVALID, message)
}

fn not_found() -> HarnessError {
    HarnessError::new("snippet-not-found", "Diesen Schnipsel gibt es nicht.")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> (SnippetStore, tempfile::TempDir) {
        let dir = tempfile::tempdir().expect("Temp-Ordner");
        (SnippetStore::new(HarnessHome::at(dir.path())), dir)
    }

    #[test]
    fn legt_an_und_aendert() {
        let (store, dir) = store();
        let (created, snippet) = store.put("rot", "Rot", "a { color: red }", None).unwrap();
        assert!(created);
        assert!(snippet.enabled);
        assert_eq!(snippet.order, 1);
        assert!(dir.path().join("snippets").join("rot.css").exists());

        let (created, snippet) = store
            .put("rot", "Rot 2", "a { color: darkred }", Some(false))
            .unwrap();
        assert!(!created);
        assert_eq!(snippet.name, "Rot 2");
        assert!(!snippet.enabled);
        assert_eq!(snippet.css, "a { color: darkred }");
        assert_eq!(store.list().len(), 1);
    }

    #[test]
    fn lehnt_ungueltige_eingaben_ab_und_speichert_nichts() {
        let (store, dir) = store();
        for (id, css) in [
            ("A", "a{}"),
            ("x", "a{}"),
            ("ok-id", "@import 'x';"),
            ("ok-id", "a { background: url(https://x.de) }"),
        ] {
            let error = store.put(id, "n", css, None).unwrap_err();
            assert_eq!(error.code(), Some(SNIPPET_INVALID), "{id} {css}");
        }
        let big = "a".repeat(MAX_SNIPPET_BYTES + 1);
        assert_eq!(
            store.put("gross", "n", &big, None).unwrap_err().code(),
            Some(SNIPPET_INVALID)
        );
        assert!(!dir.path().join("snippets").exists());
    }

    #[test]
    fn schaltet_und_loescht() {
        let (store, _dir) = store();
        store.put("rot", "Rot", "a{}", None).unwrap();
        assert!(!store.set_enabled("rot", false).unwrap().enabled);
        store.delete("rot").unwrap();
        assert!(store.list().is_empty());
        assert_eq!(
            store.delete("rot").unwrap_err().code(),
            Some("snippet-not-found")
        );
        assert_eq!(
            store.set_enabled("rot", true).unwrap_err().code(),
            Some("snippet-not-found")
        );
    }

    #[test]
    fn liefert_nur_aktive_in_fester_reihenfolge() {
        let (store, _dir) = store();
        store.put("zwei", "Zwei", "b{}", None).unwrap();
        store.put("eins", "Eins", "a{}", None).unwrap();
        store.put("aus", "Aus", "c{}", Some(false)).unwrap();
        assert_eq!(
            store.active_css(),
            "/* snippet: zwei */\nb{}\n/* snippet: eins */\na{}\n"
        );
    }
}
