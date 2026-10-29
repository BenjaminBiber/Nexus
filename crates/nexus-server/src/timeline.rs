//! Die Zeitleiste eines Tickets; portiert aus
//! `packages/server/src/dbboard/TicketTimelineAssembler.ts`.
//!
//! Der Kern kommt aus der Board-Datenbank ([`nexus_core::board::TicketTimelineService`]);
//! dazu kommen zwei rechnerlokale Quellen: die Lauf-Protokolle des Dispatchers
//! (`<home>/dispatch-logs`) und die Commits des Ticket-Branches im Repo des Boards. Beide
//! sind nur auf dem Rechner lesbar, auf dem gearbeitet wurde – die Antwort sagt je Quelle, ob
//! sie gelesen werden konnte.

use std::path::Path;
use std::sync::Arc;

use nexus_core::board::ticket_timeline::{
    sort_timeline, TimelineAgentNames, TimelineEvent, TimelineKind, TimelineRef,
};
use nexus_core::board::timeline_classifier::TimelineClassifier;
use nexus_core::dispatch::config_store::DispatchConfigStore;
use nexus_core::dispatch::invocation::RunKind;
use nexus_core::dispatch::state::log::DispatchLog;
use nexus_core::dispatch::state::log_outcome::{RunLogOutcome, RunState};
use nexus_core::git::branch_commit_reader::{
    BranchCommitReader, BranchCommitSource, BranchCommitsStatus,
};
use nexus_core::{Error, Result};
use serde::Serialize;

use crate::state::AppState;

/// Ob eine rechnerlokale Quelle gelesen werden konnte
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SourceState {
    Ok,
    Unavailable,
    Error,
}

/// Je lokaler Quelle, ob sie auf diesem Rechner lesbar war
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelineSources {
    pub runs: SourceState,
    pub git: SourceState,
    /// Nur, wenn Git unerwartet scheiterte
    #[serde(skip_serializing_if = "Option::is_none")]
    pub git_message: Option<String>,
}

/// Antwort der Route: Ereignisse und der Zustand der lokalen Quellen
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TicketTimelineDto {
    pub ticket_id: String,
    pub events: Vec<TimelineEvent>,
    pub sources: TimelineSources,
}

/// Benennung der Lauf-Arten in den Texten der Ereignisse
fn run_kind_label(kind: RunKind) -> &'static str {
    match kind {
        RunKind::Work => "Umsetzung",
        RunKind::Merge => "Merge-Session",
        RunKind::Research => "Recherche",
        RunKind::Plan => "Planung",
    }
}

/// Setzt die Zeitleiste eines Tickets zusammen.
pub struct TicketTimelineAssembler {
    state: Arc<AppState>,
}

impl TicketTimelineAssembler {
    /// Assembler über dem Zustand des Servers
    pub fn new(state: Arc<AppState>) -> Self {
        Self { state }
    }

    /// Die Zeitleiste eines Tickets; eine unbekannte ID ergibt `ticket-not-found`
    pub async fn get(&self, id: &str) -> Result<TicketTimelineDto> {
        let names = self.names().await;
        let classifier = TimelineClassifier::new(names.clone());
        let id = id.to_string();
        let (timeline, ticket, repo_path) = {
            let names = names.clone();
            self.state
                .board
                .run(|b| async move {
                    let timeline = b.services().timeline(names).get(&id).await?;
                    let ticket = b.services().tickets.get(&timeline.ticket_id).await?;
                    let board = b.services().boards.find(ticket.board_id).await?;
                    let repo_path = board.and_then(|board| board.board.repo_path);
                    Ok((timeline, ticket, repo_path))
                })
                .await?
        };
        let runs = self
            .run_events(&timeline.ticket_id, names.as_ref(), &ticket.created_at)
            .await?;
        // Der letzte Merge-Commit aus den Ereignissen der Datenbank zeigt, wo der Branch
        // gelandet ist; danach gibt es ihn oft nicht mehr
        let merge_commit = timeline
            .events
            .iter()
            .filter(|event| event.event_type == "merge")
            .filter_map(|event| event.reference.as_ref()?.commit.clone())
            .next_back();
        let branch = ticket.branch.clone();
        let target_branch = ticket.target_branch.clone();
        // Git liest Prozesse und Dateien und gehört deshalb nicht in die Laufzeit der Anfrage
        let git = tokio::task::spawn_blocking(move || {
            let source = BranchCommitSource {
                branch: branch.as_deref(),
                merge_commit: merge_commit.as_deref(),
                target_branch: target_branch.as_deref(),
            };
            BranchCommitReader::new().read(repo_path.as_deref().map(Path::new), &source)
        })
        .await
        .map_err(Error::unexpected)?;
        let commits = git.commits.iter().map(|commit| TimelineEvent {
            at: commit.at.clone(),
            kind: TimelineKind::Git,
            event_type: "commit".into(),
            actor: classifier.actor(&commit.author, None),
            field: None,
            from: None,
            to: None,
            note: None,
            text: Some(commit.subject.clone()),
            reference: Some(TimelineRef {
                commit: Some(commit.hash.clone()),
                ..TimelineRef::default()
            }),
        });
        let git_status = match git.status {
            BranchCommitsStatus::Ok => SourceState::Ok,
            BranchCommitsStatus::Unavailable => SourceState::Unavailable,
            BranchCommitsStatus::Error => SourceState::Error,
        };
        let sources = TimelineSources {
            runs: if runs.is_some() {
                SourceState::Ok
            } else {
                SourceState::Unavailable
            },
            git: git_status,
            git_message: git
                .message
                .filter(|_| git.status != BranchCommitsStatus::Ok),
        };
        let mut events = timeline.events;
        events.extend(runs.unwrap_or_default());
        events.extend(commits);
        sort_timeline(&mut events);
        Ok(TicketTimelineDto {
            ticket_id: timeline.ticket_id,
            events,
            sources,
        })
    }

    /// Start und Ende je Lauf-Protokoll; `None`, wenn es auf diesem Rechner keinen
    /// Protokollordner gibt. Läufe von vor der Anlage des Tickets gehören zu einem früheren
    /// Ticket mit derselben ID und fallen weg (der Name trägt nur ganze Sekunden)
    async fn run_events(
        &self,
        ticket_id: &str,
        names: Option<&TimelineAgentNames>,
        created_at: &str,
    ) -> Result<Option<Vec<TimelineEvent>>> {
        let home = self.state.home.clone();
        let ticket_id = ticket_id.to_string();
        let dispatcher = names
            .map(|names| names.dispatcher_name.clone())
            .unwrap_or_else(|| "dispatcher".into());
        let prefix = names
            .map(|names| names.agent_prefix.clone())
            .unwrap_or_else(|| "junior-".into());
        let created = millis(created_at).map(|at| at / 1000 * 1000);
        // Die Protokolle liegen als Dateien im Benutzerordner: blockierendes Lesen
        tokio::task::spawn_blocking(move || {
            if !home.dispatch_logs_dir().exists() {
                return Ok(None);
            }
            let logs = DispatchLog::new(home);
            let outcome = RunLogOutcome::new();
            let mut events = Vec::new();
            for run in logs.runs(&ticket_id)? {
                let Some(started) = run.started.clone() else {
                    continue;
                };
                if let (Some(started_at), Some(created)) = (millis(&started), created) {
                    if started_at < created {
                        continue;
                    }
                }
                let read = logs.read(&run.name).unwrap_or_default();
                let outcome = outcome.read(&read);
                let actor = if run.kind == RunKind::Work {
                    classified_agent(format!("{prefix}{ticket_id}"))
                } else {
                    classified_agent(dispatcher.clone())
                };
                let label = run_kind_label(run.kind);
                let result = if outcome.state == RunState::Running {
                    if outcome.timed_out {
                        "Zeitlimit erreicht".to_string()
                    } else {
                        "läuft oder ohne Ende im Protokoll".to_string()
                    }
                } else {
                    let code = outcome
                        .exit_code
                        .map(|code| code.to_string())
                        .unwrap_or_else(|| "–".into());
                    let limit = if outcome.timed_out { ", Zeitlimit" } else { "" };
                    format!("Exit-Code {code}{limit}")
                };
                let reference = TimelineRef {
                    run_log: Some(run.name.clone()),
                    exit_code: Some(outcome.exit_code.map(|code| code as i32)),
                    ..TimelineRef::default()
                };
                let start_text = if outcome.state == RunState::Running {
                    format!("{label} gestartet ({result})")
                } else {
                    format!("{label} gestartet")
                };
                events.push(TimelineEvent {
                    at: started,
                    kind: TimelineKind::Run,
                    event_type: "run-log".into(),
                    actor: actor.clone(),
                    field: None,
                    from: None,
                    to: None,
                    note: None,
                    text: Some(start_text),
                    reference: Some(reference.clone()),
                });
                if let Some(ended_at) = outcome.ended_at {
                    events.push(TimelineEvent {
                        at: ended_at,
                        kind: TimelineKind::Run,
                        event_type: "run-end".into(),
                        actor,
                        field: None,
                        from: None,
                        to: None,
                        note: None,
                        text: Some(format!("{label} beendet: {result}")),
                        reference: Some(reference),
                    });
                }
            }
            Ok(Some(events))
        })
        .await
        .map_err(Error::unexpected)?
    }

    /// Namen aus der Dispatch-Konfiguration; ohne lesbare Datei die Standardwerte, bei Fehler
    /// `None`
    async fn names(&self) -> Option<TimelineAgentNames> {
        let home = self.state.home.clone();
        tokio::task::spawn_blocking(move || {
            let read = DispatchConfigStore::new(home).read().ok()?;
            Some(TimelineAgentNames {
                dispatcher_name: read.config.dispatcher_name,
                agent_prefix: read.config.agent_prefix,
            })
        })
        .await
        .ok()
        .flatten()
    }
}

/// Ein Agent als Handelnder eines Lauf-Ereignisses
fn classified_agent(name: String) -> nexus_core::board::ticket_timeline::TimelineActor {
    nexus_core::board::ticket_timeline::TimelineActor {
        name,
        actor_type: nexus_core::board::ticket_timeline::TimelineActorType::Agent,
    }
}

/// Ein ISO-Zeitpunkt in Millisekunden seit 1970 (wie `Date.parse` in JavaScript)
fn millis(at: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(at)
        .ok()
        .map(|at| at.timestamp_millis())
}
