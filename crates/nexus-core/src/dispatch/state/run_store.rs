//! Die Lauf-Dateien unter `~/.harness/dispatch/runs`: `<id>.json` legt der Server an,
//! `<id>.result.json` schreibt der Worker, und unter `stats/` liegen vorgemerkte Lauf-Zeilen,
//! bis sie in der Datenbank stehen. Portiert aus
//! `packages/core/src/dispatch/state/RunStore.ts`.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::dispatch::config::{RunSettings, WorktreeSetupStep};
use crate::dispatch::invocation::RunKind;
use crate::dispatch::types::RunResult;
use crate::error::{Error, Result};
use crate::locking::AtomicWriter;
use crate::registry::HarnessHome;

/// Ein gestarteter Worker-Lauf; der Worker-Prozess liest ihn und schreibt sein Ergebnis daneben
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "dispatch.ts")]
pub struct RunRecord {
    pub id: String,
    pub kind: RunKind,
    /// Ticket des Laufs; bei einer Recherche die Fragen-ID (Gegenstand des Laufs)
    pub ticket_id: String,
    pub board_id: i64,
    pub title: String,
    pub agent: String,
    /// Arbeitsordner (Worktree des Tickets)
    pub cwd: String,
    pub branch: String,
    /// Merge-Session: Ziel-Branch im Hauptordner
    pub target: Option<String>,
    pub run: RunSettings,
    /// Umsetzung: Nummer des Versuchs (gescheiterte Versuche + 1)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub attempt: Option<i64>,
    pub command: String,
    pub args: Vec<String>,
    pub prompt: String,
    pub log_file: String,
    pub started_at: String,
    pub pid: Option<i64>,
    /// Umsetzung in einem neu angelegten Worktree: Einrichtungsbefehle, die der Worker vor Claude
    /// ausführt, und ihr Zeitlimit
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub setup: Option<Vec<WorktreeSetupStep>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub setup_timeout_minutes: Option<i64>,
    /// Recherche: ID der Frage
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub question_id: Option<String>,
    /// Recherche: Zahl der Recherchen der Frage beim Start (danach mehr = Ergebnis da)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub research_before: Option<i64>,
    /// Planung: `plannedAt` des Tickets beim Start (danach ein neuerer Wert = Plan gespeichert)
    #[serde(
        default,
        deserialize_with = "crate::dispatch::types::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    #[ts(optional)]
    pub planned_before: Option<Option<String>>,
    /// Recherche und Planung: Hauptordner des Repos, zu dem der losgelöste Worktree gehört
    /// (zum Aufräumen)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub repo_root: Option<String>,
}

/// Ergebnis eines Laufs samt Endzeit; schreibt nur der Worker
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "dispatch.ts")]
pub struct RunEnd {
    #[serde(flatten)]
    #[ts(flatten)]
    pub result: RunResult,
    pub ended_at: String,
}

/// Liest und schreibt die Lauf-Dateien.
#[derive(Debug, Clone)]
pub struct RunStore {
    home: HarnessHome,
    writer: AtomicWriter,
}

impl RunStore {
    /// Store im angegebenen Benutzerordner
    pub fn new(home: HarnessHome) -> Self {
        Self {
            home,
            writer: AtomicWriter::new(),
        }
    }

    /// Legt einen Lauf an
    pub fn create(&self, record: &RunRecord) -> Result<()> {
        fs::create_dir_all(self.home.dispatch_runs_dir()).map_err(Error::unexpected)?;
        self.write_json(&self.file(&record.id), record)
    }

    /// Merkt die Prozess-ID des Workers
    pub fn set_pid(&self, id: &str, pid: Option<i64>) -> Result<()> {
        if let Some(mut record) = self.read(id)? {
            record.pid = pid;
            self.write_json(&self.file(id), &record)?;
        }
        Ok(())
    }

    /// Lauf zur ID oder `None`
    pub fn read(&self, id: &str) -> Result<Option<RunRecord>> {
        self.json(&self.file(id))
    }

    /// Alle Läufe, ältester zuerst
    pub fn list(&self) -> Result<Vec<RunRecord>> {
        let dir = self.home.dispatch_runs_dir();
        if !dir.exists() {
            return Ok(Vec::new());
        }
        let mut records = Vec::new();
        for name in names_in(&dir)? {
            if !name.ends_with(".json") || name.ends_with(".result.json") {
                continue;
            }
            if let Some(record) = self.json::<RunRecord>(&dir.join(&name))? {
                records.push(record);
            }
        }
        records.sort_by(|a, b| a.started_at.cmp(&b.started_at));
        Ok(records)
    }

    /// Ergebnis des Workers oder `None`, solange er läuft
    pub fn result(&self, id: &str) -> Result<Option<RunEnd>> {
        self.json(&self.result_file(id))
    }

    /// Schreibt das Ergebnis (nur der Worker)
    pub fn finish(&self, id: &str, end: &RunEnd) -> Result<()> {
        fs::create_dir_all(self.home.dispatch_runs_dir()).map_err(Error::unexpected)?;
        self.write_json(&self.result_file(id), end)
    }

    /// Entfernt einen ausgewerteten Lauf
    pub fn remove(&self, id: &str) -> Result<()> {
        remove_file(&self.file(id))?;
        remove_file(&self.result_file(id))
    }

    /// Merkt eine Lauf-Zeile für die Auswertung vor (Unterordner `stats`), bis sie in der
    /// Datenbank steht
    pub fn keep_stats<T: Serialize>(&self, id: &str, row: &T) -> Result<()> {
        fs::create_dir_all(self.stats_dir()).map_err(Error::unexpected)?;
        self.write_json(&self.stats_dir().join(format!("{id}.json")), row)
    }

    /// Vorgemerkte Lauf-Zeilen, nach ID
    pub fn kept_stats<T: DeserializeOwned>(&self) -> Result<Vec<(String, T)>> {
        let dir = self.stats_dir();
        if !dir.exists() {
            return Ok(Vec::new());
        }
        let mut names: Vec<String> = names_in(&dir)?
            .into_iter()
            .filter(|n| n.ends_with(".json"))
            .collect();
        names.sort();
        let mut out = Vec::new();
        for name in names {
            if let Some(row) = self.json::<T>(&dir.join(&name))? {
                out.push((name[..name.len() - ".json".len()].to_string(), row));
            }
        }
        Ok(out)
    }

    /// Entfernt eine vorgemerkte Lauf-Zeile
    pub fn forget_stats(&self, id: &str) -> Result<()> {
        remove_file(&self.stats_dir().join(format!("{id}.json")))
    }

    fn stats_dir(&self) -> PathBuf {
        self.home.dispatch_runs_dir().join("stats")
    }

    /// Liest JSON; fehlende oder halb geschriebene Dateien gelten als nicht vorhanden, andere
    /// Lesefehler werfen
    fn json<T: DeserializeOwned>(&self, path: &Path) -> Result<Option<T>> {
        let text = match fs::read_to_string(path) {
            Ok(text) => text,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(Error::unexpected(error)),
        };
        Ok(serde_json::from_str(&text).ok())
    }

    fn write_json<T: Serialize>(&self, path: &Path, value: &T) -> Result<()> {
        let text = serde_json::to_string_pretty(value).map_err(Error::unexpected)?;
        self.writer.write(path, &text)
    }

    fn file(&self, id: &str) -> PathBuf {
        self.home.dispatch_runs_dir().join(format!("{id}.json"))
    }

    fn result_file(&self, id: &str) -> PathBuf {
        self.home
            .dispatch_runs_dir()
            .join(format!("{id}.result.json"))
    }
}

/// Dateinamen eines Ordners (wie `readdir`)
pub(crate) fn names_in(dir: &Path) -> Result<Vec<String>> {
    let mut names = Vec::new();
    for entry in fs::read_dir(dir).map_err(Error::unexpected)? {
        let entry = entry.map_err(Error::unexpected)?;
        if let Some(name) = entry.file_name().to_str() {
            names.push(name.to_string());
        }
    }
    Ok(names)
}

/// Entfernt eine Datei; fehlt sie, ist nichts zu tun (wie `rm(..., { force: true })`)
pub(crate) fn remove_file(path: &Path) -> Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(Error::unexpected(error)),
    }
}
