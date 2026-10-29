//! Zuletzt erzeugte (und bearbeitete) Ticket-Entwürfe je Dokument, damit sie sich später
//! wieder öffnen lassen; portiert aus `packages/server/src/docs/DocDraftStore.ts`.
//!
//! Schlüssel ist der volle Pfad der Datei; alles steht in einer Datei im Harness-Ordner
//! (je Benutzer lokal). Der Node-Server schreibt dieselbe Datei – Form und Schlüssel bleiben
//! deshalb unverändert.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use nexus_core::docs::TicketDraft;
use nexus_core::locking::AtomicWriter;
use nexus_core::{Error, HarnessHome, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use tokio::sync::Mutex;

/// Die gespeicherten Entwürfe eines Dokuments samt Kosten und Zeitpunkt der Erzeugung
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TicketDraftResult {
    pub drafts: Vec<TicketDraft>,
    pub dropped: Vec<String>,
    pub cost_usd: Option<f64>,
    pub at: String,
}

/// Name der Datei im Benutzerordner
const FILE: &str = "doc-drafts.json";

/// Hält die gespeicherten Ticket-Entwürfe aller Dokumente.
#[derive(Debug)]
pub struct DocDraftStore {
    home: HarnessHome,
    writer: AtomicWriter,
    /// Schreibvorgänge nacheinander, damit sich zwei Änderungen nicht überschreiben
    queue: Mutex<()>,
}

impl DocDraftStore {
    /// Store im Benutzerordner
    pub fn new(home: HarnessHome) -> Self {
        Self {
            home,
            writer: AtomicWriter::new(),
            queue: Mutex::new(()),
        }
    }

    /// Die Datei im Benutzerordner
    fn file(&self) -> PathBuf {
        self.home.dir().join(FILE)
    }

    /// Gespeicherte Entwürfe der Datei; `None`, wenn es keine gibt
    pub fn get(&self, full_path: &Path) -> Option<TicketDraftResult> {
        let all = self.read_all();
        let found = all.get(&key(full_path))?;
        serde_json::from_value(found.clone()).ok()
    }

    /// Die Dateien unter `full_paths`, zu denen Entwürfe gespeichert sind
    pub fn saved_among(&self, full_paths: &[PathBuf]) -> HashSet<PathBuf> {
        let all = self.read_all();
        full_paths
            .iter()
            .filter(|path| {
                all.get(&key(path))
                    .and_then(|found| found.get("drafts"))
                    .and_then(Value::as_array)
                    .is_some_and(|drafts| !drafts.is_empty())
            })
            .cloned()
            .collect()
    }

    /// Merkt sich die Entwürfe der Datei (ersetzt frühere)
    pub async fn put(&self, full_path: &Path, result: &TicketDraftResult) -> Result<()> {
        let value = serde_json::to_value(result).map_err(Error::unexpected)?;
        self.change(full_path, Some(value)).await
    }

    /// Vergisst die Entwürfe der Datei (verworfen oder als Tickets angelegt)
    pub async fn remove(&self, full_path: &Path) -> Result<()> {
        self.change(full_path, None).await
    }

    /// Liest, ändert und schreibt die Datei; Aufrufe laufen nacheinander
    async fn change(&self, full_path: &Path, value: Option<Value>) -> Result<()> {
        let _held = self.queue.lock().await;
        let mut all = self.read_all();
        match value {
            Some(value) => {
                all.insert(key(full_path), value);
            }
            None => {
                all.shift_remove(&key(full_path));
            }
        }
        fs::create_dir_all(self.home.dir()).map_err(Error::unexpected)?;
        let text = serde_json::to_string_pretty(&Value::Object(all)).map_err(Error::unexpected)?;
        self.writer.write(&self.file(), &text)
    }

    /// Alle Einträge der Datei; unlesbare oder fremde Inhalte gelten als leer. Die Einträge
    /// bleiben rohe JSON-Werte, damit ein vom Node-Server geschriebener Eintrag unverändert
    /// erhalten bleibt.
    fn read_all(&self) -> Map<String, Value> {
        let Ok(raw) = fs::read_to_string(self.file()) else {
            return Map::new();
        };
        match serde_json::from_str(&raw) {
            Ok(Value::Object(all)) => all,
            _ => Map::new(),
        }
    }
}

/// Unter Windows zählen Groß/Klein und die Richtung der Schrägstriche im Pfad nicht
fn key(full_path: &Path) -> String {
    let path = full_path.to_string_lossy().replace('\\', "/");
    if cfg!(windows) {
        path.to_lowercase()
    } else {
        path
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Entwurfsergebnis mit einem Entwurf
    fn result(title: &str) -> TicketDraftResult {
        TicketDraftResult {
            drafts: vec![TicketDraft {
                title: title.into(),
                ..TicketDraft::default()
            }],
            dropped: Vec::new(),
            cost_usd: None,
            at: "2026-10-22T10:00:00.000Z".into(),
        }
    }

    /// Benutzerordner für Tests
    fn home() -> (tempfile::TempDir, DocDraftStore) {
        let dir = tempfile::Builder::new()
            .prefix("nexus-drafts-")
            .tempdir()
            .unwrap();
        let home = HarnessHome::at(dir.path());
        (dir, DocDraftStore::new(home))
    }

    #[tokio::test]
    async fn merkt_liest_und_vergisst_entwurfe_je_datei() {
        let (_dir, store) = home();
        let spec = PathBuf::from("/repo/docs/superpowers/specs/a-design.md");
        let plan = PathBuf::from("/repo/docs/superpowers/plans/a.md");
        assert_eq!(store.get(&spec), None);

        store.put(&spec, &result("A")).await.unwrap();

        assert_eq!(store.get(&spec), Some(result("A")));
        assert_eq!(store.get(&plan), None);
        assert_eq!(
            store.saved_among(&[spec.clone(), plan.clone()]),
            HashSet::from([spec.clone()])
        );

        store.remove(&spec).await.unwrap();

        assert_eq!(store.get(&spec), None);
        assert!(store.saved_among(&[spec]).is_empty());
    }

    #[tokio::test]
    async fn lasst_fremde_eintrage_unverandert_stehen() {
        let (dir, store) = home();
        let other = serde_json::json!({
            "/repo/fremd.md": { "drafts": [], "dropped": [], "costUsd": 0.5, "at": "x", "neu": 1 },
        });
        fs::write(
            dir.path().join("doc-drafts.json"),
            serde_json::to_string(&other).unwrap(),
        )
        .unwrap();

        store
            .put(Path::new("/repo/eigen.md"), &result("A"))
            .await
            .unwrap();

        let back: Value =
            serde_json::from_str(&fs::read_to_string(dir.path().join("doc-drafts.json")).unwrap())
                .unwrap();
        assert_eq!(back["/repo/fremd.md"]["neu"], 1);
        assert_eq!(back["/repo/eigen.md"]["drafts"][0]["title"], "A");
    }

    #[tokio::test]
    async fn behandelt_eine_kaputte_datei_als_leer() {
        let (dir, store) = home();
        fs::write(dir.path().join("doc-drafts.json"), "{kaputt").unwrap();

        assert_eq!(store.get(Path::new("/repo/a.md")), None);
        store
            .put(Path::new("/repo/a.md"), &result("A"))
            .await
            .unwrap();
        assert_eq!(store.get(Path::new("/repo/a.md")), Some(result("A")));
    }
}
