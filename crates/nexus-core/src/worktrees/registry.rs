//! Verzeichnis der Worktrees im Git-Verzeichnis des Projekts (`harness/worktrees.json`).
//! Portiert aus `packages/core/src/worktrees/WorktreeRegistry.ts`.

use std::fmt;
use std::fs;

use serde::de::{MapAccess, Visitor};
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::error::{Error, Result};
use crate::locking::{AtomicWriter, FileLock, FileLockOptions};
use crate::project::HarnessLayout;

/// Stand der Einrichtung des Dispatchers (`worktreeSetup`)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SetupState {
    /// Noch nicht gelungen
    Pending,
    /// Gelaufen
    Done,
}

/// Ein Eintrag des Verzeichnisses: Ordner, Tickets und Stand des Worktrees
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeEntry {
    pub path: String,
    pub tickets: Vec<String>,
    pub ready_to_merge: bool,
    /// `true`, wenn der Ordner aufgeräumt wurde (der Branch bleibt erhalten)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub removed: Option<bool>,
    /// Einrichtung des Dispatchers (`worktreeSetup`): pending = noch nicht gelungen,
    /// done = gelaufen; fehlt = keine vorgesehen
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub setup: Option<SetupState>,
}

impl WorktreeEntry {
    /// Neuer Eintrag mit seinem ersten Ticket
    pub fn new(path: impl Into<String>, ticket_id: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            tickets: vec![ticket_id.into()],
            ready_to_merge: false,
            removed: None,
            setup: None,
        }
    }

    /// Ist der Ordner aufgeräumt?
    pub fn is_removed(&self) -> bool {
        self.removed.unwrap_or(false)
    }
}

/// Branch → Eintrag, in der Reihenfolge der Datei. JavaScript behält die Reihenfolge der
/// Schlüssel eines Objekts; `start` tritt dem ersten passenden Worktree bei, deshalb bleibt
/// sie hier erhalten statt sortiert zu werden.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WorktreeMap {
    entries: Vec<(String, WorktreeEntry)>,
}

impl WorktreeMap {
    /// Leeres Verzeichnis
    pub fn new() -> Self {
        Self::default()
    }

    /// Eintrag eines Branches
    pub fn get(&self, branch: &str) -> Option<&WorktreeEntry> {
        self.entries
            .iter()
            .find(|(name, _)| name == branch)
            .map(|(_, entry)| entry)
    }

    /// Eintrag eines Branches zum Ändern
    pub fn get_mut(&mut self, branch: &str) -> Option<&mut WorktreeEntry> {
        self.entries
            .iter_mut()
            .find(|(name, _)| name == branch)
            .map(|(_, entry)| entry)
    }

    /// Legt den Eintrag an oder ersetzt ihn (ein vorhandener behält seinen Platz)
    pub fn insert(&mut self, branch: impl Into<String>, entry: WorktreeEntry) {
        let branch = branch.into();
        match self.get_mut(&branch) {
            Some(existing) => *existing = entry,
            None => self.entries.push((branch, entry)),
        }
    }

    /// Alle Einträge in der Reihenfolge der Datei
    pub fn iter(&self) -> impl Iterator<Item = (&String, &WorktreeEntry)> {
        self.entries.iter().map(|(branch, entry)| (branch, entry))
    }

    /// Zahl der Einträge
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Gibt es keinen Eintrag?
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl Serialize for WorktreeMap {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(self.entries.len()))?;
        for (branch, entry) in &self.entries {
            map.serialize_entry(branch, entry)?;
        }
        map.end()
    }
}

impl<'de> Deserialize<'de> for WorktreeMap {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        /// Liest die Paare in der Reihenfolge der Datei
        struct MapVisitor;

        impl<'de> Visitor<'de> for MapVisitor {
            type Value = WorktreeMap;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("ein Objekt mit Branches als Schlüsseln")
            }

            fn visit_map<A: MapAccess<'de>>(
                self,
                mut access: A,
            ) -> std::result::Result<Self::Value, A::Error> {
                let mut entries = Vec::new();
                while let Some((branch, entry)) = access.next_entry::<String, WorktreeEntry>()? {
                    entries.push((branch, entry));
                }
                Ok(WorktreeMap { entries })
            }
        }

        deserializer.deserialize_map(MapVisitor)
    }
}

/// Verzeichnis der Worktrees (Branch → Pfad, Tickets, „bereit zum Merge“); Schreiben nur
/// unter Sperre.
#[derive(Debug)]
pub struct WorktreeRegistry {
    layout: HarnessLayout,
    lock: FileLock,
    writer: AtomicWriter,
}

impl WorktreeRegistry {
    /// Verzeichnis im Harness-Ordner dieses Projekts
    pub fn new(layout: HarnessLayout) -> Self {
        let lock = FileLock::new(layout.worktrees_lock(), FileLockOptions::default());
        Self {
            layout,
            lock,
            writer: AtomicWriter::new(),
        }
    }

    /// Liest alle Einträge (leer, wenn es noch keine gibt)
    pub fn read(&self) -> Result<WorktreeMap> {
        let file = self.layout.worktrees_file();
        if !file.exists() {
            return Ok(WorktreeMap::new());
        }
        let text = fs::read_to_string(&file).map_err(Error::unexpected)?;
        serde_json::from_str(&text).map_err(Error::unexpected)
    }

    /// Legt einen neuen Worktree-Eintrag mit seinem ersten Ticket an
    pub fn add(&self, branch: &str, path: &str, ticket_id: &str) -> Result<()> {
        self.change(|map| map.insert(branch, WorktreeEntry::new(path, ticket_id)))
    }

    /// Nimmt ein weiteres Ticket in den Worktree auf (ohne Doppelung)
    pub fn add_ticket(&self, branch: &str, ticket_id: &str) -> Result<()> {
        self.change(|map| {
            if let Some(entry) = map.get_mut(branch) {
                if !entry.tickets.iter().any(|id| id == ticket_id) {
                    entry.tickets.push(ticket_id.to_string());
                }
            }
        })
    }

    /// Setzt oder löscht die Markierung „bereit zum Merge“
    pub fn set_ready(&self, branch: &str, ready: bool) -> Result<()> {
        self.change(|map| {
            if let Some(entry) = map.get_mut(branch) {
                entry.ready_to_merge = ready;
            }
        })
    }

    /// Merkt den Stand der Einrichtung des Worktrees
    pub fn set_setup(&self, branch: &str, state: SetupState) -> Result<()> {
        self.change(|map| {
            if let Some(entry) = map.get_mut(branch) {
                entry.setup = Some(state);
            }
        })
    }

    /// Markiert den Worktree-Ordner als aufgeräumt
    pub fn mark_removed(&self, branch: &str) -> Result<()> {
        self.change(|map| {
            if let Some(entry) = map.get_mut(branch) {
                entry.removed = Some(true);
            }
        })
    }

    /// Markiert einen wieder angelegten Worktree-Ordner als vorhanden
    pub fn mark_restored(&self, branch: &str) -> Result<()> {
        self.change(|map| {
            if let Some(entry) = map.get_mut(branch) {
                entry.removed = Some(false);
            }
        })
    }

    /// Liest, ändert und schreibt das Verzeichnis in einem Zug unter der Sperre
    fn change(&self, mutate: impl FnOnce(&mut WorktreeMap)) -> Result<()> {
        self.lock.with_lock(|| {
            let mut map = self.read()?;
            mutate(&mut map);
            // Zwei Leerzeichen Einrückung und kein Zeilenende am Schluss wie
            // `JSON.stringify(map, null, 2)` in TypeScript
            let json = serde_json::to_string_pretty(&map).map_err(Error::unexpected)?;
            self.writer.write(&self.layout.worktrees_file(), &json)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::make_temp_dir;
    use std::path::Path;

    /// Verzeichnis in einem temporären Harness-Ordner (den sonst `nexus init` anlegt)
    fn registry(dir: &Path) -> WorktreeRegistry {
        fs::create_dir_all(dir.join("harness")).unwrap();
        WorktreeRegistry::new(HarnessLayout::new(dir))
    }

    #[test]
    fn ist_anfangs_leer() {
        let dir = make_temp_dir("harness-reg-");
        assert_eq!(registry(dir.path()).read().unwrap(), WorktreeMap::new());
    }

    #[test]
    fn fugt_eintrage_und_tickets_hinzu() {
        let dir = make_temp_dir("harness-reg-");
        let registry = registry(dir.path());

        registry
            .add("ticket/T-0001-a", "/pfad/wt", "T-0001")
            .unwrap();
        registry.add_ticket("ticket/T-0001-a", "T-0002").unwrap();

        let map = registry.read().unwrap();
        assert_eq!(map.len(), 1);
        assert_eq!(
            map.get("ticket/T-0001-a"),
            Some(&WorktreeEntry {
                path: "/pfad/wt".into(),
                tickets: vec!["T-0001".into(), "T-0002".into()],
                ready_to_merge: false,
                removed: None,
                setup: None,
            })
        );
        // Dieselbe Form wie in TypeScript: zwei Leerzeichen, nur die gesetzten Felder
        let text = fs::read_to_string(dir.path().join("harness").join("worktrees.json")).unwrap();
        assert_eq!(
            text,
            "{\n  \"ticket/T-0001-a\": {\n    \"path\": \"/pfad/wt\",\n    \"tickets\": [\n      \"T-0001\",\n      \"T-0002\"\n    ],\n    \"readyToMerge\": false\n  }\n}"
        );
    }

    #[test]
    fn setzt_bereit_zum_merge() {
        let dir = make_temp_dir("harness-reg-");
        let registry = registry(dir.path());

        registry.add("b", "/p", "T-0001").unwrap();
        registry.set_ready("b", true).unwrap();

        assert!(registry.read().unwrap().get("b").unwrap().ready_to_merge);
    }

    #[test]
    fn verliert_bei_gleichzeitigen_schreibzugriffen_nichts() {
        let dir = make_temp_dir("harness-reg-");
        let layout = HarnessLayout::new(dir.path());
        fs::create_dir_all(dir.path().join("harness")).unwrap();
        WorktreeRegistry::new(layout.clone())
            .add("b", "/p", "T-0000")
            .unwrap();

        // 15 Schreiber gleichzeitig, jeder mit eigener Sperre wie ein eigener Prozess
        let threads: Vec<_> = (1..=15)
            .map(|number| {
                let layout = layout.clone();
                std::thread::spawn(move || {
                    WorktreeRegistry::new(layout)
                        .add_ticket("b", &format!("T-{number}"))
                        .unwrap();
                })
            })
            .collect();
        for thread in threads {
            thread.join().unwrap();
        }

        let map = WorktreeRegistry::new(layout).read().unwrap();
        assert_eq!(map.get("b").unwrap().tickets.len(), 16);
    }
}
