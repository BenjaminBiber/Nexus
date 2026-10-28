//! Zähler für die Session-Nummern eines Repos; nur unter Sperre gelesen und geschrieben.

use std::collections::BTreeMap;
use std::fs;

use crate::error::{Error, HarnessError, Result};
use crate::locking::{AtomicWriter, FileLock, FileLockOptions};
use crate::project::HarnessLayout;

/// Art des Zählers; ältere Schlüssel (spec, ticket, question) bleiben unverändert erhalten
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CounterKind {
    Session,
}

impl CounterKind {
    /// Schlüssel in `counters.json`
    pub fn as_str(&self) -> &'static str {
        match self {
            CounterKind::Session => "session",
        }
    }
}

/// Stände je Art. Die Schlüssel stehen sortiert in der Datei; JSON kennt keine
/// Reihenfolge, TypeScript liest dieselbe Datei unverändert weiter.
pub type CounterState = BTreeMap<String, i64>;

/// Liest und erhöht die Zählerstände eines Projekts.
#[derive(Debug)]
pub struct Counter {
    layout: HarnessLayout,
    lock: FileLock,
    writer: AtomicWriter,
}

impl Counter {
    /// Zähler im Harness-Ordner dieses Projekts
    pub fn new(layout: HarnessLayout) -> Self {
        let lock = FileLock::new(layout.counters_lock(), FileLockOptions::default());
        Self {
            layout,
            lock,
            writer: AtomicWriter::new(),
        }
    }

    /// Erhöht den Zähler und liefert die neue Nummer (die erste Nummer ist 1)
    pub fn next(&self, kind: CounterKind) -> Result<i64> {
        self.lock.with_lock(|| {
            let mut state = self.read()?;
            let value = state.entry(kind.as_str().to_string()).or_insert(0);
            *value += 1;
            let next = *value;
            let json = serde_json::to_string(&state).map_err(Error::unexpected)?;
            self.writer.write(&self.layout.counters_file(), &json)?;
            Ok(next)
        })
    }

    /// Liest den Stand; fehlende oder beschädigte Datei stoppt, statt bei 1 neu zu beginnen
    pub fn read(&self) -> Result<CounterState> {
        let file = self.layout.counters_file();
        let corrupt = || {
            HarnessError::new(
                "counter-corrupt",
                format!(
                    "Der Zähler ist beschädigt: {}. Es wird nicht bei 1 neu begonnen.",
                    file.display()
                ),
            )
        };
        let text = fs::read_to_string(&file).map_err(|_| corrupt())?;
        let data: serde_json::Map<String, serde_json::Value> =
            serde_json::from_str(&text).map_err(|_| corrupt())?;
        // `session` kam später hinzu: in älteren Dateien fehlt er und beginnt bei 0
        let session = match data.get("session") {
            None => 0,
            Some(value) => value.as_i64().ok_or_else(corrupt)?,
        };
        // Nicht mehr genutzte Schlüssel wie `spec` bleiben erhalten, damit alte Dateien
        // unverändert weiterleben
        let mut state: CounterState = data
            .iter()
            .filter_map(|(key, value)| Some((key.clone(), value.as_i64()?)))
            .collect();
        state.insert("session".to_string(), session);
        Ok(state)
    }
}
