//! Setzt die Kopf- und Laufdaten einer Session zusammen: Session aus der Liste, Kennzahlen
//! aus dem Transcript, Laufdaten des Dispatchers. Dazu der Stand des Transcripts, der beim
//! Wachsen der Datei nur das Neue liest.

use std::path::Path;

use serde_json::Value;

use super::jsonl;
use super::transcript_changes::TranscriptChanges;
use super::transcript_facts::{FactsCollector, TranscriptFacts};
use super::types::{RunInfo, SessionDetail, TranscriptInfo};
use crate::sessions::dto::SessionDto;
use crate::sessions::session::SessionClient;

/// Quelle der Session: Lauf des Dispatchers, sonst Hook, sonst Transcript, sonst unbekannt
pub fn resolve_client(session: &SessionDto, facts: Option<&TranscriptFacts>) -> SessionClient {
    if session.run_name.is_some() {
        return SessionClient::Nexus;
    }
    if session.client != SessionClient::Unknown {
        return session.client;
    }
    facts
        .and_then(|facts| facts.client)
        .unwrap_or(SessionClient::Unknown)
}

/// Art des Laufs aus dem Namen des Protokolls (`T-1-20261104-091500[-merge].log`)
pub fn run_kind_of(name: &str) -> String {
    let stem = name.strip_suffix(".log").unwrap_or(name);
    for kind in ["merge", "research", "plan"] {
        if stem.ends_with(&format!("-{kind}")) {
            return kind.to_string();
        }
    }
    "work".to_string()
}

/// Die Teile der Detailansicht
pub struct SessionDetailParts {
    pub session: SessionDto,
    pub transcript_path: Option<String>,
    pub transcript_size: Option<i64>,
    pub facts: Option<TranscriptFacts>,
    pub run: Option<RunInfo>,
}

impl SessionDetailParts {
    /// Die Detailansicht
    pub fn build(self) -> SessionDetail {
        let client = resolve_client(&self.session, self.facts.as_ref());
        let facts = self.facts.unwrap_or_default();
        // Was der Hook nicht gemeldet hat, ergänzt das Transcript
        let mut session = self.session;
        if session.model.is_none() {
            session.model = facts.model.clone();
        }
        if session.title.is_none() {
            session.title = facts.title.clone();
        }
        SessionDetail {
            client,
            transcript: TranscriptInfo {
                available: self.transcript_path.is_some(),
                path: self.transcript_path,
                size_bytes: self.transcript_size,
            },
            branch: facts.branch,
            claude_version: facts.version,
            tokens: facts.tokens,
            cost_usd: facts.cost_usd,
            duration_ms: facts.duration_ms,
            context: facts.context,
            todos: facts.todos,
            run: self.run,
            session,
        }
    }
}

/// Stand einer Transcript-Datei: Kennzahlen und Änderungen bis zum Offset
#[derive(Debug, Clone)]
pub struct TranscriptScan {
    cwd: String,
    offset: u64,
    facts: FactsCollector,
    changes: TranscriptChanges,
}

impl TranscriptScan {
    /// Leerer Stand für eine Session im Arbeitsordner `cwd`
    pub fn new(cwd: &str) -> Self {
        Self {
            cwd: cwd.to_string(),
            offset: 0,
            facts: FactsCollector::new(),
            changes: TranscriptChanges::new(cwd),
        }
    }

    /// Liest das Neue der Datei; ist sie kürzer als bisher gelesen, von vorn
    pub fn update(&mut self, path: &Path) -> std::io::Result<()> {
        let len = std::fs::metadata(path)?.len();
        if len < self.offset {
            *self = TranscriptScan::new(&self.cwd);
        }
        let bytes = jsonl::read_from(path, self.offset)?;
        self.feed(&bytes);
        Ok(())
    }

    /// Verarbeitet die vollständigen Zeilen von `bytes` (ab dem bisherigen Offset)
    pub fn feed(&mut self, bytes: &[u8]) {
        for line in jsonl::lines(bytes, self.offset) {
            if let Ok(value) = serde_json::from_str::<Value>(line.text) {
                self.facts.feed(&value);
                self.changes.feed(&value);
            }
        }
        self.offset = jsonl::complete_end(bytes, self.offset);
    }

    /// Gelesen bis hier
    pub fn offset(&self) -> u64 {
        self.offset
    }

    /// Die Kennzahlen
    pub fn facts(&self) -> TranscriptFacts {
        self.facts.facts()
    }

    /// Die Änderungen
    pub fn changes(&self) -> &TranscriptChanges {
        &self.changes
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::super::fixtures;
    use super::*;
    use crate::sessions::session::{DisplayState, Session, SessionState, SessionView};

    fn dto(client: Option<SessionClient>, run: Option<&str>) -> SessionDto {
        SessionDto::of(
            &SessionView {
                session: Session {
                    id: "s1".into(),
                    project_id: "p".into(),
                    tickets: Vec::new(),
                    cwd: "D:/wt".into(),
                    state: SessionState::Idle,
                    started_at: "t".into(),
                    last_event_at: "t".into(),
                    ended_at: None,
                    title: None,
                    model: None,
                    subagents: Vec::new(),
                    transcript_path: None,
                    client,
                    run_name: run.map(str::to_string),
                },
                display_state: DisplayState::Idle,
                listening: false,
                runtime_ms: 0,
            },
            &HashMap::new(),
        )
    }

    #[test]
    fn quelle_lauf_vor_hook_vor_transcript() {
        let facts = TranscriptFacts {
            client: Some(SessionClient::ClaudeDesktop),
            ..TranscriptFacts::default()
        };
        assert_eq!(
            resolve_client(
                &dto(Some(SessionClient::ClaudeCode), Some("T-1.log")),
                Some(&facts)
            ),
            SessionClient::Nexus
        );
        assert_eq!(
            resolve_client(&dto(Some(SessionClient::ClaudeCode), None), Some(&facts)),
            SessionClient::ClaudeCode
        );
        assert_eq!(
            resolve_client(&dto(None, None), Some(&facts)),
            SessionClient::ClaudeDesktop
        );
        assert_eq!(
            resolve_client(&dto(None, None), None),
            SessionClient::Unknown
        );
    }

    #[test]
    fn ergaenzt_modell_und_titel_aus_dem_transcript() {
        let detail = SessionDetailParts {
            session: dto(None, None),
            transcript_path: None,
            transcript_size: None,
            facts: Some(TranscriptFacts {
                model: Some("claude-opus-5-5".into()),
                title: Some("Titel".into()),
                ..TranscriptFacts::default()
            }),
            run: None,
        }
        .build();
        assert_eq!(detail.session.model.as_deref(), Some("claude-opus-5-5"));
        assert_eq!(detail.session.title.as_deref(), Some("Titel"));
    }

    #[test]
    fn art_des_laufs_aus_dem_namen() {
        assert_eq!(run_kind_of("ARC-0001-20261104-091500.log"), "work");
        assert_eq!(run_kind_of("ARC-0001-20261104-091500-merge.log"), "merge");
        assert_eq!(run_kind_of("ARC-0001-20261104-091500-plan.log"), "plan");
        assert_eq!(run_kind_of("ARC-0001-20261104-091500-research"), "research");
    }

    #[test]
    fn ohne_transcript_ist_die_ansicht_leer_aber_vollstaendig() {
        let detail = SessionDetailParts {
            session: dto(None, None),
            transcript_path: None,
            transcript_size: None,
            facts: None,
            run: None,
        }
        .build();
        let json = serde_json::to_value(&detail).unwrap();
        assert_eq!(json["client"], "unknown");
        assert_eq!(
            json["transcript"],
            serde_json::json!({ "available": false, "path": null, "sizeBytes": null })
        );
        assert_eq!(
            json["tokens"],
            serde_json::json!({ "input": 0, "output": 0, "cacheRead": 0, "cacheWrite": 0 })
        );
        assert_eq!(json["context"], serde_json::Value::Null);
        assert_eq!(json["run"], serde_json::Value::Null);
        assert_eq!(json["session"]["id"], "s1");
    }

    #[test]
    fn liest_eine_wachsende_datei_nur_im_neuen_teil() {
        let dir = crate::test_support::make_temp_dir("scan-");
        let path = dir.path().join("s.jsonl");
        let content = fixtures::transcript();
        let middle = content.len() / 2;
        // Bis zur Mitte (meist mitten in einer Zeile)
        std::fs::write(&path, &content.as_bytes()[..middle]).unwrap();
        let mut scan = TranscriptScan::new(fixtures::CWD);
        scan.update(&path).unwrap();
        assert!(scan.offset() <= middle as u64);
        std::fs::write(&path, &content).unwrap();
        scan.update(&path).unwrap();
        assert_eq!(scan.offset(), content.len() as u64);
        assert_eq!(scan.facts().todos.len(), 3);
        assert_eq!(scan.changes().files().len(), 2);
        // Kürzer als gelesen: von vorn
        std::fs::write(
            &path,
            fixtures::line(serde_json::json!({ "type": "custom-title", "customTitle": "Neu" })),
        )
        .unwrap();
        scan.update(&path).unwrap();
        assert_eq!(scan.facts().title.as_deref(), Some("Neu"));
        assert!(scan.facts().todos.is_empty());
    }
}
