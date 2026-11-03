//! Die Verbrauchsproben der Accounts unter `~/.nexus/dispatch/account-usage.jsonl`: je beendetem
//! Umsetzungslauf eine JSON-Zeile mit der Auslastung des Accounts beim Start und am Ende.

use std::fs::{self, OpenOptions};
use std::io::{self, Write};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::{Error, Result};
use crate::registry::HarnessHome;

/// Auslastung eines Accounts zu einem Zeitpunkt; `None` = unbekannt
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "dispatch.ts")]
pub struct UsageSnapshot {
    /// Auslastung des 5-Stunden-Fensters in Prozent
    pub five_hour: Option<i64>,
    /// Auslastung der Woche in Prozent
    pub seven_day: Option<i64>,
    /// Rücksetzung des 5-Stunden-Fensters
    pub five_hour_resets_at: Option<String>,
    /// Rücksetzung der Woche
    pub seven_day_resets_at: Option<String>,
}

/// Ein beendeter Umsetzungslauf mit der Auslastung seines Accounts beim Start und am Ende
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageSample {
    pub account: String,
    pub profile: String,
    pub model: Option<String>,
    pub effort: Option<String>,
    pub started_at: String,
    pub ended_at: String,
    /// Auslastung beim Start des Laufs
    pub before: UsageSnapshot,
    /// Auslastung am Ende des Laufs
    pub after: UsageSnapshot,
}

/// Liest und schreibt die Verbrauchsproben (`dispatch/account-usage.jsonl`).
#[derive(Debug, Clone)]
pub struct UsageHistory {
    home: HarnessHome,
}

impl UsageHistory {
    /// Verlauf im angegebenen Benutzerordner
    pub fn new(home: HarnessHome) -> Self {
        Self { home }
    }

    /// Hängt eine Probe als eine JSON-Zeile an (legt Ordner und Datei an)
    pub fn append(&self, sample: &UsageSample) -> Result<()> {
        fs::create_dir_all(self.home.dispatch_dir()).map_err(Error::unexpected)?;
        let mut line = serde_json::to_string(sample).map_err(Error::unexpected)?;
        line.push('\n');
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.home.account_usage_file())
            .map_err(Error::unexpected)?;
        file.write_all(line.as_bytes()).map_err(Error::unexpected)
    }

    /// Die letzten `limit` Proben, älteste zuerst; kaputte Zeilen werden übersprungen, eine
    /// fehlende Datei ergibt eine leere Liste
    pub fn recent(&self, limit: usize) -> Result<Vec<UsageSample>> {
        let text = match fs::read_to_string(self.home.account_usage_file()) {
            Ok(text) => text,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(Error::unexpected(error)),
        };
        let mut samples: Vec<UsageSample> = text
            .lines()
            .filter_map(|line| serde_json::from_str(line).ok())
            .collect();
        let skip = samples.len().saturating_sub(limit);
        samples.drain(..skip);
        Ok(samples)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::HarnessHome;
    use crate::test_support::make_temp_dir;
    use std::fs;

    fn sample(account: &str, ended_at: &str) -> UsageSample {
        UsageSample {
            account: account.to_string(),
            profile: "klein".to_string(),
            model: Some("sonnet".to_string()),
            effort: Some("high".to_string()),
            started_at: "2026-11-03T10:00:00.000Z".to_string(),
            ended_at: ended_at.to_string(),
            before: UsageSnapshot {
                five_hour: Some(10),
                seven_day: Some(20),
                five_hour_resets_at: Some("2026-11-03T14:00:00.000Z".to_string()),
                seven_day_resets_at: None,
            },
            after: UsageSnapshot {
                five_hour: Some(15),
                seven_day: Some(21),
                five_hour_resets_at: Some("2026-11-03T14:00:00.000Z".to_string()),
                seven_day_resets_at: None,
            },
        }
    }

    #[test]
    fn missing_file_is_empty() {
        let dir = make_temp_dir("nexus-usage-");
        let history = UsageHistory::new(HarnessHome::at(dir.path()));
        assert!(history.recent(10).unwrap().is_empty());
    }

    #[test]
    fn appends_and_reads_oldest_first() {
        let dir = make_temp_dir("nexus-usage-");
        let history = UsageHistory::new(HarnessHome::at(dir.path()));
        let samples = [
            sample("a", "2026-11-03T11:00:00.000Z"),
            sample("b", "2026-11-03T12:00:00.000Z"),
            sample("c", "2026-11-03T13:00:00.000Z"),
        ];
        for s in &samples {
            history.append(s).unwrap();
        }
        assert_eq!(history.recent(10).unwrap(), samples.to_vec());
        assert_eq!(history.recent(2).unwrap(), samples[1..].to_vec());
        assert!(history.recent(0).unwrap().is_empty());
    }

    #[test]
    fn skips_broken_lines() {
        let dir = make_temp_dir("nexus-usage-");
        let home = HarnessHome::at(dir.path());
        let history = UsageHistory::new(home.clone());
        history
            .append(&sample("a", "2026-11-03T11:00:00.000Z"))
            .unwrap();
        let file = home.account_usage_file();
        let mut text = fs::read_to_string(&file).unwrap();
        text.push_str("kaputt\n");
        fs::write(&file, text).unwrap();
        history
            .append(&sample("b", "2026-11-03T12:00:00.000Z"))
            .unwrap();
        let recent = history.recent(10).unwrap();
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].account, "a");
        assert_eq!(recent[1].account, "b");
    }
}
