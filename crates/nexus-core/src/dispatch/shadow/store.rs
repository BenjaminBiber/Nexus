//! Die Ablage des Schattenbetriebs unter `<Benutzerordner>/dispatch/shadow/`.
//!
//! Mehr schreibt Rust im Schattenbetrieb nicht: je Tag eine JSONL-Datei für Entscheidungen,
//! Auswertungen und Bereitschaft, dazu Kopien der Lauf- und Ergebnisdateien unter `runs/`,
//! weil der Node-Dispatcher sie nach der Auswertung entfernt.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Local, NaiveDate, Utc};
use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::dispatch::shadow::report::ShadowReport;
use crate::dispatch::shadow::types::{ShadowDecision, ShadowEvaluation, ShadowReadiness};
use crate::dispatch::state::run_store::{RunEnd, RunRecord};
use crate::error::{Error, HarnessError, Result};
use crate::registry::HarnessHome;

/// Vorsilbe der Entscheidungs-Dateien
pub const DECISIONS: &str = "decisions";
/// Vorsilbe der Auswertungs-Dateien
pub const EVALUATIONS: &str = "evaluations";
/// Vorsilbe der Bereitschafts-Dateien
pub const READINESS: &str = "readiness";

/// So viele verglichene Läufe bleiben gemerkt; ältere fallen heraus (ihre Lauf-Dateien gibt
/// es längst nicht mehr, sie können also nicht noch einmal auftauchen)
pub const MAX_DECIDED: usize = 1_000;

/// Liest und schreibt die Dateien des Schattenbetriebs.
#[derive(Debug, Clone)]
pub struct ShadowStore {
    home: HarnessHome,
}

impl ShadowStore {
    /// Ablage im angegebenen Benutzerordner
    pub fn new(home: HarnessHome) -> Self {
        Self { home }
    }

    /// Der Ordner, in den der Schattenbetrieb schreibt
    pub fn dir(&self) -> PathBuf {
        self.home.dispatch_shadow_dir()
    }

    /// Schreibt eine Entscheidung in die Datei ihres Tages
    pub fn append_decision(&self, line: &ShadowDecision) -> Result<()> {
        self.append(DECISIONS, &line.at, line)
    }

    /// Schreibt eine Auswertung in die Datei ihres Tages
    pub fn append_evaluation(&self, line: &ShadowEvaluation) -> Result<()> {
        self.append(EVALUATIONS, &line.at, line)
    }

    /// Schreibt eine Abweichung der Bereitschaft in die Datei ihres Tages
    pub fn append_readiness(&self, line: &ShadowReadiness) -> Result<()> {
        self.append(READINESS, &line.at, line)
    }

    /// Alle Entscheidungen im Zeitraum (Tage einschließlich; `None` = ohne Grenze)
    pub fn decisions(
        &self,
        from: Option<NaiveDate>,
        to: Option<NaiveDate>,
    ) -> Result<Vec<ShadowDecision>> {
        self.read(DECISIONS, from, to)
    }

    /// Alle Auswertungen im Zeitraum
    pub fn evaluations(
        &self,
        from: Option<NaiveDate>,
        to: Option<NaiveDate>,
    ) -> Result<Vec<ShadowEvaluation>> {
        self.read(EVALUATIONS, from, to)
    }

    /// Alle Bereitschafts-Abweichungen im Zeitraum
    pub fn readiness(
        &self,
        from: Option<NaiveDate>,
        to: Option<NaiveDate>,
    ) -> Result<Vec<ShadowReadiness>> {
        self.read(READINESS, from, to)
    }

    /// Der Bericht über einen Zeitraum; die Tage kommen als `JJJJ-MM-TT`, leere Angaben
    /// lassen die Grenze offen
    pub fn report(&self, from: Option<&str>, until: Option<&str>) -> Result<ShadowReport> {
        let start = day("--since", from)?;
        let end = day("--until", until)?;
        Ok(ShadowReport::build(
            start.map(day_text),
            end.map(day_text),
            self.decisions(start, end)?,
            self.evaluations(start, end)?,
            self.readiness(start, end)?,
        ))
    }

    /// Legt eine Kopie des Laufs ab, bevor der Node-Dispatcher ihn entfernt; eine vorhandene
    /// Kopie bleibt, weil sie den Stand beim Start zeigt
    pub fn keep_run(&self, record: &RunRecord) -> Result<()> {
        let file = self.runs_dir().join(format!("{}.json", record.id));
        if file.exists() {
            return Ok(());
        }
        self.write_json(&file, record)
    }

    /// Legt eine Kopie des Ergebnisses ab; eine vorhandene Kopie bleibt
    pub fn keep_result(&self, run_id: &str, end: &RunEnd) -> Result<()> {
        let file = self.runs_dir().join(format!("{run_id}.result.json"));
        if file.exists() {
            return Ok(());
        }
        self.write_json(&file, end)
    }

    /// Der kopierte Lauf oder `None`
    pub fn kept_run(&self, run_id: &str) -> Result<Option<RunRecord>> {
        json(&self.runs_dir().join(format!("{run_id}.json")))
    }

    /// Das kopierte Ergebnis oder `None`
    pub fn kept_result(&self, run_id: &str) -> Result<Option<RunEnd>> {
        json(&self.runs_dir().join(format!("{run_id}.result.json")))
    }

    /// IDs aller Läufe, von denen eine Kopie samt Ergebnis vorliegt
    pub fn kept_results(&self) -> Result<Vec<String>> {
        let dir = self.runs_dir();
        if !dir.exists() {
            return Ok(Vec::new());
        }
        let mut ids = Vec::new();
        for entry in fs::read_dir(&dir).map_err(Error::unexpected)? {
            let entry = entry.map_err(Error::unexpected)?;
            let Some(name) = entry.file_name().to_str().map(str::to_string) else {
                continue;
            };
            if let Some(id) = name.strip_suffix(".result.json") {
                ids.push(id.to_string());
            }
        }
        ids.sort();
        Ok(ids)
    }

    /// Entfernt die Kopien eines Laufs, dessen Auswertung verglichen ist
    pub fn forget_run(&self, run_id: &str) -> Result<()> {
        for name in [format!("{run_id}.json"), format!("{run_id}.result.json")] {
            match fs::remove_file(self.runs_dir().join(name)) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(Error::unexpected(error)),
            }
        }
        Ok(())
    }

    /// Ordner der kopierten Läufe
    pub fn runs_dir(&self) -> PathBuf {
        self.home.dispatch_shadow_runs_dir()
    }

    /// Läufe, deren Entscheidung schon verglichen ist; so bleibt es nach einem Neustart des
    /// Servers bei einer Zeile je Lauf
    pub fn decided(&self) -> Result<Vec<String>> {
        Ok(json(&self.decided_file())?.unwrap_or_default())
    }

    /// Merkt einen verglichenen Lauf; die Liste bleibt auf [`MAX_DECIDED`] begrenzt
    pub fn mark_decided(&self, run_id: &str) -> Result<()> {
        let mut ids = self.decided()?;
        if ids.iter().any(|id| id == run_id) {
            return Ok(());
        }
        ids.push(run_id.to_string());
        if ids.len() > MAX_DECIDED {
            ids.drain(..ids.len() - MAX_DECIDED);
        }
        fs::create_dir_all(self.dir()).map_err(Error::unexpected)?;
        let text = serde_json::to_string(&ids).map_err(Error::unexpected)?;
        fs::write(self.decided_file(), text).map_err(Error::unexpected)
    }

    fn decided_file(&self) -> PathBuf {
        self.dir().join("decided.json")
    }

    /// Hängt eine Zeile an die Tagesdatei einer Vorsilbe an
    fn append<T: Serialize>(&self, prefix: &str, at: &str, value: &T) -> Result<()> {
        fs::create_dir_all(self.dir()).map_err(Error::unexpected)?;
        let file = self.dir().join(format!("{prefix}-{}.jsonl", day_of(at)));
        let line = serde_json::to_string(value).map_err(Error::unexpected)?;
        let mut handle = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&file)
            .map_err(Error::unexpected)?;
        writeln!(handle, "{line}").map_err(Error::unexpected)
    }

    /// Alle Zeilen einer Vorsilbe im Zeitraum, älteste Datei zuerst; unlesbare Zeilen (ein
    /// halb geschriebener Rest) werden übergangen
    fn read<T: DeserializeOwned>(
        &self,
        prefix: &str,
        from: Option<NaiveDate>,
        to: Option<NaiveDate>,
    ) -> Result<Vec<T>> {
        let dir = self.dir();
        if !dir.exists() {
            return Ok(Vec::new());
        }
        let mut files: Vec<(String, PathBuf)> = Vec::new();
        for entry in fs::read_dir(&dir).map_err(Error::unexpected)? {
            let entry = entry.map_err(Error::unexpected)?;
            let Some(name) = entry.file_name().to_str().map(str::to_string) else {
                continue;
            };
            let Some(stamp) = name
                .strip_prefix(&format!("{prefix}-"))
                .and_then(|rest| rest.strip_suffix(".jsonl"))
            else {
                continue;
            };
            let Ok(date) = NaiveDate::parse_from_str(stamp, "%Y%m%d") else {
                continue;
            };
            if from.is_some_and(|from| date < from) || to.is_some_and(|to| date > to) {
                continue;
            }
            files.push((stamp.to_string(), entry.path()));
        }
        files.sort();
        let mut lines = Vec::new();
        for (_, path) in files {
            let text = fs::read_to_string(&path).map_err(Error::unexpected)?;
            for line in text.lines().filter(|line| !line.trim().is_empty()) {
                if let Ok(value) = serde_json::from_str::<T>(line) {
                    lines.push(value);
                }
            }
        }
        Ok(lines)
    }

    fn write_json<T: Serialize>(&self, path: &Path, value: &T) -> Result<()> {
        fs::create_dir_all(self.runs_dir()).map_err(Error::unexpected)?;
        let text = serde_json::to_string_pretty(value).map_err(Error::unexpected)?;
        fs::write(path, text).map_err(Error::unexpected)
    }
}

/// Liest JSON; eine fehlende oder halb geschriebene Datei gilt als nicht vorhanden
fn json<T: DeserializeOwned>(path: &Path) -> Result<Option<T>> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(Error::unexpected(error)),
    };
    Ok(serde_json::from_str(&text).ok())
}

/// Eine Grenze des Zeitraums als Kalendertag; ohne Angabe bleibt sie offen, alles andere
/// (auch der 30. Februar) wird abgelehnt
fn day(option: &str, raw: Option<&str>) -> Result<Option<NaiveDate>> {
    let Some(trimmed) = raw.map(str::trim).filter(|raw| !raw.is_empty()) else {
        return Ok(None);
    };
    NaiveDate::parse_from_str(trimmed, "%Y-%m-%d")
        .ok()
        // Rundlauf: nur was genauso wieder herauskommt, ist ein Tag
        .filter(|day| day_text(*day) == trimmed)
        .map(Some)
        .ok_or_else(|| {
            Error::from(HarnessError::new(
                "date-invalid",
                format!("„{option}“ muss ein Datum JJJJ-MM-TT sein."),
            ))
        })
}

/// Ein Kalendertag als `JJJJ-MM-TT`, wie der Bericht ihn nennt
fn day_text(day: NaiveDate) -> String {
    day.format("%Y-%m-%d").to_string()
}

/// Der Tag eines Zeitpunkts in Ortszeit (`JJJJMMTT`), wie ihn die Lauf-Namen nutzen; ein
/// unlesbarer Zeitpunkt landet im heutigen Tag
pub fn day_of(at: &str) -> String {
    let moment = DateTime::parse_from_rfc3339(at)
        .map(|at| at.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now());
    moment.with_timezone(&Local).format("%Y%m%d").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dispatch::shadow::types::ShadowDiff;

    fn store() -> (tempfile::TempDir, ShadowStore) {
        let dir = tempfile::tempdir().expect("Ordner");
        let home = HarnessHome::at(dir.path());
        (dir, ShadowStore::new(home))
    }

    fn decision(at: &str, run_id: &str, equal: bool) -> ShadowDecision {
        ShadowDecision {
            at: at.into(),
            run_id: run_id.into(),
            ticket_id: "ARC-0001".into(),
            kind: "work".into(),
            equal,
            diffs: if equal {
                Vec::new()
            } else {
                vec![ShadowDiff::new(
                    "attempt",
                    Some("1".into()),
                    Some("2".into()),
                )]
            },
            prompt_diff: None,
            skipped: None,
        }
    }

    #[test]
    fn zeilen_landen_in_der_datei_ihres_tages() {
        let (dir, store) = store();
        let at = Local::now().to_rfc3339();
        store
            .append_decision(&decision(&at, "r1", true))
            .expect("schreiben");
        store
            .append_decision(&decision(&at, "r2", false))
            .expect("schreiben");
        let day = Local::now().format("%Y%m%d").to_string();
        let file = dir
            .path()
            .join("dispatch")
            .join("shadow")
            .join(format!("decisions-{day}.jsonl"));
        let text = fs::read_to_string(file).expect("Datei");
        assert_eq!(text.lines().count(), 2);
        let back = store.decisions(None, None).expect("lesen");
        assert_eq!(back.len(), 2);
        assert_eq!(back[0].run_id, "r1");
        assert!(!back[1].equal);
    }

    #[test]
    fn der_zeitraum_grenzt_die_tage_ein() {
        let (_dir, store) = store();
        for at in [
            "2026-11-01T12:00:00.000+01:00",
            "2026-11-03T12:00:00.000+01:00",
            "2026-11-05T12:00:00.000+01:00",
        ] {
            store
                .append_decision(&decision(at, at, true))
                .expect("schreiben");
        }
        let from = NaiveDate::from_ymd_opt(2026, 11, 2).expect("Datum");
        let to = NaiveDate::from_ymd_opt(2026, 11, 4).expect("Datum");
        let lines = store.decisions(Some(from), Some(to)).expect("lesen");
        assert_eq!(lines.len(), 1);
        assert!(lines[0].run_id.starts_with("2026-11-03"));
    }

    #[test]
    fn der_bericht_nimmt_die_tage_als_jjjj_mm_tt() {
        let (_dir, store) = store();
        for at in [
            "2026-11-01T12:00:00.000+01:00",
            "2026-11-05T12:00:00.000+01:00",
        ] {
            store
                .append_decision(&decision(at, at, false))
                .expect("schreiben");
        }
        let report = store
            .report(Some("2026-11-04"), Some(" "))
            .expect("Bericht");
        assert_eq!(report.from.as_deref(), Some("2026-11-04"));
        assert_eq!(report.until, None);
        assert_eq!(report.summary.decisions_different, 1);
    }

    #[test]
    fn ein_tag_ohne_jjjj_mm_tt_wird_abgelehnt() {
        let (_dir, store) = store();
        for raw in ["1.11.2026", "2026-02-30", "2026-11-1"] {
            let error = store.report(Some(raw), None).expect_err("Fehler");
            assert_eq!(error.code(), Some("date-invalid"));
            assert_eq!(
                error.to_string(),
                "„--since“ muss ein Datum JJJJ-MM-TT sein."
            );
        }
    }

    #[test]
    fn eine_kaputte_zeile_haelt_den_bericht_nicht_auf() {
        let (dir, store) = store();
        let at = Local::now().to_rfc3339();
        store
            .append_decision(&decision(&at, "r1", true))
            .expect("schreiben");
        let day = Local::now().format("%Y%m%d").to_string();
        let file = dir
            .path()
            .join("dispatch")
            .join("shadow")
            .join(format!("decisions-{day}.jsonl"));
        let mut handle = OpenOptions::new().append(true).open(&file).expect("Datei");
        writeln!(handle, "{{halb geschrieben").expect("schreiben");
        assert_eq!(store.decisions(None, None).expect("lesen").len(), 1);
    }

    #[test]
    fn ohne_ordner_gibt_es_nichts_zu_lesen() {
        let (_dir, store) = store();
        assert!(store.decisions(None, None).expect("lesen").is_empty());
        assert!(store.kept_results().expect("lesen").is_empty());
        assert!(store.decided().expect("lesen").is_empty());
    }

    #[test]
    fn verglichene_laeufe_bleiben_gemerkt_und_die_liste_waechst_nicht_endlos() {
        let (_dir, store) = store();
        store.mark_decided("r1").expect("merken");
        store.mark_decided("r1").expect("merken");
        store.mark_decided("r2").expect("merken");
        assert_eq!(store.decided().expect("lesen"), vec!["r1", "r2"]);
        for n in 0..MAX_DECIDED {
            store.mark_decided(&format!("x{n}")).expect("merken");
        }
        let ids = store.decided().expect("lesen");
        assert_eq!(ids.len(), MAX_DECIDED);
        assert!(!ids.contains(&"r1".to_string()));
        assert_eq!(ids.last().map(String::as_str), Some("x999"));
    }
}
