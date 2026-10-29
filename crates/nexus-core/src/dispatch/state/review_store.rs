//! Review-Einträge unter `~/.nexus/dispatch/reviews/<ID>.json`; portiert aus
//! `packages/core/src/dispatch/state/ReviewStore.ts`.

use std::fs;
use std::io;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::dispatch::state::run_store::remove_file;
use crate::dispatch::types::TestRun;
use crate::error::{Error, Result};
use crate::locking::AtomicWriter;
use crate::registry::HarnessHome;

/// Review eines Tickets vor dem Merge: Lauf und Testläufe des Agenten
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "dispatch.ts")]
pub struct ReviewEntry {
    pub ticket_id: String,
    pub branch: String,
    pub run_id: String,
    /// Lauf-Protokoll des Laufs
    pub log_file: String,
    /// Zeitpunkt des Gates (ISO)
    pub at: String,
    pub tests: Vec<TestRun>,
}

/// Liest und schreibt die Review-Einträge.
#[derive(Debug, Clone)]
pub struct ReviewStore {
    home: HarnessHome,
    writer: AtomicWriter,
}

impl ReviewStore {
    /// Store im angegebenen Benutzerordner
    pub fn new(home: HarnessHome) -> Self {
        Self {
            home,
            writer: AtomicWriter::new(),
        }
    }

    /// Speichert den Eintrag (ersetzt einen früheren desselben Tickets)
    pub fn save(&self, entry: &ReviewEntry) -> Result<()> {
        fs::create_dir_all(self.home.dispatch_reviews_dir()).map_err(Error::unexpected)?;
        let text = serde_json::to_string_pretty(entry).map_err(Error::unexpected)?;
        self.writer.write(&self.file(&entry.ticket_id), &text)
    }

    /// Eintrag zum Ticket oder `None`; fehlende oder kaputte Dateien gelten als nicht vorhanden
    pub fn read(&self, ticket_id: &str) -> Result<Option<ReviewEntry>> {
        let text = match fs::read_to_string(self.file(ticket_id)) {
            Ok(text) => text,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(Error::unexpected(error)),
        };
        Ok(serde_json::from_str(&text).ok())
    }

    /// Entfernt den Eintrag
    pub fn remove(&self, ticket_id: &str) -> Result<()> {
        remove_file(&self.file(ticket_id))
    }

    /// Datei des Tickets; alles außer `[A-Za-z0-9_-]` im Namen wird zu `_`
    fn file(&self, ticket_id: &str) -> PathBuf {
        let safe: String = ticket_id
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        self.home
            .dispatch_reviews_dir()
            .join(format!("{safe}.json"))
    }
}
