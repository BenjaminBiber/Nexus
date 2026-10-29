//! Testumgebung für Start und Auswertung eines Durchlaufs; Vorbild
//! `packages/server/tests/helpers/DispatchFixture.ts` und `FakeLauncher.ts`.
//!
//! Anders als dort gibt es hier noch keine Tick-Schleife (die kommt in P4-14c): die Tests
//! rufen [`DispatchFixture::start`] und [`DispatchFixture::evaluate`] selbst auf – genau die
//! zwei Schritte, die der Tick je Ticket tut.

#![allow(dead_code)]

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use nexus_core::board::basics::{BoardActor, TicketStatus};
use nexus_core::board::dispatch_run_types::DispatchRun;
use nexus_core::board::ticket_service::{
    NewTicketInput, TicketDetail, TicketPatch, TicketView, WriteOptions,
};
use nexus_core::board::types::Board;
use nexus_core::clock::{iso, Clock};
use nexus_core::dispatch::{
    DispatchConfig, DispatchConfigNormalizer, DispatchLog, DispatchStateStore, ReviewStore, RunEnd,
    RunRecord, RunResult, RunStore,
};
use nexus_core::{HarnessError, HarnessHome, Result};
use nexus_server::board_runtime::BoardBackend;
use nexus_server::dispatch::context::{DispatchContext, DispatchStores, RepoWork};
use nexus_server::dispatch::notifier::{DispatchNotice, DispatchNotify};
use nexus_server::dispatch::result_handler::ResultHandler;
use nexus_server::dispatch::run_recorder::{HandleResult, RunRecorder};
use nexus_server::dispatch::ticket_starter::{StartOutcome, TicketStarter};
use nexus_server::dispatch::worker_launcher::WorkerLauncher;
use serde_json::{json, Value};

use crate::common::{BoardBackendWithServices, TempRepo};

/// Ergebnis-Ereignis eines gelungenen Laufs (wie `OK_OUTPUT` in TypeScript)
pub const OK_OUTPUT: &str =
    r#"{"type":"result","subtype":"success","is_error":false,"result":"fertig"}"#;

/// Startet nichts, merkt sich nur die Aufrufe; Vorbild `FakeLauncher.ts`
#[derive(Debug, Default)]
pub struct FakeLauncher {
    launched: Mutex<Vec<String>>,
    /// Fehlermeldung, mit der der Start scheitert (wie `launcher.fail` in TypeScript)
    fail: Mutex<Option<String>>,
    next_pid: AtomicI64,
}

impl FakeLauncher {
    /// Starter, der immer eine Prozessnummer liefert
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            next_pid: AtomicI64::new(4242),
            ..Self::default()
        })
    }

    /// Lässt jeden weiteren Start mit dieser Meldung scheitern
    pub fn fail_with(&self, message: &str) {
        *self.fail.lock().expect("Attrappe") = Some(message.to_string());
    }

    /// Läufe, die gestartet wurden
    pub fn launched(&self) -> Vec<String> {
        self.launched.lock().expect("Attrappe").clone()
    }
}

impl WorkerLauncher for FakeLauncher {
    fn launch(&self, run_id: &str, _env: &BTreeMap<String, String>) -> Result<Option<u32>> {
        if let Some(message) = self.fail.lock().expect("Attrappe").clone() {
            return Err(HarnessError::new("worker-start-failed", message).into());
        }
        self.launched
            .lock()
            .expect("Attrappe")
            .push(run_id.to_string());
        Ok(Some(self.next_pid.fetch_add(1, Ordering::SeqCst) as u32))
    }
}

/// Sammelt die Meldungen, statt sie zu schicken
#[derive(Debug, Default)]
pub struct CollectedNotices(Mutex<Vec<DispatchNotice>>);

impl CollectedNotices {
    /// Sammler ohne Meldungen
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// Alle gesammelten Meldungen
    pub fn all(&self) -> Vec<DispatchNotice> {
        self.0.lock().expect("Meldungen").clone()
    }

    /// Die jüngste Meldung
    pub fn last(&self) -> Option<DispatchNotice> {
        self.0.lock().expect("Meldungen").last().cloned()
    }
}

#[async_trait]
impl DispatchNotify for CollectedNotices {
    async fn notify(&self, notice: &DispatchNotice) -> Result<()> {
        self.0.lock().expect("Meldungen").push(notice.clone());
        Ok(())
    }
}

/// Alles, was Start und Auswertung eines Tickets brauchen: eigener Benutzerordner, Git-Repo als
/// Board-Repo, Board im Speicher, Attrappe statt Worker.
pub struct DispatchFixture {
    _dir: tempfile::TempDir,
    pub home: HarnessHome,
    pub repo: TempRepo,
    pub backend: Arc<BoardBackendWithServices>,
    pub board: Board,
    pub launcher: Arc<FakeLauncher>,
    pub notices: Arc<CollectedNotices>,
    pub ctx: DispatchContext,
    pub cfg: DispatchConfig,
    now: Arc<Mutex<DateTime<Utc>>>,
}

impl DispatchFixture {
    /// Legt alles an; die Einstellung ist die aus dem Beispiel, eingeschaltet und ohne Freigabe
    /// vor dem Merge – `patch` überschreibt einzelne Felder der rohen Einstellung
    pub async fn create(patch: Value) -> Self {
        let dir = tempfile::Builder::new()
            .prefix("nexus-dispatch-")
            .tempdir()
            .expect("Ordner");
        let home = HarnessHome::at(dir.path());
        let repo = TempRepo::new();
        repo.run_git(&["config", "user.name", "t"]);
        repo.run_git(&["config", "user.email", "t@t"]);
        let backend = BoardBackendWithServices::create().await;
        let actor = backend.actor();
        backend
            .services
            .projects
            .create("ARC", "Arches", &actor)
            .await
            .expect("Projekt");
        let board = backend
            .services
            .boards
            .create("ARC", "Sprint", &actor)
            .await
            .expect("Board");
        let board = backend
            .services
            .boards
            .set_repo_path(board.board.id, Some(&repo.path().to_string_lossy()))
            .await
            .expect("Repo-Pfad")
            .board;
        let cfg = config(patch);
        let now = Arc::new(Mutex::new(Utc::now()));
        let clock = {
            let now = now.clone();
            Clock::from_fn(move || *now.lock().expect("Uhr"))
        };
        let launcher = FakeLauncher::new();
        let notices = CollectedNotices::new();
        let stores = DispatchStores {
            runs: RunStore::new(home.clone()),
            state: DispatchStateStore::with_clock(home.clone(), clock.clone()),
            log: DispatchLog::with_clock(home.clone(), clock.clone()),
            reviews: ReviewStore::new(home.clone()),
        };
        let me = nexus_core::board::repo_board_resolver::RepoBoardOwner {
            user_id: actor.user_id.clone(),
            active_board: Some(board.id),
        };
        let ctx = DispatchContext::new(
            cfg.clone(),
            me,
            home.clone(),
            backend.clone() as Arc<dyn BoardBackend>,
            stores,
            launcher.clone(),
            clock,
            notices.clone(),
            BTreeMap::new(),
            // Die Tests laufen auf jeder Plattform gegen denselben Zweig: ohne Windows keine
            // Suche nach der Git Bash
            "linux",
        );
        Self {
            _dir: dir,
            home,
            repo,
            backend,
            board,
            launcher,
            notices,
            ctx,
            cfg,
            now,
        }
    }

    /// Der Handelnde des Testbenutzers
    pub fn actor(&self) -> BoardActor {
        self.backend.actor()
    }

    /// Handelnder als Junior-Agent des Tickets
    pub fn agent(&self, ticket_id: &str) -> BoardActor {
        BoardActor {
            user_id: self.backend.actor().user_id,
            name: format!("junior-{ticket_id}"),
        }
    }

    /// Legt ein bereites Ticket an (Geplant, Label `auto`, Plan, Handbuch, Abnahme)
    pub async fn ready(&self, title: &str, patch: TicketPatch) -> String {
        let actor = self.actor();
        let created = self
            .backend
            .services
            .tickets
            .create(
                self.board.id,
                &NewTicketInput {
                    acceptance: Some("Tests grün".into()),
                    labels: Some(vec!["auto".into()]),
                    needs_planning: Some(false),
                    ..NewTicketInput::titled(title)
                },
                &actor,
                &WriteOptions::default(),
            )
            .await
            .expect("Ticket anlegen");
        self.backend
            .services
            .tickets
            .update(
                &created.ticket.id,
                &TicketPatch {
                    plan: Some("## Plan\nUmsetzen".into()),
                    handbook: Some("npm test".into()),
                    status: Some(TicketStatus::Planned),
                    ..patch
                },
                &actor,
                &WriteOptions::default(),
            )
            .await
            .expect("Ticket vorbereiten");
        created.ticket.id
    }

    /// Das Board samt Repo und Worktree-Verwaltung
    pub async fn work(&self) -> Arc<RepoWork> {
        self.ctx.repo(&self.board).await.expect("Repo des Boards")
    }

    /// Sicht auf ein Ticket, wie der Tick sie hat
    pub async fn view(&self, ticket_id: &str) -> TicketView {
        self.detail(ticket_id).await.view
    }

    /// Das Ticket selbst (Deref auf `TicketDetail` lässt es nicht herausnehmen)
    pub async fn ticket(&self, ticket_id: &str) -> nexus_core::board::Ticket {
        self.detail(ticket_id).await.view.ticket
    }

    /// Legt eine offene Frage zum Ticket an und liefert ihre Kennung
    pub async fn question(&self, ticket_id: &str, title: &str) -> String {
        let detail = self
            .backend
            .services
            .questions
            .create(
                "ARC",
                &nexus_core::board::question_service::NewBoardQuestionInput {
                    ticket_ids: Some(vec![ticket_id.to_string()]),
                    ..nexus_core::board::question_service::NewBoardQuestionInput::titled(title)
                },
                &nexus_core::board::question_types::BoardQuestionActor {
                    actor: self.agent(ticket_id),
                    kind: nexus_core::actors::AuthorKind::Session,
                    session: None,
                },
            )
            .await
            .expect("Frage anlegen");
        detail.view.question.id
    }

    /// Ticket mit Kommentaren
    pub async fn detail(&self, ticket_id: &str) -> TicketDetail {
        self.backend
            .services
            .tickets
            .get(ticket_id)
            .await
            .expect("Ticket")
    }

    /// Startet ein Ticket wie der Tick: frisch lesen, prüfen, claim, Worktree, Lauf
    pub async fn start(&self, ticket_id: &str) -> Result<StartOutcome> {
        let view = self.view(ticket_id).await;
        let work = self.work().await;
        let results = ResultHandler::without_gates(&self.ctx);
        TicketStarter::new(&self.ctx, &results)
            .start(&view, &work)
            .await
    }

    /// Jüngster Lauf zum Ticket
    pub fn run(&self, ticket_id: &str) -> RunRecord {
        self.ctx
            .stores
            .runs
            .list()
            .expect("Läufe")
            .into_iter()
            .rfind(|run| run.ticket_id == ticket_id)
            .unwrap_or_else(|| panic!("kein Lauf für {ticket_id}"))
    }

    /// Schreibt das Ergebnis des jüngsten Laufs, als hätte der Worker es geschrieben
    pub fn end(&self, ticket_id: &str, output: &str, code: Option<i64>) -> RunRecord {
        self.end_with(
            ticket_id,
            RunResult {
                code,
                output: output.to_string(),
                ..RunResult::default()
            },
        )
    }

    /// Wie [`DispatchFixture::end`], aber mit einem selbst gebauten Ergebnis
    pub fn end_with(&self, ticket_id: &str, result: RunResult) -> RunRecord {
        let record = self.run(ticket_id);
        self.ctx
            .stores
            .runs
            .finish(
                &record.id,
                &RunEnd {
                    result,
                    ended_at: self.ctx.now_iso(),
                },
            )
            .expect("Ergebnis schreiben");
        record
    }

    /// Wertet den jüngsten beendeten Lauf aus und merkt ihn für die Auswertung vor – die zwei
    /// Schritte, die der Tick nach dem Ende eines Laufs tut
    pub async fn evaluate(&self, ticket_id: &str) -> Result<HandleResult> {
        let record = self.run(ticket_id);
        let end = self
            .ctx
            .stores
            .runs
            .result(&record.id)?
            .expect("Ergebnis des Laufs");
        let results = ResultHandler::without_gates(&self.ctx);
        let result = results.handle(&record, &end, None).await?;
        RunRecorder::new(&self.ctx)
            .keep(&record, &end, &result)
            .await;
        // Der ausgewertete Lauf ist erledigt; der Tick entfernt ihn hier
        self.ctx.stores.runs.remove(&record.id)?;
        Ok(result)
    }

    /// Überträgt die vorgemerkten Lauf-Zeilen in die Datenbank
    pub async fn flush(&self) -> Result<usize> {
        RunRecorder::new(&self.ctx).flush().await
    }

    /// Alle erfassten Läufe, nach Versuch sortiert
    pub async fn rows(&self) -> Vec<DispatchRun> {
        let mut rows = self
            .backend
            .services
            .dispatch_runs
            .list_all(
                "2000-01-01T00:00:00.000Z".parse().expect("von"),
                "2100-01-01T00:00:00.000Z".parse().expect("bis"),
            )
            .await
            .expect("Läufe");
        rows.sort_by_key(|row| row.attempt.unwrap_or(0));
        rows
    }

    /// Texte aller Kommentare des Tickets
    pub async fn comments(&self, ticket_id: &str) -> Vec<String> {
        self.detail(ticket_id)
            .await
            .comments
            .into_iter()
            .map(|comment| comment.body)
            .collect()
    }

    /// Vorgemerkte Lauf-Zeilen
    pub fn kept(&self) -> Vec<(String, DispatchRun)> {
        self.ctx.stores.runs.kept_stats().expect("Vorgemerkte")
    }

    /// Stellt die Uhr des Dispatchers vor
    pub fn advance(&self, minutes: i64) {
        let mut now = self.now.lock().expect("Uhr");
        *now += chrono::Duration::minutes(minutes);
    }

    /// Die aktuelle Zeit des Dispatchers als ISO-Text
    pub fn now_iso(&self) -> String {
        iso(*self.now.lock().expect("Uhr"))
    }
}

/// Die Beispiel-Einstellung, eingeschaltet, ohne Freigabe vor dem Merge und mit `patch`
pub fn config(patch: Value) -> DispatchConfig {
    let normalizer = DispatchConfigNormalizer::new();
    let mut raw = normalizer.example();
    let object = raw.as_object_mut().expect("Einstellung als Objekt");
    object.insert("enabled".into(), json!(true));
    object.insert("reviewBeforeMerge".into(), json!(false));
    if let Some(fields) = patch.as_object() {
        for (key, value) in fields {
            object.insert(key.clone(), value.clone());
        }
    }
    normalizer.normalize(&raw).expect("Einstellung")
}
