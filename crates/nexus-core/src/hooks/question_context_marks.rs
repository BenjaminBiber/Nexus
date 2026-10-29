//! Merkt sich je Session, wann zuletzt offene Fragen eingespeist wurden, damit sich der Text
//! nicht bei jedem Prompt wiederholt.
//! Portiert aus `packages/core/src/hooks/QuestionContextMarks.ts`.

use std::collections::BTreeMap;
use std::fs;

use crate::clock::Clock;
use crate::error::{Error, Result};
use crate::locking::{AtomicWriter, FileLock, FileLockOptions};
use crate::project::HarnessLayout;

/// Einträge, die älter als 30 Tage sind, fallen beim Schreiben weg
const KEEP_MS: i64 = 30 * 24 * 60 * 60 * 1000;

/// Session-Kennung → Zeitpunkt der letzten Einspeisung
type Marks = BTreeMap<String, String>;

/// Liest und schreibt die Marken der Fragen-Einspeisung eines Projekts
#[derive(Debug)]
pub struct QuestionContextMarks {
    layout: HarnessLayout,
    lock: FileLock,
    writer: AtomicWriter,
    clock: Clock,
}

impl QuestionContextMarks {
    /// Marken dieses Projektordners mit dieser Uhr
    pub fn new(layout: HarnessLayout, clock: Clock) -> Self {
        let lock = FileLock::new(layout.question_context_lock(), FileLockOptions::default());
        Self {
            layout,
            lock,
            writer: AtomicWriter::new(),
            clock,
        }
    }

    /// Zeitpunkt der letzten Einspeisung für diese Session; eine fehlende oder beschädigte
    /// Datei zählt als „nie“
    pub fn get(&self, session_id: &str) -> Option<String> {
        self.read().get(session_id).cloned()
    }

    /// Setzt den Zeitpunkt und räumt Einträge auf, die älter als 30 Tage sind
    pub fn set(&self, session_id: &str, iso: &str) -> Result<()> {
        self.lock.with_lock(|| {
            let mut marks = self.read();
            marks.insert(session_id.to_string(), iso.to_string());
            let now = self.clock.now().timestamp_millis();
            marks.retain(|_, at| match chrono::DateTime::parse_from_rfc3339(at) {
                Ok(at) => now - at.timestamp_millis() < KEEP_MS,
                // Ein unlesbarer Zeitpunkt ist in JavaScript NaN und fällt damit weg
                Err(_) => false,
            });
            self.writer.write(
                &self.layout.question_context_file(),
                &serde_json::to_string(&marks).map_err(Error::unexpected)?,
            )
        })
    }

    /// Liest die Datei; Fehler ergeben eine leere Zuordnung
    fn read(&self) -> Marks {
        fs::read_to_string(self.layout.question_context_file())
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Marken in einem leeren Projektordner
    fn marks(dir: &std::path::Path, now: &str) -> QuestionContextMarks {
        let layout = HarnessLayout::new(dir);
        fs::create_dir_all(layout.dir()).unwrap();
        QuestionContextMarks::new(
            layout,
            Clock::fixed(now.parse().expect("der Zeitpunkt ist gültig")),
        )
    }

    #[test]
    fn liefert_ohne_datei_nichts() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            marks(dir.path(), "2026-10-10T10:00:00.000Z").get("s-1"),
            None
        );
    }

    #[test]
    fn merkt_sich_den_zeitpunkt_je_session() {
        let dir = tempfile::tempdir().unwrap();
        let m = marks(dir.path(), "2026-10-10T10:00:00.000Z");
        m.set("s-1", "2026-10-10T09:00:00.000Z").unwrap();
        assert_eq!(m.get("s-1").as_deref(), Some("2026-10-10T09:00:00.000Z"));
        assert_eq!(m.get("s-2"), None);
    }

    #[test]
    fn raumt_eintrage_auf_die_alter_als_dreissig_tage_sind() {
        let dir = tempfile::tempdir().unwrap();
        let alt = marks(dir.path(), "2026-10-10T10:00:00.000Z");
        alt.set("alt", "2026-10-10T10:00:00.000Z").unwrap();
        let spater = marks(dir.path(), "2026-12-01T10:00:00.000Z");
        spater.set("neu", "2026-12-01T10:00:00.000Z").unwrap();
        assert_eq!(spater.get("alt"), None);
        assert_eq!(
            spater.get("neu").as_deref(),
            Some("2026-12-01T10:00:00.000Z")
        );
    }

    #[test]
    fn eine_beschadigte_datei_zahlt_als_nie() {
        let dir = tempfile::tempdir().unwrap();
        let m = marks(dir.path(), "2026-10-10T10:00:00.000Z");
        fs::write(m.layout.question_context_file(), "{kaputt").unwrap();
        assert_eq!(m.get("s-1"), None);
        m.set("s-1", "2026-10-10T10:00:00.000Z").unwrap();
        assert_eq!(m.get("s-1").as_deref(), Some("2026-10-10T10:00:00.000Z"));
    }
}
