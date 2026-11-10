//! Recherche-Läufe zu offenen Fragen: Start von Hand, Auswertung und Automatik; portiert aus
//! `packages/server/tests/unit/dispatch/DispatchResearch.test.ts`.
//!
//! Die Tick-Schleife gibt es in Rust noch nicht (sie kommt in P4-14c): die Tests rufen
//! [`ResearchRunner::start`], [`ResearchRunner::evaluate`] und [`ResearchRunner::start_auto`]
//! selbst auf – genau die Schritte, die der Tick je Frage tut. Pause, Zeitfenster und die
//! Statusanzeige hängen am Dienst und stehen deshalb in P4-14c bzw. P4-16a.

mod common;
mod dispatch_support;

use std::path::Path;

use nexus_core::actors::AuthorKind;
use nexus_core::board::basics::BoardActor;
use nexus_core::board::basics::TicketStatus;
use nexus_core::board::database::BoardDatabase as _;
use nexus_core::board::question_service::NewBoardQuestionInput;
use nexus_core::board::question_types::{BoardQuestionActor, BoardQuestionDetail};
use nexus_core::board::ticket_service::TicketPatch;
use nexus_core::board::types::BoardUser;
use nexus_core::dispatch::autopilot::is_autopilot_decision;
use nexus_core::dispatch::config::{RESEARCH_DENIED_TOOLS, RESEARCH_TOOLS};
use nexus_core::dispatch::git::{ResearchWorktree, ResearchWorktreeOptions, WorktreePurpose};
use nexus_core::dispatch::{markers, RunKind, RunRecord};
use nexus_core::questions::state::QuestionState;
use nexus_core::questions::{ResearchInput, ResearchOptionInput};
use nexus_core::tx;
use nexus_server::dispatch::notifier::DispatchNoticeKind;
use nexus_server::dispatch::research_runner::{ResearchOutcome, ResearchRunner, StartMode};
use nexus_server::dispatch::ticket_starter::StartOutcome;
use serde_json::json;

use dispatch_support::DispatchFixture;

const OTHER_USER_ID: &str = "22222222-2222-2222-2222-222222222222";

/// Der Mensch, der die Fragen stellt (der Benutzer des Tests)
fn human(f: &DispatchFixture) -> BoardQuestionActor {
    BoardQuestionActor {
        actor: f.actor(),
        kind: AuthorKind::Human,
        session: None,
    }
}

/// Ein Agent des Dispatchers als Autor
fn session(f: &DispatchFixture, name: &str) -> BoardQuestionActor {
    BoardQuestionActor {
        actor: BoardActor {
            user_id: f.actor().user_id,
            name: name.to_string(),
        },
        kind: AuthorKind::Session,
        session: None,
    }
}

/// Eine Recherche mit Kontext und zwei Optionen (das `RESEARCH` aus TypeScript)
fn research() -> ResearchInput {
    ResearchInput {
        context: Some("Fundstelle src/a.ts:1".into()),
        options: Some(vec![
            ResearchOptionInput {
                title: Some("Brutto".into()),
                text: Some("immer brutto".into()),
                ..ResearchOptionInput::default()
            },
            ResearchOptionInput {
                title: Some("Netto".into()),
                text: Some("immer netto".into()),
                ..ResearchOptionInput::default()
            },
        ]),
    }
}

/// Legt eine offene Frage im Projekt an und liefert ihre Kennung
async fn ask(f: &DispatchFixture, title: &str, actor: &BoardQuestionActor) -> String {
    ask_in(f, "ARC", title, actor, &[]).await
}

/// Wie [`ask`], mit Projekt und verknüpften Tickets
async fn ask_in(
    f: &DispatchFixture,
    project: &str,
    title: &str,
    actor: &BoardQuestionActor,
    tickets: &[String],
) -> String {
    f.backend
        .services
        .questions
        .create(
            project,
            &NewBoardQuestionInput {
                ticket_ids: Some(tickets.to_vec()),
                ..NewBoardQuestionInput::titled(title)
            },
            actor,
        )
        .await
        .expect("Frage anlegen")
        .view
        .question
        .id
}

/// Die Frage mit Recherchen und Kommentaren
async fn detail(f: &DispatchFixture, id: &str) -> BoardQuestionDetail {
    f.backend
        .services
        .questions
        .get(id)
        .await
        .expect("Frage lesen")
}

/// Texte der Kommentare an der Frage
async fn comments(f: &DispatchFixture, id: &str) -> Vec<String> {
    detail(f, id)
        .await
        .comments
        .into_iter()
        .map(|comment| comment.body)
        .collect()
}

/// Schaltet die Automatik ein, als hätte ein Tick den Zeitpunkt „ab jetzt“ gemerkt
async fn auto_on(f: &DispatchFixture) {
    pause().await;
    let since = nexus_core::clock::iso(chrono::Utc::now());
    f.ctx
        .stores
        .state
        .update(move |state| state.research_auto.since = Some(since))
        .await
        .expect("Automatik einschalten");
    pause().await;
}

/// Wartet kurz, damit Zeitstempel davor und danach sicher verschieden sind
async fn pause() {
    tokio::time::sleep(std::time::Duration::from_millis(5)).await;
}

/// Wertet den Lauf aus, wie der Tick es nach seinem Ende tut
async fn evaluate(
    f: &DispatchFixture,
    record: &RunRecord,
    reason: Option<&str>,
) -> ResearchOutcome {
    let end = f.result(&record.id);
    let outcome = ResearchRunner::new(&f.ctx)
        .evaluate(record, &end, reason)
        .await
        .expect("Auswertung");
    f.drop_run(&record.id);
    outcome
}

/// Der Lauf einer Frage
fn run_of(f: &DispatchFixture, question_id: &str) -> RunRecord {
    f.run(question_id)
}

#[tokio::test]
async fn startet_die_recherche_im_losgelosten_worktree_des_board_repos_eingeschrankt_ohne_harness_ticket(
) {
    let f = DispatchFixture::create(json!({
        "enabled": false,
        "claude": {
            "permissionMode": "bypassPermissions",
            "extraArgs": ["--dangerously-skip-permissions"],
            "allowedTools": ["Bash(*)"],
        },
    }))
    .await;
    let id = ask(&f, "Brutto oder netto?", &human(&f)).await;
    let record = ResearchRunner::new(&f.ctx)
        .start(&id, StartMode::Manual)
        .await
        .expect("Recherche starten");

    assert_eq!(record.kind, RunKind::Research);
    assert_eq!(record.ticket_id, id);
    assert_eq!(record.question_id.as_deref(), Some(id.as_str()));
    assert_eq!(record.title, "Brutto oder netto?");
    assert_eq!(record.agent, format!("research-{id}"));
    assert_eq!(record.branch, "");
    assert_eq!(record.research_before, Some(0));
    assert!(record.repo_root.is_some());
    assert!(
        record.cwd.ends_with(&format!("-worktrees/research-{id}")),
        "{}",
        record.cwd
    );
    assert!(Path::new(&record.cwd).exists());
    assert!(record.log_file.ends_with(&format!("{}.log", record.id)));
    // Der Worktree hängt an keinem Branch
    assert_eq!(
        f.repo
            .git_out(&["-C", &record.cwd, "rev-parse", "--abbrev-ref", "HEAD"]),
        "HEAD"
    );

    // Einstellungen (Rechte-Modus, extraArgs, allowedTools) heben die Einschränkung nicht auf
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
        RESEARCH_DENIED_TOOLS.map(String::from)
    );
    assert_eq!(record.args[allowed + 1..], RESEARCH_TOOLS.map(String::from));
    assert!(record.prompt.contains(&format!(
        "harness board question research {id} --from research.json --author research-{id}"
    )));
    assert!(!f.launcher.env_of(&record.id).contains_key("HARNESS_TICKET"));

    let state = f.ctx.stores.state.read().expect("Zustand");
    let entry = state.research.get(&id).expect("Eintrag der Frage");
    assert_eq!(entry.count, 1);
    assert_eq!(entry.auto_at, None);
}

#[tokio::test]
async fn lehnt_ab_laufende_recherche_nicht_offene_frage_unbekannte_frage_und_kein_board_mit_repo() {
    let f = DispatchFixture::create(json!({})).await;
    let runner = ResearchRunner::new(&f.ctx);
    let id = ask(&f, "Offen", &human(&f)).await;
    runner.start(&id, StartMode::Manual).await.expect("Start");
    assert_eq!(
        runner
            .start(&id, StartMode::Manual)
            .await
            .expect_err("zweiter Start")
            .code(),
        Some("research-running")
    );

    let done = ask(&f, "Erledigt", &human(&f)).await;
    f.backend
        .services
        .questions
        .discard(&done, Some("egal"), &human(&f), &Default::default())
        .await
        .expect("verwerfen");
    assert_eq!(
        runner
            .start(&done, StartMode::Manual)
            .await
            .expect_err("verworfene Frage")
            .code(),
        Some("question-not-open")
    );
    assert_eq!(
        runner
            .start("ARC-F0099", StartMode::Manual)
            .await
            .expect_err("unbekannte Frage")
            .code(),
        Some("question-not-found")
    );

    f.backend
        .services
        .projects
        .create("OTH", "Anderes", &f.actor())
        .await
        .expect("zweites Projekt");
    let foreign = ask_in(&f, "OTH", "Ohne Board", &human(&f), &[]).await;
    let error = runner
        .start(&foreign, StartMode::Manual)
        .await
        .expect_err("Projekt ohne eigenes Board mit Repo");
    assert_eq!(error.code(), Some("research-no-repo"));
    assert!(error.to_string().contains(&format!(
        "Für Frage {foreign} ist kein eigenes Board mit Repo-Pfad bekannt"
    )));
}

#[tokio::test]
async fn nimmt_das_eigene_board_eines_verknupften_tickets_auch_bei_anderem_aktivem_board() {
    let f = DispatchFixture::create(json!({})).await;
    let ticket = f.ready("Rabatte", TicketPatch::default()).await;
    let second = f
        .backend
        .services
        .boards
        .create("ARC", "Zweites", &f.actor())
        .await
        .expect("zweites Board")
        .board;
    f.backend
        .services
        .boards
        .set_repo_path(second.id, Some(&f.repo.path().to_string_lossy()))
        .await
        .expect("Repo-Pfad");
    let id = ask_in(&f, "ARC", "Zum Ticket", &human(&f), &[ticket]).await;

    // Aktiv ist das zweite Board; das Board des verknüpften Tickets gewinnt trotzdem
    let ctx = f.context_with_active_board(Some(second.id));
    let record = ResearchRunner::new(&ctx)
        .start(&id, StartMode::Manual)
        .await
        .expect("Recherche starten");
    assert_eq!(record.board_id, i64::from(f.board.id));
}

#[tokio::test]
async fn ohne_verknupftes_ticket_aktives_board_des_projekts_sonst_das_mit_der_kleinsten_id() {
    let f = DispatchFixture::create(json!({})).await;
    let second = f
        .backend
        .services
        .boards
        .create("ARC", "Zweites", &f.actor())
        .await
        .expect("zweites Board")
        .board;
    f.backend
        .services
        .boards
        .set_repo_path(second.id, Some(&f.repo.path().to_string_lossy()))
        .await
        .expect("Repo-Pfad");
    let no_repo = f
        .backend
        .services
        .boards
        .create("ARC", "Ohne Repo", &f.actor())
        .await
        .expect("Board ohne Repo")
        .board;

    // Aktiv ist ein Board ohne Repo-Pfad: es zählt das eigene Board mit Repo und kleinster ID
    let first = ask(&f, "Eins", &human(&f)).await;
    let ctx = f.context_with_active_board(Some(no_repo.id));
    let record = ResearchRunner::new(&ctx)
        .start(&first, StartMode::Manual)
        .await
        .expect("Recherche starten");
    assert_eq!(record.board_id, i64::from(f.board.id.min(second.id)));

    // Ist das aktive Board ein Board des Projekts mit Repo, gewinnt es
    let other = ask(&f, "Zwei", &human(&f)).await;
    let ctx = f.context_with_active_board(Some(second.id));
    let question = detail(&f, &other).await;
    let board = ResearchRunner::new(&ctx)
        .locate(&question.view)
        .await
        .expect("Board der Frage");
    assert_eq!(board.id, second.id);
}

#[tokio::test]
async fn ein_startfehler_des_workers_lasst_keinen_lauf_und_keinen_worktree_zuruck() {
    let f = DispatchFixture::create(json!({})).await;
    let id = ask(&f, "Frage", &human(&f)).await;
    f.launcher.fail_with("claude fehlt");
    let error = ResearchRunner::new(&f.ctx)
        .start(&id, StartMode::Manual)
        .await
        .expect_err("Start scheitert");
    assert!(error.to_string().contains("claude fehlt"));
    assert!(f.ctx.stores.runs.list().expect("Läufe").is_empty());
    assert!(!f.repo.git_out(&["worktree", "list"]).contains("research-"));
}

#[tokio::test]
async fn neue_recherche_des_agents_ok_worktree_entfernt_kein_kommentar() {
    let f = DispatchFixture::create(json!({ "enabled": false })).await;
    let id = ask(&f, "Frage", &human(&f)).await;
    ResearchRunner::new(&f.ctx)
        .start(&id, StartMode::Manual)
        .await
        .expect("Start");
    let record = run_of(&f, &id);
    f.backend
        .services
        .questions
        .add_research(&id, &research(), &session(&f, &format!("research-{id}")))
        .await
        .expect("Recherche speichern");
    f.end(&id, dispatch_support::OK_OUTPUT, Some(0));

    assert_eq!(
        evaluate(&f, &record, None).await,
        ResearchOutcome::Researched
    );
    assert!(!Path::new(&record.cwd).exists());
    assert!(f.ctx.stores.runs.list().expect("Läufe").is_empty());
    assert!(comments(&f, &id).await.is_empty());
}

#[tokio::test]
async fn eine_recherche_eines_anderen_autors_wahrend_des_laufs_zahlt_nicht_als_ergebnis() {
    let f = DispatchFixture::create(json!({ "enabled": false })).await;
    let id = ask(&f, "Frage", &human(&f)).await;
    ResearchRunner::new(&f.ctx)
        .start(&id, StartMode::Manual)
        .await
        .expect("Start");
    let record = run_of(&f, &id);
    f.backend
        .services
        .questions
        .add_research(&id, &research(), &human(&f))
        .await
        .expect("fremde Recherche");
    f.end(&id, dispatch_support::OK_OUTPUT, Some(0));

    assert_eq!(evaluate(&f, &record, None).await, ResearchOutcome::Empty);
    assert!(comments(&f, &id).await[0].starts_with(markers::RESEARCH_EMPTY));
}

#[tokio::test]
async fn ohne_neue_recherche_kommentar_mit_grund_und_ausgabe_an_der_frage() {
    let f = DispatchFixture::create(json!({ "enabled": false })).await;
    let id = ask(&f, "Frage", &human(&f)).await;
    ResearchRunner::new(&f.ctx)
        .start(&id, StartMode::Manual)
        .await
        .expect("Start");
    let record = run_of(&f, &id);
    let denied = json!({
        "type": "result", "subtype": "success", "is_error": false,
        "result": "nichts gefunden", "permission_denials": [{ "tool_name": "Edit" }],
    })
    .to_string();
    f.end(&id, &denied, None);

    assert_eq!(evaluate(&f, &record, None).await, ResearchOutcome::Empty);
    assert!(!Path::new(&record.cwd).exists());
    let question = detail(&f, &id).await;
    assert_eq!(question.comments.len(), 1);
    let comment = &question.comments[0];
    assert_eq!(comment.author_name, format!("research-{id}"));
    assert_eq!(comment.author_kind, AuthorKind::Session);
    assert!(comment.body.starts_with(&format!(
        "{}: Lauf endete, ohne eine Recherche an der Frage zu speichern; abgelehnte \
Werkzeugaufrufe: Edit",
        markers::RESEARCH_EMPTY
    )));
    assert!(comment.body.contains("nichts gefunden"));
    assert!(comment
        .body
        .contains(&format!("Protokoll: {}.log", record.id)));
}

#[tokio::test]
async fn nutzungslimit_pause_kommentar_worktree_entfernt() {
    let f = DispatchFixture::create(json!({})).await;
    let id = ask(&f, "Frage", &human(&f)).await;
    ResearchRunner::new(&f.ctx)
        .start(&id, StartMode::Manual)
        .await
        .expect("Start");
    let record = run_of(&f, &id);
    f.end(&id, "Claude usage limit reached", Some(1));

    assert_eq!(evaluate(&f, &record, None).await, ResearchOutcome::Limit);
    assert!(!Path::new(&record.cwd).exists());
    assert!(f
        .ctx
        .stores
        .state
        .read()
        .expect("Zustand")
        .paused_until
        .is_some());
    assert!(comments(&f, &id).await[0].starts_with(&format!(
        "{}: Nutzungslimit erreicht",
        markers::RESEARCH_EMPTY
    )));
    assert_eq!(
        f.notices.last().expect("Meldung").kind,
        DispatchNoticeKind::Paused
    );
}

#[tokio::test]
async fn wiederholte_auswertung_kein_zweiter_kommentar_und_keine_zweite_pause() {
    let f = DispatchFixture::create(json!({ "enabled": false })).await;
    let id = ask(&f, "Frage", &human(&f)).await;
    ResearchRunner::new(&f.ctx)
        .start(&id, StartMode::Manual)
        .await
        .expect("Start");
    let record = run_of(&f, &id);
    f.backend
        .services
        .questions
        .comment(
            &id,
            &format!("{}: Nutzungslimit erreicht. …", markers::RESEARCH_EMPTY),
            &session(&f, &format!("research-{id}")),
        )
        .await
        .expect("Kommentar des Laufs");
    f.end(&id, "Claude usage limit reached", Some(1));

    assert_eq!(evaluate(&f, &record, None).await, ResearchOutcome::Limit);
    assert_eq!(comments(&f, &id).await.len(), 1);
    assert_eq!(
        f.ctx.stores.state.read().expect("Zustand").paused_until,
        None
    );
    assert!(f.notices.all().is_empty());

    let other = ask(&f, "Zweite", &human(&f)).await;
    ResearchRunner::new(&f.ctx)
        .start(&other, StartMode::Manual)
        .await
        .expect("Start");
    let second = run_of(&f, &other);
    f.backend
        .services
        .questions
        .comment(
            &other,
            &format!("{}: schon da.", markers::RESEARCH_EMPTY),
            &session(&f, &format!("research-{other}")),
        )
        .await
        .expect("Kommentar des Laufs");
    f.end(&other, dispatch_support::OK_OUTPUT, Some(0));
    assert_eq!(evaluate(&f, &second, None).await, ResearchOutcome::Empty);
    assert_eq!(comments(&f, &other).await.len(), 1);
}

#[tokio::test]
async fn verschwundener_worker_kommentar_mit_grund_worktree_entfernt() {
    let f = DispatchFixture::create(json!({ "enabled": false })).await;
    let id = ask(&f, "Frage", &human(&f)).await;
    ResearchRunner::new(&f.ctx)
        .start(&id, StartMode::Manual)
        .await
        .expect("Start");
    let record = run_of(&f, &id);
    f.end(&id, "", None);

    let outcome = evaluate(&f, &record, Some("Worker-Prozess endete ohne Ergebnis")).await;
    assert_eq!(outcome, ResearchOutcome::Empty);
    assert!(!Path::new(&record.cwd).exists());
    assert!(comments(&f, &id).await[0].contains("Worker-Prozess endete ohne Ergebnis"));
}

#[tokio::test]
async fn eine_verschwundene_frage_endet_als_gone() {
    let f = DispatchFixture::create(json!({ "enabled": false })).await;
    let id = ask(&f, "Frage", &human(&f)).await;
    ResearchRunner::new(&f.ctx)
        .start(&id, StartMode::Manual)
        .await
        .expect("Start");
    let record = RunRecord {
        question_id: Some("ARC-F0099".into()),
        ..run_of(&f, &id)
    };
    f.end(&id, dispatch_support::OK_OUTPUT, Some(0));
    let end = f.result(&record.id);
    let outcome = ResearchRunner::new(&f.ctx)
        .evaluate(&record, &end, None)
        .await
        .expect("Auswertung");
    assert_eq!(outcome, ResearchOutcome::Gone);
    assert!(!Path::new(&record.cwd).exists());
}

#[tokio::test]
async fn entfernt_verwaiste_recherche_worktrees_ohne_lauf_laufende_bleiben() {
    let f = DispatchFixture::create(json!({})).await;
    let id = ask(&f, "Läuft", &human(&f)).await;
    ResearchRunner::new(&f.ctx)
        .start(&id, StartMode::Manual)
        .await
        .expect("Start");
    let running = run_of(&f, &id);
    let root = running.repo_root.clone().expect("Hauptordner");
    let orphan = ResearchWorktree::with_options(
        WorktreePurpose::Research,
        ResearchWorktreeOptions {
            home: Some(f.home.clone()),
            ..ResearchWorktreeOptions::default()
        },
    )
    .create(Path::new(&root), "ARC-F0042")
    .expect("verwaister Worktree");

    let notes =
        ResearchRunner::new(&f.ctx).remove_orphans(&root, std::slice::from_ref(&running.cwd));
    assert_eq!(notes.len(), 1);
    assert!(notes[0].contains("verwaister Recherche-Worktree"));
    assert!(!Path::new(&orphan.path).exists());
    assert!(Path::new(&running.cwd).exists());
}

#[tokio::test]
async fn automatik_nur_fragen_ab_dem_einschalten_alteste_zuerst_bis_wip_je_frage_einmal() {
    let f = DispatchFixture::create(json!({
        "enabled": false,
        "research": { "wip": 1, "maxPerDay": 10 },
    }))
    .await;
    let old = ask(&f, "Vor dem Einschalten", &human(&f)).await;
    let runner = ResearchRunner::new(&f.ctx);
    // Aus: nichts
    assert!(runner.start_auto().await.expect("aus").started.is_empty());

    auto_on(&f).await;
    let first = ask(&f, "Erste", &human(&f)).await;
    pause().await;
    let second = ask(&f, "Zweite", &human(&f)).await;
    let researched = ask(&f, "Schon recherchiert", &human(&f)).await;
    f.backend
        .services
        .questions
        .add_research(&researched, &research(), &human(&f))
        .await
        .expect("Recherche");

    assert_eq!(
        runner.start_auto().await.expect("an").started,
        vec![first.clone()]
    );
    // Der einzige Platz ist belegt
    assert!(runner.start_auto().await.expect("voll").started.is_empty());
    let state = f.ctx.stores.state.read().expect("Zustand");
    assert!(state.research[&first].auto_at.is_some());

    let record = run_of(&f, &first);
    f.end(&first, dispatch_support::OK_OUTPUT, Some(0));
    evaluate(&f, &record, None).await;
    assert_eq!(
        runner.start_auto().await.expect("nächste").started,
        vec![second.clone()]
    );
    let record = run_of(&f, &second);
    f.end(&second, dispatch_support::OK_OUTPUT, Some(0));
    evaluate(&f, &record, None).await;
    // Keine zweite Runde für dieselben Fragen, ältere bleiben außen vor
    assert!(runner
        .start_auto()
        .await
        .expect("fertig")
        .started
        .is_empty());
    let state = f.ctx.stores.state.read().expect("Zustand");
    assert!(!state.research.contains_key(&old));
    assert!(!state.research.contains_key(&researched));
}

#[tokio::test]
async fn automatik_tageslimit_max_per_day_am_nachsten_tag_geht_es_weiter() {
    let f = DispatchFixture::create(json!({
        "enabled": false,
        "research": { "wip": 5, "maxPerDay": 2 },
    }))
    .await;
    auto_on(&f).await;
    let runner = ResearchRunner::new(&f.ctx);
    let mut ids = Vec::new();
    for title in ["Eins", "Zwei", "Drei"] {
        ids.push(ask(&f, title, &human(&f)).await);
        pause().await;
    }
    assert_eq!(
        runner.start_auto().await.expect("erste").started,
        ids[..2].to_vec()
    );
    assert!(runner
        .start_auto()
        .await
        .expect("Tageslimit")
        .started
        .is_empty());
    let state = f.ctx.stores.state.read().expect("Zustand");
    assert_eq!(
        state.research_auto.day.as_deref(),
        Some(ResearchRunner::day(f.ctx.now()).as_str())
    );
    assert_eq!(state.research_auto.count, 2);

    f.advance(24 * 60);
    assert_eq!(
        runner.start_auto().await.expect("neuer Tag").started,
        vec![ids[2].clone()]
    );
    assert_eq!(
        f.ctx
            .stores
            .state
            .read()
            .expect("Zustand")
            .research_auto
            .count,
        1
    );
}

#[tokio::test]
async fn automatik_nur_fragen_die_mich_betreffen_fremde_nur_von_hand() {
    let f = DispatchFixture::create(json!({
        "enabled": false,
        "research": { "wip": 5, "maxPerDay": 10 },
    }))
    .await;
    let user = BoardUser {
        id: OTHER_USER_ID.into(),
        name: "Bob".into(),
    };
    let db = f.backend.services.db.clone();
    tx!(db, [user], |u| { u.users().upsert(&user).await })
        .await
        .expect("zweiten Benutzer anlegen");
    let other = BoardQuestionActor {
        actor: BoardActor {
            user_id: OTHER_USER_ID.into(),
            name: "Bob".into(),
        },
        kind: AuthorKind::Human,
        session: None,
    };
    let ticket = f.ready("Rabatte", TicketPatch::default()).await;
    auto_on(&f).await;
    let foreign = ask(&f, "Fremd", &other).await;
    pause().await;
    let linked = ask_in(&f, "ARC", "Fremd, zu meinem Ticket", &other, &[ticket]).await;
    pause().await;
    let mine = ask(&f, "Meine (vom Agent)", &session(&f, "junior-ARC-0001")).await;

    let runner = ResearchRunner::new(&f.ctx);
    let mut started = runner.start_auto().await.expect("Automatik").started;
    started.sort();
    let mut expected = vec![linked, mine];
    expected.sort();
    assert_eq!(started, expected);
    assert!(runner
        .start_auto()
        .await
        .expect("zweite Runde")
        .started
        .is_empty());
    // Von Hand geht die fremde Frage trotzdem
    assert_eq!(
        runner
            .start(&foreign, StartMode::Manual)
            .await
            .expect("von Hand")
            .question_id
            .as_deref(),
        Some(foreign.as_str())
    );
}

#[tokio::test]
async fn automatik_umgebungsfehler_abbruch_ohne_merken_einmal_gemeldet_neuversuch_nach_30_min() {
    let f = DispatchFixture::create(json!({
        "enabled": false,
        "research": { "wip": 5, "maxPerDay": 10 },
    }))
    .await;
    auto_on(&f).await;
    let q1 = ask(&f, "Eins", &human(&f)).await;
    pause().await;
    let q2 = ask(&f, "Zwei", &human(&f)).await;
    f.launcher.fail_with("Git-Index gesperrt");

    let runner = ResearchRunner::new(&f.ctx);
    let first = runner.start_auto().await.expect("erster Versuch");
    assert!(first.started.is_empty());
    assert_eq!(first.errors.len(), 1);
    assert!(
        first.errors[0].starts_with("Recherche-Automatik ausgesetzt bis")
            && first.errors[0].ends_with("Git-Index gesperrt"),
        "{}",
        first.errors[0]
    );
    let state = f.ctx.stores.state.read().expect("Zustand");
    assert!(state.research.is_empty());
    assert_eq!(
        state.research_auto.last_error.as_deref(),
        Some("Git-Index gesperrt")
    );
    assert!(state.research_auto.retry_at.is_some());
    assert!(!f.repo.git_out(&["worktree", "list"]).contains("research-"));

    // Ausgesetzt: kein neuer Versuch, kein zweiter Bericht
    f.advance(10);
    let again = runner.start_auto().await.expect("ausgesetzt");
    assert!(again.started.is_empty() && again.errors.is_empty());
    f.advance(25);
    assert!(runner
        .start_auto()
        .await
        .expect("immer noch")
        .started
        .is_empty());

    f.launcher.succeed();
    f.advance(31);
    assert_eq!(
        runner.start_auto().await.expect("wieder frei").started,
        vec![q1, q2]
    );
    let state = f.ctx.stores.state.read().expect("Zustand");
    assert_eq!(state.research_auto.last_error, None);
    assert_eq!(state.research_auto.retry_at, None);
}

#[tokio::test]
async fn automatik_repo_pfad_ohne_git_repo_gilt_als_umgebungsfehler_nicht_als_fehler_der_frage() {
    let f = DispatchFixture::create(json!({
        "enabled": false,
        "research": { "wip": 5, "maxPerDay": 10 },
    }))
    .await;
    auto_on(&f).await;
    let id = ask(&f, "Eins", &human(&f)).await;
    let empty = tempfile::Builder::new()
        .prefix("nexus-ohne-git-")
        .tempdir()
        .expect("Ordner");
    f.backend
        .services
        .boards
        .set_repo_path(f.board.id, Some(&empty.path().to_string_lossy()))
        .await
        .expect("Repo-Pfad");

    let report = ResearchRunner::new(&f.ctx)
        .start_auto()
        .await
        .expect("Automatik");
    assert_eq!(report.errors.len(), 1);
    assert!(report.errors[0].starts_with("Recherche-Automatik ausgesetzt bis"));
    assert!(!f
        .ctx
        .stores
        .state
        .read()
        .expect("Zustand")
        .research
        .contains_key(&id));
}

/// Einstellung mit eingeschaltetem Autopilot; die Recherche-Automatik bleibt aus
fn autopilot(research: serde_json::Value) -> serde_json::Value {
    json!({
        "enabled": false,
        "autopilot": { "enabled": true },
        "research": research,
    })
}

/// Eine Recherche mit zwei Optionen; `recommended` nennt die empfohlenen (nach Titel)
fn research_recommending(recommended: &[&str]) -> ResearchInput {
    let option = |title: &str| ResearchOptionInput {
        title: Some(title.into()),
        text: Some(format!("immer {}", title.to_lowercase())),
        recommended: Some(recommended.contains(&title)),
        ..ResearchOptionInput::default()
    };
    ResearchInput {
        context: Some("Fundstelle src/a.ts:1".into()),
        options: Some(vec![option("Brutto"), option("Netto")]),
    }
}

/// Speichert eine Recherche als Recherche-Agent der Frage
async fn researched_as_agent(f: &DispatchFixture, id: &str, input: &ResearchInput) {
    f.backend
        .services
        .questions
        .add_research(id, input, &session(f, &format!("research-{id}")))
        .await
        .expect("Recherche speichern");
}

/// Kennung der Option mit diesem Titel in der neuesten Recherche
async fn option_id(f: &DispatchFixture, id: &str, title: &str) -> String {
    detail(f, id)
        .await
        .research
        .last()
        .expect("Recherche")
        .options
        .iter()
        .find(|option| option.title == title)
        .expect("Option")
        .id
        .clone()
}

#[tokio::test]
async fn autopilot_frage_eines_agents_wird_recherchiert_und_mit_der_empfehlung_beantwortet() {
    let f = DispatchFixture::create(autopilot(json!({ "wip": 2, "maxPerDay": 10 }))).await;
    let ticket = f.ready("Rabatte", TicketPatch::default()).await;
    let question = f.question(&ticket, "Brutto oder netto?").await;
    assert!(matches!(
        f.start(&ticket).await.expect("Start vor der Antwort"),
        StartOutcome::Skipped(_)
    ));

    let runner = ResearchRunner::new(&f.ctx);
    assert_eq!(
        runner.start_autopilot().await.expect("Autopilot").started,
        vec![question.clone()]
    );
    // Läuft schon: kein zweiter Start
    assert!(runner
        .start_autopilot()
        .await
        .expect("zweiter Durchlauf")
        .started
        .is_empty());
    let record = run_of(&f, &question);
    assert_eq!(record.run.profile, f.ctx.chooser().research().profile);

    researched_as_agent(&f, &question, &research_recommending(&["Netto"])).await;
    f.end(&question, dispatch_support::OK_OUTPUT, Some(0));
    assert_eq!(
        evaluate(&f, &record, None).await,
        ResearchOutcome::Researched
    );

    let answered = detail(&f, &question).await.view.question;
    assert_eq!(answered.state, QuestionState::Answered);
    let decision = answered.decision.expect("Entscheidung");
    assert_eq!(
        decision.option,
        Some(option_id(&f, &question, "Netto").await)
    );
    assert_eq!(
        decision.reason,
        "Autopilot: Empfehlung der Recherche übernommen"
    );
    assert!(is_autopilot_decision(&decision));
    assert!(f.comments(&ticket).await.contains(&format!(
        "Autopilot: Frage {question} mit „Netto“ entschieden"
    )));
    let state = f.ctx.stores.state.read().expect("Zustand");
    assert!(!state.autopilot_research.contains_key(&question));

    // Das Ticket ist wieder bereit, der nächste Arbeitslauf kennt die Antwort
    let StartOutcome::Started(work) = f.start(&ticket).await.expect("Start nach der Antwort")
    else {
        panic!("Ticket startet nach der Antwort nicht");
    };
    assert!(work.prompt.contains(&question), "{}", work.prompt);
    assert!(
        work.prompt
            .contains("Autopilot: Empfehlung der Recherche übernommen"),
        "{}",
        work.prompt
    );
}

#[tokio::test]
async fn autopilot_ohne_empfehlung_zweiter_versuch_mit_groesserem_profil_dann_blockiert() {
    let f = DispatchFixture::create(autopilot(json!({ "wip": 2, "maxPerDay": 10 }))).await;
    let ticket = f.ready("Rabatte", TicketPatch::default()).await;
    let question = f.question(&ticket, "Brutto oder netto?").await;
    let runner = ResearchRunner::new(&f.ctx);

    // Versuch 1: Recherche ohne Empfehlung
    runner.start_autopilot().await.expect("Versuch 1");
    let first = run_of(&f, &question);
    researched_as_agent(&f, &question, &research_recommending(&[])).await;
    f.end(&question, dispatch_support::OK_OUTPUT, Some(0));
    assert_eq!(
        evaluate(&f, &first, None).await,
        ResearchOutcome::Researched
    );
    assert_eq!(
        detail(&f, &question).await.view.question.state,
        QuestionState::Open
    );
    assert_eq!(
        f.ctx
            .stores
            .state
            .read()
            .expect("Zustand")
            .autopilot_research[&question],
        1
    );
    assert_eq!(f.ticket(&ticket).await.status, TicketStatus::Planned);

    // Versuch 2: nächste Stufe der Leiter, Lauf ohne Recherche (später, damit die Recherche
    // von Versuch 1 nicht als seine zählt)
    f.advance(10);
    assert_eq!(
        runner.start_autopilot().await.expect("Versuch 2").started,
        vec![question.clone()]
    );
    let second = run_of(&f, &question);
    assert_eq!(
        second.run.profile,
        f.ctx.chooser().research_attempt(2).profile
    );
    assert_ne!(second.run.profile, first.run.profile);
    f.end(&question, dispatch_support::OK_OUTPUT, Some(0));
    assert_eq!(evaluate(&f, &second, None).await, ResearchOutcome::Empty);

    let blocked = f.ticket(&ticket).await;
    let reason = format!("Autopilot: keine Empfehlung zu {question}");
    assert_eq!(blocked.status, TicketStatus::Blocked);
    assert_eq!(blocked.blocked_reason.as_deref(), Some(reason.as_str()));
    let notice = f.notices.last().expect("Meldung");
    assert_eq!(notice.kind, DispatchNoticeKind::Blocked);
    assert_eq!(notice.ticket_id.as_deref(), Some(ticket.as_str()));
    assert_eq!(notice.text, reason);
    assert_eq!(
        detail(&f, &question).await.view.question.state,
        QuestionState::Open
    );
    // Kein dritter Versuch
    assert!(runner
        .start_autopilot()
        .await
        .expect("danach")
        .started
        .is_empty());
}

#[tokio::test]
async fn fragen_von_menschen_interaktiven_sessions_und_ausnahme_tickets_bleiben_offen() {
    let f = DispatchFixture::create(autopilot(json!({ "wip": 5, "maxPerDay": 10 }))).await;
    let ticket = f.ready("Rabatte", TicketPatch::default()).await;
    let except = f
        .ready(
            "Interaktiv",
            TicketPatch {
                labels: Some(vec!["auto".into(), "interaktiv".into()]),
                ..TicketPatch::default()
            },
        )
        .await;
    let linked = std::slice::from_ref(&ticket);
    let by_human = ask_in(&f, "ARC", "Vom Menschen", &human(&f), linked).await;
    ask_in(
        &f,
        "ARC",
        "Aus einer Claude-Session",
        &session(&f, "claude"),
        linked,
    )
    .await;
    f.question(&except, "Zum Ausnahme-Ticket").await;

    let runner = ResearchRunner::new(&f.ctx);
    assert!(runner
        .start_autopilot()
        .await
        .expect("Autopilot")
        .started
        .is_empty());

    // Auch eine Recherche mit Empfehlung entscheidet die Frage eines Menschen nicht
    runner
        .start(&by_human, StartMode::Manual)
        .await
        .expect("von Hand");
    let record = run_of(&f, &by_human);
    researched_as_agent(&f, &by_human, &research_recommending(&["Netto"])).await;
    f.end(&by_human, dispatch_support::OK_OUTPUT, Some(0));
    assert_eq!(
        evaluate(&f, &record, None).await,
        ResearchOutcome::Researched
    );
    assert_eq!(
        detail(&f, &by_human).await.view.question.state,
        QuestionState::Open
    );
}

#[tokio::test]
async fn autopilot_frage_startet_trotz_tageslimit_aber_nur_im_wip_und_nur_mit_autopilot() {
    let f = DispatchFixture::create(json!({
        "autopilot": { "enabled": true },
        "research": { "auto": false, "wip": 1, "maxPerDay": 1 },
    }))
    .await;
    let today = ResearchRunner::day(f.ctx.now());
    f.ctx
        .stores
        .state
        .update(move |state| {
            state.research_auto.day = Some(today);
            state.research_auto.count = 1;
        })
        .await
        .expect("Tageslimit erreicht");
    let ticket = f.ready("Rabatte", TicketPatch::default()).await;
    let question = f.question(&ticket, "Brutto oder netto?").await;

    // Ohne Autopilot bleibt alles wie heute
    let mut off = f.context_with_active_board(Some(f.board.id));
    off.cfg.autopilot.enabled = false;
    assert!(ResearchRunner::new(&off)
        .start_autopilot()
        .await
        .expect("aus")
        .started
        .is_empty());

    // Der Durchlauf startet sie trotz research.auto aus und erreichtem Tageslimit
    let report = f.tick().await;
    assert_eq!(
        report.research,
        vec![question.clone()],
        "{:?}",
        report.errors
    );
    assert_eq!(
        f.ctx
            .stores
            .state
            .read()
            .expect("Zustand")
            .research_auto
            .count,
        1
    );

    // research.wip gilt weiter
    let other = f.ready("Lieferung", TicketPatch::default()).await;
    f.question(&other, "Spedition?").await;
    assert!(ResearchRunner::new(&f.ctx)
        .start_autopilot()
        .await
        .expect("voll")
        .started
        .is_empty());
}
