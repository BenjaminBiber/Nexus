//! Protokolle des Dispatchers: `dispatch-logs/dispatch.log` (Ereigniszeilen) und je Lauf eine
//! Datei mit Arbeitsanweisung und Ausgabe. Portiert aus
//! `packages/core/src/dispatch/state/DispatchLog.ts`.

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Local, NaiveDate, TimeZone, Utc};
use regex::Regex;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::clock::{iso, Clock};
use crate::dispatch::invocation::RunKind;
use crate::dispatch::state::run_store::names_in;
use crate::error::{Error, HarnessError, Result};
use crate::registry::HarnessHome;

/// Über dieser Größe wird `dispatch.log` neu begonnen (1 MB)
const MAX_LOG_BYTES: u64 = 1024 * 1024;
/// Höchstens so viele Zeichen liefert [`DispatchLog::read`]
const MAX_READ_CHARS: usize = 200_000;
/// Höchstens so viele Bytes werden vom Ende einer Protokolldatei gelesen
const MAX_READ_BYTES: u64 = MAX_READ_CHARS as u64 * 4;

/// Lauf-Protokoll eines Tickets oder einer Frage
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "dispatch.ts")]
pub struct RunLogEntry {
    /// Dateiname, z. B. `ARC-0001-20261016-091500-merge.log`
    pub name: String,
    pub kind: RunKind,
    /// Start laut Namen als ISO-Zeit
    pub started: Option<String>,
}

/// Erlaubter Name einer Protokolldatei
fn name_pattern() -> Regex {
    Regex::new(r"^[A-Za-z0-9._-]+\.log$").expect("festes Muster")
}

/// Zeitstempel im Namen einer Lauf-Protokolldatei
fn stamp_pattern() -> Regex {
    Regex::new(r"(\d{8}-\d{6})").expect("festes Muster")
}

/// Erlaubte Ticket- oder Fragen-ID
fn id_pattern() -> Regex {
    Regex::new(r"^[A-Z][A-Z0-9]*-[A-Z]?\d+$").expect("festes Muster")
}

/// Liest und schreibt die Protokolle des Dispatchers.
#[derive(Debug, Clone)]
pub struct DispatchLog {
    home: HarnessHome,
    clock: Clock,
}

impl DispatchLog {
    /// Protokoll im angegebenen Benutzerordner
    pub fn new(home: HarnessHome) -> Self {
        Self::with_clock(home, Clock::system())
    }

    /// Protokoll mit eigener Uhr (für Tests mit festen Zeitstempeln)
    pub fn with_clock(home: HarnessHome, clock: Clock) -> Self {
        Self { home, clock }
    }

    /// Hängt eine Zeile an `dispatch.log` an; über 1 MB wird neu begonnen. Fehler sind kein
    /// Grund zum Abbruch – das Protokoll ist nur Hilfe
    pub fn line(&self, text: &str) {
        let _ = self.try_line(text);
    }

    /// Der Versuch hinter [`DispatchLog::line`]; der Fehler wird dort verschluckt
    fn try_line(&self, text: &str) -> std::io::Result<()> {
        fs::create_dir_all(self.home.dispatch_logs_dir())?;
        let file = self.file_path("dispatch.log");
        let size = fs::metadata(&file).map(|m| m.len()).unwrap_or(0);
        if size > MAX_LOG_BYTES {
            File::create(&file)?;
        }
        let mut handle = OpenOptions::new().create(true).append(true).open(&file)?;
        writeln!(handle, "{}  {}", self.stamp(self.clock.now()), text)
    }

    /// Öffnet (und legt bei Bedarf den Ordner an) eine Lauf-Protokolldatei zum Anhängen
    pub fn open_run_log(&self, file: &Path) -> Result<File> {
        if let Some(parent) = file.parent() {
            fs::create_dir_all(parent).map_err(Error::unexpected)?;
        }
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(file)
            .map_err(Error::unexpected)
    }

    /// Name eines Lauf-Protokolls (ohne `.log`):
    /// `<ID>-<JJJJMMTT-HHMMSS>[-merge|-research|-plan]` in Ortszeit
    pub fn run_name(&self, ticket_id: &str, kind: RunKind, at: DateTime<Utc>) -> String {
        let local = at.with_timezone(&Local);
        let stamp = local.format("%Y%m%d-%H%M%S");
        let suffix = match kind {
            RunKind::Work => String::new(),
            RunKind::Merge => "-merge".to_string(),
            RunKind::Research => "-research".to_string(),
            RunKind::Plan => "-plan".to_string(),
        };
        format!("{ticket_id}-{stamp}{suffix}")
    }

    /// Voller Pfad einer Protokolldatei
    pub fn file_path(&self, name: &str) -> PathBuf {
        self.home.dispatch_logs_dir().join(name)
    }

    /// Protokolldateien: `dispatch.log` zuerst, danach die neuesten Läufe
    pub fn list(&self) -> Result<Vec<String>> {
        let dir = self.home.dispatch_logs_dir();
        if !dir.exists() {
            return Ok(Vec::new());
        }
        let allowed = name_pattern();
        let names: Vec<String> = names_in(&dir)?
            .into_iter()
            .filter(|n| allowed.is_match(n))
            .collect();
        let mut runs: Vec<String> = names
            .iter()
            .filter(|n| *n != "dispatch.log")
            .cloned()
            .collect();
        let stamps = stamp_pattern();
        runs.sort_by(|a, b| {
            stamp_of(&stamps, b)
                .cmp(&stamp_of(&stamps, a))
                .then_with(|| a.cmp(b))
        });
        let mut out = Vec::new();
        if names.iter().any(|n| n == "dispatch.log") {
            out.push("dispatch.log".to_string());
        }
        out.extend(runs);
        Ok(out)
    }

    /// Lauf-Protokolle eines Tickets oder einer Frage, neueste zuerst. Der Name beginnt genau mit
    /// `<ID>-` und dem Zeitstempel (`ARC-00010-…` gehört nicht zu `ARC-0001`); `started` ist der
    /// Zeitstempel des Namens (Ortszeit) als ISO-Zeit
    pub fn runs(&self, id: &str) -> Result<Vec<RunLogEntry>> {
        let ticket = id.trim().to_uppercase();
        if !id_pattern().is_match(&ticket) {
            return Err(HarnessError::new(
                "id-invalid",
                format!("„{id}“ ist keine Ticket- oder Fragen-ID."),
            )
            .into());
        }
        let dir = self.home.dispatch_logs_dir();
        if !dir.exists() {
            return Ok(Vec::new());
        }
        let own = Regex::new(&format!(
            r"^{}-(\d{{8}}-\d{{6}})(?:-(merge|research|plan))?\.log$",
            regex::escape(&ticket)
        ))
        .map_err(Error::unexpected)?;
        let mut out = Vec::new();
        for name in names_in(&dir)? {
            let Some(found) = own.captures(&name) else {
                continue;
            };
            let kind = match found.get(2).map(|m| m.as_str()) {
                Some("merge") => RunKind::Merge,
                Some("research") => RunKind::Research,
                Some("plan") => RunKind::Plan,
                _ => RunKind::Work,
            };
            let started = started_at(&found[1]);
            out.push(RunLogEntry {
                name,
                kind,
                started,
            });
        }
        let stamps = stamp_pattern();
        out.sort_by(|a, b| {
            stamp_of(&stamps, &b.name)
                .cmp(&stamp_of(&stamps, &a.name))
                .then_with(|| a.name.cmp(&b.name))
        });
        Ok(out)
    }

    /// Inhalt (Ende) einer Protokolldatei; nur einfache Dateinamen, die tatsächlich im
    /// Protokollordner liegen (reservierte Gerätenamen wie `CON.log` fallen so heraus).
    /// Gelesen wird nur das Ende der Datei
    pub fn read(&self, name: &str) -> Result<String> {
        if !name_pattern().is_match(name) || name.contains("..") {
            return Err(HarnessError::new(
                "log-name-invalid",
                format!("„{name}“ ist kein Protokoll des Dispatchers."),
            )
            .into());
        }
        let dir = self.home.dispatch_logs_dir();
        let names = if dir.exists() {
            names_in(&dir)?
        } else {
            Vec::new()
        };
        if !names.iter().any(|n| n == name) {
            return Err(HarnessError::new(
                "log-not-found",
                format!("Das Protokoll {name} gibt es nicht."),
            )
            .into());
        }
        let bytes = tail_bytes(&self.file_path(name), MAX_READ_BYTES)?;
        let text = String::from_utf8_lossy(&bytes.1);
        let chars = text.chars().count();
        if bytes.0 > 0 || chars > MAX_READ_CHARS {
            let skip = chars.saturating_sub(MAX_READ_CHARS);
            Ok(format!("…{}", text.chars().skip(skip).collect::<String>()))
        } else {
            Ok(text.into_owned())
        }
    }

    /// Ortszeit `JJJJ-MM-TT HH:MM:SS`
    fn stamp(&self, at: DateTime<Utc>) -> String {
        at.with_timezone(&Local)
            .format("%Y-%m-%d %H:%M:%S")
            .to_string()
    }
}

/// Zeitstempel im Namen für die Sortierung (`JJJJMMTT-HHMMSS`); ohne Zeitstempel leer
fn stamp_of(pattern: &Regex, name: &str) -> String {
    pattern
        .captures(name)
        .map(|found| found[1].to_string())
        .unwrap_or_default()
}

/// Zeitstempel `JJJJMMTT-HHMMSS` (Ortszeit) als ISO-Zeit; ungültig → `None`
fn started_at(stamp: &str) -> Option<String> {
    let number = |from: usize, to: usize| stamp[from..to].parse::<u32>().ok();
    let date = NaiveDate::from_ymd_opt(number(0, 4)? as i32, number(4, 6)?, number(6, 8)?)?
        .and_hms_opt(number(9, 11)?, number(11, 13)?, number(13, 15)?)?;
    // Zur Sommerzeit kann eine Ortszeit zweimal vorkommen; JavaScript nimmt dann die frühere
    let local = Local
        .from_local_datetime(&date)
        .earliest()
        .or_else(|| Local.from_local_datetime(&date).latest())?;
    Some(iso(local.with_timezone(&Utc)))
}

/// Liest höchstens `limit` Bytes vom Ende der Datei; liefert die Startstelle und die Bytes
fn tail_bytes(path: &Path, limit: u64) -> Result<(u64, Vec<u8>)> {
    use std::io::{Read, Seek, SeekFrom};

    let mut file = File::open(path).map_err(Error::unexpected)?;
    let size = file.metadata().map_err(Error::unexpected)?.len();
    let start = size.saturating_sub(limit);
    file.seek(SeekFrom::Start(start))
        .map_err(Error::unexpected)?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).map_err(Error::unexpected)?;
    Ok((start, bytes))
}
