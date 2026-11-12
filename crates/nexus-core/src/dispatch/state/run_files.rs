//! Ablage neben dem Protokoll eines Laufs: der Ordner `<Lauf-ID>.files/` mit der Session von
//! Claude Code (`session.json`) und den Screenshots des Agenten (`shots.jsonl` und die Bilder).
//! Er entsteht erst, wenn es etwas abzulegen gibt.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::clock::iso;
use crate::error::{Error, HarnessError, Result};
use crate::registry::HarnessHome;

/// Höchstens so viele Screenshots hält ein Lauf fest
pub const MAX_SHOTS: usize = 60;
/// Größer darf ein Screenshot nicht sein
pub const MAX_SHOT_BYTES: usize = 8 * 1024 * 1024;

const SESSION_FILE: &str = "session.json";
const INDEX_FILE: &str = "shots.jsonl";

/// Die Session von Claude Code zu einem Lauf; mit ihr findet sich das Transcript
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunSession {
    pub session_id: String,
    /// Arbeitsordner des Laufs (in ihm hat Claude Code das Transcript abgelegt)
    pub cwd: String,
}

/// Ein abgelegter Screenshot
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunShot {
    /// Dateiname im Ordner, z. B. `001.png`
    pub file: String,
    /// ID des Tool-Aufrufs, dessen Ergebnis das Bild war
    pub tool_use_id: String,
    /// Zeitpunkt der Ablage (ISO)
    pub at: String,
    pub media_type: String,
    pub bytes: u64,
}

/// Der Ordner `<Lauf-ID>.files/` eines Laufs
#[derive(Debug, Clone)]
pub struct RunFiles {
    dir: PathBuf,
}

/// Dateinamen von Protokollen – dieselbe Regel wie in `DispatchLog::read`
fn log_pattern() -> Regex {
    Regex::new(r"^[A-Za-z0-9._-]+\.log$").expect("festes Muster")
}

/// Dateinamen abgelegter Bilder
fn shot_pattern() -> Regex {
    Regex::new(r"^\d{3}\.(png|jpg|gif|webp)$").expect("festes Muster")
}

/// Endung zum Medientyp; `None` heißt: wird nicht abgelegt
fn extension(media_type: &str) -> Option<&'static str> {
    match media_type {
        "image/png" => Some("png"),
        "image/jpeg" => Some("jpg"),
        "image/gif" => Some("gif"),
        "image/webp" => Some("webp"),
        _ => None,
    }
}

impl RunFiles {
    /// Die Ablage zu einem Protokoll: `<…>/x.log` → `<…>/x.files`
    pub fn beside(log_file: &Path) -> Self {
        Self {
            dir: log_file.with_extension("files"),
        }
    }

    /// Die Ablage zum Protokoll `name` im Protokollordner; `name` muss ein Dateiname eines Protokolls sein
    pub fn for_run(home: &HarnessHome, name: &str) -> Result<Self> {
        if !log_pattern().is_match(name) || name.contains("..") {
            return Err(HarnessError::new(
                "log-name-invalid",
                format!("„{name}“ ist kein Protokoll des Dispatchers."),
            )
            .into());
        }
        Ok(Self::beside(&home.dispatch_logs_dir().join(name)))
    }

    /// Der Ordner der Ablage (muss nicht existieren)
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Hält die Session des Laufs fest
    pub fn write_session(&self, session: &RunSession) -> Result<()> {
        fs::create_dir_all(&self.dir).map_err(Error::unexpected)?;
        let text = serde_json::to_string(session).map_err(Error::unexpected)?;
        fs::write(self.dir.join(SESSION_FILE), text).map_err(Error::unexpected)?;
        Ok(())
    }

    /// Die festgehaltene Session; `None`, wenn es keine gibt oder die Datei nicht lesbar ist
    pub fn session(&self) -> Option<RunSession> {
        let text = fs::read_to_string(self.dir.join(SESSION_FILE)).ok()?;
        serde_json::from_str(&text).ok()
    }

    /// Legt einen Screenshot ab und trägt ihn in den Index ein. `Ok(None)`: nicht abgelegt
    /// (unbekannter Typ, leer, zu groß oder die Grenze `MAX_SHOTS` ist erreicht).
    pub fn add_shot(
        &self,
        tool_use_id: &str,
        media_type: &str,
        data: &[u8],
        at: DateTime<Utc>,
    ) -> Result<Option<RunShot>> {
        let Some(ext) = extension(media_type) else {
            return Ok(None);
        };
        if data.is_empty() || data.len() > MAX_SHOT_BYTES {
            return Ok(None);
        }
        let count = self.shots().len();
        if count >= MAX_SHOTS {
            return Ok(None);
        }
        fs::create_dir_all(&self.dir).map_err(Error::unexpected)?;
        let shot = RunShot {
            file: format!("{:03}.{ext}", count + 1),
            tool_use_id: tool_use_id.to_string(),
            at: iso(at),
            media_type: media_type.to_string(),
            bytes: data.len() as u64,
        };
        fs::write(self.dir.join(&shot.file), data).map_err(Error::unexpected)?;
        let line = serde_json::to_string(&shot).map_err(Error::unexpected)?;
        let mut index = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.dir.join(INDEX_FILE))
            .map_err(Error::unexpected)?;
        writeln!(index, "{line}").map_err(Error::unexpected)?;
        Ok(Some(shot))
    }

    /// Alle abgelegten Screenshots in der Reihenfolge der Ablage; ohne Ablage leer
    pub fn shots(&self) -> Vec<RunShot> {
        let Ok(text) = fs::read_to_string(self.dir.join(INDEX_FILE)) else {
            return Vec::new();
        };
        let pattern = shot_pattern();
        text.lines()
            .filter_map(|line| serde_json::from_str::<RunShot>(line).ok())
            .filter(|shot| pattern.is_match(&shot.file))
            .collect()
    }

    /// Ein Screenshot samt Pfad – nur, wenn er im Index steht und die Datei existiert
    pub fn find_shot(&self, file: &str) -> Option<(RunShot, PathBuf)> {
        let shot = self.shots().into_iter().find(|shot| shot.file == file)?;
        let path = self.dir.join(&shot.file);
        path.is_file().then_some((shot, path))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    const LOG: &str = "ARC-0001-20261016-101500.log";

    fn at() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 10, 16, 8, 16, 0).unwrap()
    }

    fn files(dir: &tempfile::TempDir) -> RunFiles {
        RunFiles::beside(&dir.path().join(LOG))
    }

    #[test]
    fn liegt_neben_dem_protokoll() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            files(&dir).dir(),
            dir.path().join("ARC-0001-20261016-101500.files")
        );
        let home = HarnessHome::at(dir.path());
        let run = RunFiles::for_run(&home, LOG).unwrap();
        assert_eq!(
            run.dir(),
            home.dispatch_logs_dir()
                .join("ARC-0001-20261016-101500.files")
        );
    }

    #[test]
    fn lehnt_namen_ab_die_kein_protokoll_sind() {
        let dir = tempfile::tempdir().unwrap();
        let home = HarnessHome::at(dir.path());
        for name in ["x.txt", "../x.log", "a/b.log", "a..b.log", ""] {
            let error = RunFiles::for_run(&home, name).unwrap_err();
            assert_eq!(error.code(), Some("log-name-invalid"), "{name}");
        }
    }

    #[test]
    fn haelt_die_session_fest() {
        let dir = tempfile::tempdir().unwrap();
        let files = files(&dir);
        assert_eq!(files.session(), None);
        assert!(!files.dir().exists(), "ohne Ablage kein Ordner");
        let session = RunSession {
            session_id: "sitzung-1".into(),
            cwd: "C:/work/ARC-0001".into(),
        };
        files.write_session(&session).unwrap();
        assert_eq!(files.session(), Some(session));
        let text = fs::read_to_string(files.dir().join("session.json")).unwrap();
        assert_eq!(
            text,
            r#"{"sessionId":"sitzung-1","cwd":"C:/work/ARC-0001"}"#
        );
    }

    #[test]
    fn legt_screenshots_nummeriert_ab() {
        let dir = tempfile::tempdir().unwrap();
        let files = files(&dir);
        assert!(files.shots().is_empty());
        let first = files
            .add_shot("toolu_1", "image/png", b"eins", at())
            .unwrap()
            .unwrap();
        let second = files
            .add_shot("toolu_2", "image/jpeg", b"zwei!", at())
            .unwrap()
            .unwrap();
        assert_eq!(
            first,
            RunShot {
                file: "001.png".into(),
                tool_use_id: "toolu_1".into(),
                at: "2026-10-16T08:16:00.000Z".into(),
                media_type: "image/png".into(),
                bytes: 4,
            }
        );
        assert_eq!(second.file, "002.jpg");
        assert_eq!(files.shots(), vec![first.clone(), second]);
        assert_eq!(fs::read(files.dir().join("001.png")).unwrap(), b"eins");
        assert_eq!(
            files.find_shot("001.png"),
            Some((first, files.dir().join("001.png")))
        );
        let index = fs::read_to_string(files.dir().join("shots.jsonl")).unwrap();
        assert!(index.starts_with(r#"{"file":"001.png","toolUseId":"toolu_1","at":"2026-10-16T08:16:00.000Z","mediaType":"image/png","bytes":4}"#), "{index}");
    }

    #[test]
    fn uebergeht_was_nicht_abgelegt_wird() {
        let dir = tempfile::tempdir().unwrap();
        let files = files(&dir);
        assert_eq!(
            files
                .add_shot("t", "image/svg+xml", b"<svg/>", at())
                .unwrap(),
            None
        );
        assert_eq!(files.add_shot("t", "image/png", b"", at()).unwrap(), None);
        assert_eq!(
            files
                .add_shot("t", "image/png", &vec![0u8; MAX_SHOT_BYTES + 1], at())
                .unwrap(),
            None
        );
        assert!(files.shots().is_empty());
        assert!(!files.dir().exists(), "nichts abgelegt, kein Ordner");
        for _ in 0..MAX_SHOTS {
            files
                .add_shot("t", "image/png", b"x", at())
                .unwrap()
                .unwrap();
        }
        assert_eq!(files.add_shot("t", "image/png", b"x", at()).unwrap(), None);
        assert_eq!(files.shots().len(), MAX_SHOTS);
        assert_eq!(files.shots().last().unwrap().file, "060.png");
    }

    #[test]
    fn liest_nur_gueltige_zeilen_des_index() {
        let dir = tempfile::tempdir().unwrap();
        let files = files(&dir);
        files
            .add_shot("toolu_1", "image/png", b"eins", at())
            .unwrap();
        let mut index = fs::OpenOptions::new()
            .append(true)
            .open(files.dir().join("shots.jsonl"))
            .unwrap();
        writeln!(index, "kaputt").unwrap();
        writeln!(index, r#"{{"file":"../geheim.png","toolUseId":"t","at":"x","mediaType":"image/png","bytes":1}}"#).unwrap();
        writeln!(
            index,
            r#"{{"file":"002.png","toolUseId":"t","at":"x","mediaType":"image/png","bytes":1}}"#
        )
        .unwrap();
        let names: Vec<String> = files.shots().into_iter().map(|shot| shot.file).collect();
        assert_eq!(names, ["001.png", "002.png"]);
        assert_eq!(files.find_shot("../geheim.png"), None);
        assert_eq!(files.find_shot("002.png"), None, "Indexzeile ohne Datei");
        fs::write(files.dir().join("fremd.png"), b"x").unwrap();
        assert_eq!(files.find_shot("fremd.png"), None, "Datei ohne Indexzeile");
    }
}
