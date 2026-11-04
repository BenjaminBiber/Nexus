//! Planungsläufe an Tickets: Start von Hand, Auswertung und Automatik; portiert aus
//! `packages/server/tests/unit/dispatch/DispatchPlanning.test.ts` und
//! `DispatchPlanningAuto.test.ts`.
//!
//! Die Tick-Schleife gibt es in Rust noch nicht (sie kommt in P4-14c): die Tests rufen
//! [`PlanRunner::start`], [`PlanRunner::evaluate`] und [`PlanRunner::start_auto`] selbst auf –
//! genau die Schritte, die der Tick je Ticket tut. Pause, Zeitfenster, das WIP der Tickets und
//! die Statusanzeige hängen am Dienst und stehen deshalb in P4-14c bzw. P4-16a; `start_auto`
//! bekommt die freien WIP-Plätze von dort als Zahl.

mod common;
mod dispatch_support;

use std::path::Path;

use nexus_core::actors::AuthorKind;
use nexus_core::board::basics::{BoardActor, TicketStatus};
use nexus_core::board::planning_service::PlanInput;
use nexus_core::board::question_service::NewBoardQuestionInput;
use nexus_core::board::question_types::BoardQuestionActor;
use nexus_core::board::ticket_service::{NewTicketInput, TicketPatch, WriteOptions};
use nexus_core::dispatch::config::{planning_denied_tools, PLANNING_TOOLS};
use nexus_core::dispatch::{markers, RunKind, RunRecord};
use nexus_server::dispatch::notifier::DispatchNoticeKind;
use nexus_server::dispatch::plan_runner::{PlanOutcome, PlanRunner};
use nexus_server::dispatch::research_runner::StartMode;
use serde_json::json;

use dispatch_support::DispatchFixture;

/// Der Mensch, der die Fragen stellt (der Benutzer des Tests)
fn human(f: &DispatchFixture) -> BoardQuestionActor {
    BoardQuestionActor {
        actor: f.actor(),
        kind: AuthorKind::Human,
        session: None,
    }
}

/// Legt ein Ticket mit Planungsbedarf an und liefert die Kennung
async fn backlog(f: &DispatchFixture, title: &str) -> String {
    backlog_on(f, f.board.id, title).await
}

/// Wie [`backlog`], auf einem bestimmten Board
async fn backlog_on(f: &DispatchFixture, board_id: i32, title: &str) -> String {
    f.backend
        .services
        .tickets
        .create(
            board_id,
            &NewTicketInput {
                needs_planning: Some(true),
                ..NewTicketInput::titled(title)
            },
            &f.actor(),
            &WriteOptions::default(),
        )
        .await
        .expect("Ticket anlegen")
        .ticket
        .id
}

/// Speichert einen Plan, als hätte der Planungs-Agent `harness board plan-json` aufgerufen
async fn plan_as_agent(f: &DispatchFixture, id: &str) {
    f.backend
        .services
        .planning
        .plan(
            id,
            &PlanInput {
                plan: Some("## Plan\nUmsetzen\n\n## Aufwand\nklein – eine Datei".into()),
                handbook: Some("npm test".into()),
                diagram: Some("graph TD; A-->B".into()),
                ..PlanInput::default()
            },
            &BoardActor {
                user_id: f.actor().user_id,
                name: format!("planner-{id}"),
            },
        )
        .await
        .expect("Plan speichern");
}

/// Schaltet die Automatik ein, als hätte ein Tick den Zeitpunkt „ab jetzt“ gemerkt
async fn auto_on(f: &DispatchFixture) {
    pause().await;
    let since = nexus_core::clock::iso(chrono::Utc::now());
    f.ctx
        .stores
        .state
        .update(move |state| state.planning_auto.since = Some(since))
        .await
        .expect("Automatik einschalten");
    pause().await;
}

/// Wartet kurz, damit Zeitstempel davor und danach sicher verschieden sind
async fn pause() {
    tokio::time::sleep(std::time::Duration::from_millis(5)).await;
}

/// Wertet den Lauf aus, wie der Tick es nach seinem Ende tut
async fn evaluate(f: &DispatchFixture, record: &RunRecord, reason: Option<&str>) -> PlanOutcome {
    let end = f.result(&record.id);
    let outcome = PlanRunner::new(&f.ctx)
        .evaluate(record, &end, reason)
        .await
        .expect("Auswertung");
    f.drop_run(&record.id);
    outcome
}

#[tokio::test]
async fn startet_die_planung_im_losgelosten_worktree_eingeschrankt_mit_claim_und_harness_ticket() {
    let f = DispatchFixture::create(json!({
        "enabled": false,
        "claude": {
            "permissionMode": "bypassPermissions",
            "extraArgs": ["--dangerously-skip-permissions"],
            "allowedTools": ["Bash(*)"],
        },
    }))
    .await;
    let id = backlog(&f, "Rabatte").await;
    let record = PlanRunner::new(&f.ctx)
        .start(&id.to_lowercase(), StartMode::Manual)
        .await
        .expect("Planung starten");

    assert_eq!(record.kind, RunKind::Plan);
    assert_eq!(record.ticket_id, id);
    assert_eq!(record.title, "Rabatte");
    assert_eq!(record.agent, format!("planner-{id}"));
    assert_eq!(record.branch, "");
    assert_eq!(record.planned_before, Some(None));
    assert!(record.repo_root.is_some());
    assert!(
        record.cwd.ends_with(&format!("-worktrees/plan-{id}")),
        "{}",
        record.cwd
    );
    assert!(Path::new(&record.cwd).exists());
    assert!(record.log_file.ends_with(&format!("{}.log", record.id)));

    let mode = record
        .args
        .iter()
        .position(|arg| arg == "--permission-mode")
        .expect("Rechte-Modus");
    assert_eq!(record.args[mode + 1], "default");
    assert!(!record
        .args
        .iter()
        .any(|arg| arg == "--dangerously-skip-permissions"));
    let denied = record
        .args
        .iter()
        .position(|arg| arg == "--disallowedTools")
        .expect("Verbote");
    let allowed = record
        .args
        .iter()
        .position(|arg| arg == "--allowedTools")
        .expect("Erlaubte");
    assert_eq!(
        record.args[denied + 1..allowed],
        planning_denied_tools()
            .into_iter()
            .map(String::from)
            .collect::<Vec<_>>()[..]
    );
    assert_eq!(record.args[allowed + 1..], PLANNING_TOOLS.map(String::from));
    assert!(record.prompt.contains(&format!(
        "harness board plan-json {id} --file plan.json --by planner-{id}"
    )));
    assert!(record.prompt.contains("## Aufwand"));
    assert_eq!(
        f.launcher
            .env_of(&record.id)
            .get("HARNESS_TICKET")
            .map(String::as_str),
        Some(id.as_str())
    );
    assert_eq!(
        f.ticket(&id).await.planning_claim_by.as_deref(),
        Some(format!("planner-{id}").as_str())
    );

    let state = f.ctx.stores.state.read().expect("Zustand");
    let entry = state.planning.get(&id).expect("Eintrag des Tickets");
    assert_eq!(entry.count, 1);
    assert_eq!(entry.auto_at, None);
}

#[tokio::test]
async fn lehnt_ab_laufende_planung_falscher_status_offene_frage_und_board_ohne_repo() {
    let f = DispatchFixture::create(json!({})).await;
    let runner = PlanRunner::new(&f.ctx);
    let id = backlog(&f, "Rabatte").await;
    runner.start(&id, StartMode::Manual).await.expect("Start");
    assert_eq!(
        runner
            .start(&id, StartMode::Manual)
            .await
            .expect_err("zweiter Start")
            .code(),
        Some("plan-running")
    );

    let done = backlog(&f, "Fertig").await;
    f.backend
        .services
        .tickets
        .update(
            &done,
            &TicketPatch {
                status: Some(TicketStatus::InProgress),
                ..TicketPatch::default()
            },
            &f.actor(),
            &WriteOptions::default(),
        )
        .await
        .expect("Status setzen");
    assert_eq!(
        runner
            .start(&done, StartMode::Manual)
            .await
            .expect_err("falscher Status")
            .code(),
        Some("plan-not-possible")
    );

    let asked = backlog(&f, "Mit Frage").await;
    f.backend
        .services
        .questions
        .create(
            "ARC",
            &NewBoardQuestionInput {
                ticket_ids: Some(vec![asked.clone()]),
                ..NewBoardQuestionInput::titled("Wie?")
            },
            &human(&f),
        )
        .await
        .expect("Frage anlegen");
    assert_eq!(
        runner
            .start(&asked, StartMode::Manual)
            .await
            .expect_err("offene Frage")
            .code(),
        Some("plan-waiting-question")
    );

    let no_repo = f
        .backend
        .services
        .boards
        .create("ARC", "Ohne Repo", &f.actor())
        .await
        .expect("Board ohne Repo")
        .board;
    let elsewhere = backlog_on(&f, no_repo.id, "Woanders").await;
    assert_eq!(
        runner
            .start(&elsewhere, StartMode::Manual)
            .await
            .expect_err("Board ohne Repo")
            .code(),
        Some("plan-no-repo")
    );
    assert_eq!(f.ticket(&elsewhere).await.planning_claim_by, None);
}

#[tokio::test]
async fn plan_des_agents_gespeichert_planned_worktree_entfernt_kein_kommentar() {
    let f = DispatchFixture::create(json!({ "enabled": false })).await;
    let id = backlog(&f, "Rabatte").await;
    PlanRunner::new(&f.ctx)
        .start(&id, StartMode::Manual)
        .await
        .expect("Start");
    let record = f.run(&id);
    plan_as_agent(&f, &id).await;
    f.end(&id, dispatch_support::OK_OUTPUT, Some(0));

    assert_eq!(evaluate(&f, &record, None).await, PlanOutcome::Planned);
    assert!(!Path::new(&record.cwd).exists());
    let ticket = f.ticket(&id).await;
    assert_eq!(ticket.status, TicketStatus::Planned);
    assert!(!ticket.needs_planning);
    assert_eq!(ticket.planning_claim_by, None);
    assert_eq!(
        ticket.planned_by.as_deref(),
        Some(format!("planner-{id}").as_str())
    );
    assert!(f.comments(&id).await.is_empty());
}

#[tokio::test]
async fn neue_offene_frage_question_claim_frei_kein_kommentar() {
    let f = DispatchFixture::create(json!({ "enabled": false })).await;
    let id = backlog(&f, "Rabatte").await;
    PlanRunner::new(&f.ctx)
        .start(&id, StartMode::Manual)
        .await
        .expect("Start");
    let record = f.run(&id);
    f.backend
        .services
        .questions
        .create(
            "ARC",
            &NewBoardQuestionInput {
                ticket_ids: Some(vec![id.clone()]),
                ..NewBoardQuestionInput::titled("Brutto oder netto?")
            },
            &BoardQuestionActor {
                actor: BoardActor {
                    user_id: f.actor().user_id,
                    name: format!("planner-{id}"),
                },
                kind: AuthorKind::Session,
                session: None,
            },
        )
        .await
        .expect("Frage des Planers");
    f.end(&id, dispatch_support::OK_OUTPUT, Some(0));

    assert_eq!(evaluate(&f, &record, None).await, PlanOutcome::Question);
    assert_eq!(f.ticket(&id).await.planning_claim_by, None);
    assert!(f.comments(&id).await.is_empty());
}

#[tokio::test]
async fn ohne_plan_und_ohne_frage_kommentar_mit_protokoll_claim_frei_kein_doppelter_kommentar() {
    let f = DispatchFixture::create(json!({ "enabled": false })).await;
    let id = backlog(&f, "Rabatte").await;
    PlanRunner::new(&f.ctx)
        .start(&id, StartMode::Manual)
        .await
        .expect("Start");
    let record = f.run(&id);
    f.end(&id, dispatch_support::OK_OUTPUT, Some(0));
    let end = f.result(&record.id);

    let runner = PlanRunner::new(&f.ctx);
    assert_eq!(
        runner.evaluate(&record, &end, None).await.expect("erste"),
        PlanOutcome::Empty
    );
    let comments = f.comments(&id).await;
    assert_eq!(comments.len(), 1);
    assert!(comments[0].starts_with(markers::PLAN_EMPTY));
    assert!(comments[0].contains(&format!("{}.log", record.id)));
    let ticket = f.ticket(&id).await;
    assert_eq!(ticket.status, TicketStatus::Backlog);
    assert!(ticket.needs_planning);
    assert_eq!(ticket.planning_claim_by, None);
    assert!(!Path::new(&record.cwd).exists());

    // Dieselbe Auswertung noch einmal (Verbindungsfehler danach): kein zweiter Kommentar
    assert_eq!(
        runner.evaluate(&record, &end, None).await.expect("zweite"),
        PlanOutcome::Empty
    );
    assert_eq!(f.comments(&id).await.len(), 1);
    f.drop_run(&record.id);
}

#[tokio::test]
async fn nutzungslimit_pause_und_kommentar() {
    let f = DispatchFixture::create(json!({ "enabled": false })).await;
    let id = backlog(&f, "Rabatte").await;
    PlanRunner::new(&f.ctx)
        .start(&id, StartMode::Manual)
        .await
        .expect("Start");
    let record = f.run(&id);
    f.end(&id, "Claude usage limit reached", Some(1));

    assert_eq!(evaluate(&f, &record, None).await, PlanOutcome::Limit);
    assert!(f
        .ctx
        .stores
        .state
        .read()
        .expect("Zustand")
        .paused_until
        .is_some());
    assert!(f.comments(&id).await[0].contains("Nutzungslimit erreicht"));
    assert_eq!(
        f.notices.last().expect("Meldung").kind,
        DispatchNoticeKind::Paused
    );
    assert!(!Path::new(&record.cwd).exists());
}

#[tokio::test]
async fn ein_erneuter_plan_an_einem_schon_geplanten_ticket_zahlt_erst_mit_neuerem_planned_at() {
    let f = DispatchFixture::create(json!({ "enabled": false })).await;
    let id = backlog(&f, "Rabatte").await;
    plan_as_agent(&f, &id).await;
    pause().await;
    // Nach dem Plan braucht das Ticket wieder Planung, damit „Planen“ erneut greift
    f.backend
        .services
        .tickets
        .update(
            &id,
            &TicketPatch {
                needs_planning: Some(true),
                ..TicketPatch::default()
            },
            &f.actor(),
            &WriteOptions::default(),
        )
        .await
        .expect("Planungsbedarf");
    PlanRunner::new(&f.ctx)
        .start(&id, StartMode::Manual)
        .await
        .expect("Start");
    let record = f.run(&id);
    assert_eq!(
        record.planned_before,
        Some(f.ticket(&id).await.planned_at.clone())
    );
    f.end(&id, dispatch_support::OK_OUTPUT, Some(0));
    assert_eq!(evaluate(&f, &record, None).await, PlanOutcome::Empty);
}

#[tokio::test]
async fn ein_verschwundenes_ticket_endet_als_gone() {
    let f = DispatchFixture::create(json!({ "enabled": false })).await;
    let id = backlog(&f, "Rabatte").await;
    PlanRunner::new(&f.ctx)
        .start(&id, StartMode::Manual)
        .await
        .expect("Start");
    let record = RunRecord {
        ticket_id: "ARC-9999".into(),
        ..f.run(&id)
    };
    f.end(&id, dispatch_support::OK_OUTPUT, Some(0));
    let end = f.result(&record.id);
    let outcome = PlanRunner::new(&f.ctx)
        .evaluate(&record, &end, None)
        .await
        .expect("Auswertung");
    assert_eq!(outcome, PlanOutcome::Gone);
    assert!(!Path::new(&record.cwd).exists());
}

#[tokio::test]
async fn entfernt_verwaiste_planungs_worktrees_ohne_lauf_laufende_bleiben() {
    let f = DispatchFixture::create(json!({})).await;
    let id = backlog(&f, "Läuft").await;
    PlanRunner::new(&f.ctx)
        .start(&id, StartMode::Manual)
        .await
        .expect("Start");
    let running = f.run(&id);
    let root = running.repo_root.clone().expect("Hauptordner");
    let orphan = nexus_core::dispatch::git::ResearchWorktree::with_options(
        nexus_core::dispatch::git::WorktreePurpose::Plan,
        nexus_core::dispatch::git::ResearchWorktreeOptions {
            home: Some(f.home.clone()),
            ..Default::default()
        },
    )
    .create(Path::new(&root), "ARC-0042")
    .expect("verwaister Worktree");

    let notes = PlanRunner::new(&f.ctx).remove_orphans(&root, std::slice::from_ref(&running.cwd));
    assert_eq!(notes.len(), 1);
    assert!(notes[0].contains("verwaister Planungs-Worktree"));
    assert!(!Path::new(&orphan.path).exists());
    assert!(Path::new(&running.cwd).exists());
}

#[tokio::test]
async fn automatik_nur_tickets_ab_dem_einschalten_je_ticket_einmal() {
    let f = DispatchFixture::create(json!({
        "enabled": false,
        "planning": { "wip": 1 },
    }))
    .await;
    let early = backlog(&f, "Vor dem Einschalten").await;
    let runner = PlanRunner::new(&f.ctx);
    // Aus: nichts
    assert!(runner.start_auto(5).await.expect("aus").started.is_empty());

    auto_on(&f).await;
    let ticket = backlog(&f, "Rabatte berechnen").await;
    assert_eq!(
        runner.start_auto(5).await.expect("an").started,
        vec![ticket.clone()]
    );
    let record = f.run(&ticket);
    assert_eq!(record.kind, RunKind::Plan);
    assert_eq!(record.agent, format!("planner-{ticket}"));
    assert_eq!(record.branch, "");
    assert!(record.cwd.ends_with(&format!("-worktrees/plan-{ticket}")));
    let allowed = record
        .args
        .iter()
        .position(|arg| arg == "--allowedTools")
        .expect("Erlaubte");
    assert_eq!(record.args[allowed + 1..], PLANNING_TOOLS.map(String::from));
    let state = f.ctx.stores.state.read().expect("Zustand");
    assert_eq!(state.planning[&ticket].count, 1);
    assert!(state.planning[&ticket].auto_at.is_some());

    // Kein zweiter Anstoß; ältere Tickets bleiben außen vor
    assert!(runner
        .start_auto(5)
        .await
        .expect("zweite Runde")
        .started
        .is_empty());
    assert!(!f
        .ctx
        .stores
        .state
        .read()
        .expect("Zustand")
        .planning
        .contains_key(&early));
}

#[tokio::test]
async fn automatik_startet_erst_wenn_im_wip_der_tickets_platz_ist() {
    let f = DispatchFixture::create(json!({
        "enabled": false,
        "planning": { "wip": 1 },
    }))
    .await;
    auto_on(&f).await;
    let ticket = backlog(&f, "Planen").await;
    let runner = PlanRunner::new(&f.ctx);
    assert!(runner
        .start_auto(0)
        .await
        .expect("kein Platz")
        .started
        .is_empty());
    assert_eq!(
        runner.start_auto(1).await.expect("Platz").started,
        vec![ticket]
    );
}

#[tokio::test]
async fn planning_wip_begrenzt_gleichzeitige_planungen_ein_tageslimit_gibt_es_nicht() {
    let f = DispatchFixture::create(json!({
        "enabled": false,
        "planning": { "wip": 2 },
    }))
    .await;
    auto_on(&f).await;
    let mut ids = Vec::new();
    for title in ["Eins", "Zwei", "Drei", "Vier"] {
        ids.push(backlog(&f, title).await);
    }
    let runner = PlanRunner::new(&f.ctx);
    assert_eq!(
        runner.start_auto(5).await.expect("erste").started,
        ids[..2].to_vec()
    );
    assert!(runner.start_auto(5).await.expect("voll").started.is_empty());
    for id in &ids[..2] {
        let record = f.run(id);
        f.end(id, dispatch_support::OK_OUTPUT, Some(0));
        evaluate(&f, &record, None).await;
    }
    assert_eq!(
        runner.start_auto(5).await.expect("nächste").started,
        ids[2..].to_vec()
    );
}

#[tokio::test]
async fn automatik_uberspringt_tickets_mit_offener_frage_und_boards_ohne_repo_pfad() {
    let f = DispatchFixture::create(json!({
        "enabled": false,
        "planning": { "wip": 5 },
    }))
    .await;
    let no_repo = f
        .backend
        .services
        .boards
        .create("ARC", "Ohne Repo", &f.actor())
        .await
        .expect("Board ohne Repo")
        .board;
    auto_on(&f).await;
    let waiting = backlog(&f, "Mit Frage").await;
    f.backend
        .services
        .questions
        .create(
            "ARC",
            &NewBoardQuestionInput {
                ticket_ids: Some(vec![waiting.clone()]),
                ..NewBoardQuestionInput::titled("Brutto oder netto?")
            },
            &human(&f),
        )
        .await
        .expect("Frage anlegen");
    let homeless = backlog_on(&f, no_repo.id, "Ohne Repo").await;
    let fine = backlog(&f, "Sauber").await;

    assert_eq!(
        PlanRunner::new(&f.ctx)
            .start_auto(5)
            .await
            .expect("Automatik")
            .started,
        vec![fine]
    );
    let state = f.ctx.stores.state.read().expect("Zustand");
    assert!(!state.planning.contains_key(&waiting));
    assert!(!state.planning.contains_key(&homeless));
}

#[tokio::test]
async fn plant_ein_ticket_nach_einem_lauf_ohne_ergebnis_nicht_erneut_automatisch() {
    let f = DispatchFixture::create(json!({
        "enabled": false,
        "planning": { "wip": 5 },
    }))
    .await;
    auto_on(&f).await;
    let ticket = backlog(&f, "Bleibt offen").await;
    let runner = PlanRunner::new(&f.ctx);
    assert_eq!(
        runner.start_auto(5).await.expect("erster Anstoß").started,
        vec![ticket.clone()]
    );
    let record = f.run(&ticket);
    f.end(&ticket, "", Some(1));
    assert_eq!(evaluate(&f, &record, None).await, PlanOutcome::Empty);
    assert!(f.ticket(&ticket).await.needs_planning);
    assert!(runner
        .start_auto(5)
        .await
        .expect("kein zweiter Anstoß")
        .started
        .is_empty());
}

#[tokio::test]
async fn fehler_des_tickets_merken_es_als_erledigt_die_ubrigen_werden_weiter_bedient() {
    let f = DispatchFixture::create(json!({
        "enabled": false,
        "planning": { "wip": 5 },
    }))
    .await;
    auto_on(&f).await;
    // Planungsbedarf, aber „In Arbeit“: ein Kandidat der Automatik, den die Prüfung ablehnt
    let broken = backlog(&f, "Kaputt").await;
    f.backend
        .services
        .tickets
        .update(
            &broken,
            &TicketPatch {
                status: Some(TicketStatus::InProgress),
                needs_planning: Some(true),
                ..TicketPatch::default()
            },
            &f.actor(),
            &WriteOptions::default(),
        )
        .await
        .expect("Status setzen");
    let fine = backlog(&f, "Gut").await;

    let runner = PlanRunner::new(&f.ctx);
    let report = runner.start_auto(5).await.expect("Automatik");
    assert_eq!(report.started, vec![fine]);
    assert_eq!(report.errors.len(), 1);
    assert!(
        report.errors[0].starts_with(&format!("{broken}: {broken} steht auf")),
        "{}",
        report.errors[0]
    );
    assert!(
        f.ctx.stores.state.read().expect("Zustand").planning[&broken]
            .auto_at
            .is_some()
    );
    // Gemerkt: kein zweiter Versuch
    assert!(runner
        .start_auto(5)
        .await
        .expect("zweite Runde")
        .started
        .is_empty());
}

#[tokio::test]
async fn board_mit_repo_pfad_ohne_git_halt_die_ubrigen_boards_nicht_auf() {
    let f = DispatchFixture::create(json!({
        "enabled": false,
        "planning": { "wip": 5 },
    }))
    .await;
    let empty = tempfile::Builder::new()
        .prefix("nexus-ohne-git-")
        .tempdir()
        .expect("Ordner");
    let broken_board = f
        .backend
        .services
        .boards
        .create("ARC", "Ohne Git", &f.actor())
        .await
        .expect("Board ohne Git")
        .board;
    f.backend
        .services
        .boards
        .set_repo_path(broken_board.id, Some(&empty.path().to_string_lossy()))
        .await
        .expect("Repo-Pfad");
    auto_on(&f).await;
    // Kleinere Nummer: steht in der Reihenfolge vor dem sauberen Ticket
    let stuck = backlog_on(&f, broken_board.id, "Im kaputten Repo").await;
    let fine = backlog(&f, "Sauber").await;

    let runner = PlanRunner::new(&f.ctx);
    let first = runner.start_auto(5).await.expect("erster Tick");
    assert_eq!(first.started, vec![fine]);
    assert_eq!(first.errors.len(), 1, "{:?}", first.errors);
    assert!(
        first.errors[0].contains("Ohne Git") && first.errors[0].contains("kein Git-Projekt"),
        "{}",
        first.errors[0]
    );
    let state = f.ctx.stores.state.read().expect("Zustand");
    // Nicht gemerkt: ist das Repo repariert, kommt das Ticket noch dran
    assert!(!state.planning.contains_key(&stuck));
    // Keine Pause für alle Boards
    assert_eq!(state.planning_auto.retry_at, None);
    assert!(state
        .planning_auto
        .last_error
        .as_deref()
        .is_some_and(|error| error.contains("Ohne Git")));

    // Derselbe Fehler wird nicht jede Minute erneut gemeldet
    let again = runner.start_auto(5).await.expect("zweiter Tick");
    assert!(
        again.started.is_empty() && again.errors.is_empty(),
        "{again:?}"
    );
}

#[tokio::test]
async fn umgebungsfehler_abbruch_ohne_merken_einmal_gemeldet_neuversuch_nach_30_min() {
    let f = DispatchFixture::create(json!({
        "enabled": false,
        "planning": { "wip": 5 },
    }))
    .await;
    auto_on(&f).await;
    let ticket = backlog(&f, "Planen").await;
    f.launcher.fail_with("Festplatte voll");

    let runner = PlanRunner::new(&f.ctx);
    let first = runner.start_auto(5).await.expect("erster Versuch");
    assert!(first.started.is_empty());
    assert_eq!(first.errors.len(), 1);
    assert!(
        first.errors[0].starts_with("Planungs-Automatik ausgesetzt bis")
            && first.errors[0].ends_with("Festplatte voll"),
        "{}",
        first.errors[0]
    );
    let state = f.ctx.stores.state.read().expect("Zustand");
    assert!(!state.planning.contains_key(&ticket));
    assert_eq!(
        state.planning_auto.last_error.as_deref(),
        Some("Festplatte voll")
    );
    // Ausgesetzt: kein neuer Versuch, kein zweiter Bericht
    let again = runner.start_auto(5).await.expect("ausgesetzt");
    assert!(again.started.is_empty() && again.errors.is_empty());
    // Der Claim des gescheiterten Starts ist wieder frei
    assert_eq!(f.ticket(&ticket).await.planning_claim_by, None);

    f.launcher.succeed();
    f.advance(31);
    assert_eq!(
        runner.start_auto(5).await.expect("wieder frei").started,
        vec![ticket]
    );
    let state = f.ctx.stores.state.read().expect("Zustand");
    assert_eq!(state.planning_auto.last_error, None);
    assert_eq!(state.planning_auto.retry_at, None);
}

#[tokio::test]
async fn aus_und_wieder_einschalten_neuer_zeitpunkt_tickets_aus_der_zwischenzeit_bleiben_aussen() {
    let f = DispatchFixture::create(json!({
        "enabled": false,
        "planning": { "wip": 5 },
    }))
    .await;
    auto_on(&f).await;
    f.ctx
        .stores
        .state
        .update(|state| state.planning_auto.since = None)
        .await
        .expect("Automatik aus");
    backlog(&f, "Zwischendurch").await;
    auto_on(&f).await;
    let fresh = backlog(&f, "Neu").await;
    assert_eq!(
        PlanRunner::new(&f.ctx)
            .start_auto(5)
            .await
            .expect("Automatik")
            .started,
        vec![fresh]
    );
}
