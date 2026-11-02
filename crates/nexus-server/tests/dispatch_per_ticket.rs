//! Start und Auswertung eines einzelnen Tickets; portiert aus
//! `packages/server/tests/unit/dispatch/DispatchPerTicket.test.ts` und den Start- bzw.
//! Auswertungsfällen aus `DispatchService.test.ts`.
//!
//! Die Fälle, die am Merge hängen (Merge-Auftrag ausführen, „Abnehmen“), stehen dort weiter:
//! den Merge bringt P4-15a, die Tick-Schleife P4-14c.

mod common;
mod dispatch_support;

use std::collections::HashSet;
use std::path::Path;

use nexus_core::board::basics::TicketStatus;
use nexus_core::board::ticket_service::{TicketPatch, WriteOptions};
use nexus_core::dispatch::{RunResult, SetupFailure, WorktreeSetupStep};
use nexus_server::dispatch::notifier::DispatchNoticeKind;
use nexus_server::dispatch::run_recorder::HandleOutcome;
use nexus_server::dispatch::ticket_starter::StartOutcome;
use serde_json::json;

use dispatch_support::{DispatchFixture, OK_OUTPUT};

/// Der Lauf des Tickets, wenn er gestartet wurde
fn started(outcome: StartOutcome) -> nexus_core::dispatch::RunRecord {
    match outcome {
        StartOutcome::Started(record) => *record,
        other => panic!("Start erwartet, nicht {other:?}"),
    }
}

/// Der Grund, warum das Ticket liegen blieb
fn skipped(outcome: StartOutcome) -> String {
    match outcome {
        StartOutcome::Skipped(reason) => reason,
        other => panic!("übersprungen erwartet, nicht {other:?}"),
    }
}

#[tokio::test]
async fn drei_bereite_tickets_derselben_spec_starten_jedes_im_eigenen_worktree() {
    let f = DispatchFixture::create(json!({})).await;
    let mut ids = Vec::new();
    for (title, task, touch) in [
        ("Erstes", "T001", "a"),
        ("Zweites", "T002", "b"),
        ("Drittes", "T003", "c"),
    ] {
        ids.push(
            f.ready(
                title,
                TicketPatch {
                    spec: Some(Some("001-x".into())),
                    spec_task: Some(Some(task.into())),
                    touches: Some(vec![touch.into()]),
                    ..TicketPatch::default()
                },
            )
            .await,
        );
    }
    for id in &ids {
        started(f.start(id).await.expect("Start"));
    }
    let mut branches = HashSet::new();
    let mut worktrees = HashSet::new();
    for id in &ids {
        let ticket = f.ticket(id).await;
        let branch = ticket.branch.clone().expect("Branch");
        let worktree = ticket.worktree.clone().expect("Worktree");
        assert_eq!(
            branch,
            format!("ticket/{}-{}", ticket.id, ticket.title.to_lowercase())
        );
        assert!(Path::new(&worktree).exists(), "{worktree} liegt auf Platte");
        branches.insert(branch);
        worktrees.insert(worktree);
    }
    assert_eq!(branches.len(), 3, "jedes Ticket auf eigenem Branch");
    assert_eq!(worktrees.len(), 3, "jedes Ticket im eigenen Worktree");
}

#[tokio::test]
async fn der_start_kommentar_nennt_agent_branch_profil_und_grund() {
    let f = DispatchFixture::create(json!({})).await;
    let id = f.ready("Kommentar", TicketPatch::default()).await;
    let record = started(f.start(&id).await.expect("Start"));
    let comments = f.comments(&id).await;
    let start = comments
        .iter()
        .find(|body| body.starts_with("[dispatch] Start"))
        .expect("Start-Kommentar");
    let run = &record.run;
    let expected = format!(
        "[dispatch] Start: an junior-{id} übergeben – Branch {}, Profil {} (Modell {}, \
Effort {}, max. {} Schritte, {} min). Grund: {}.",
        record.branch,
        run.profile,
        run.model.as_deref().unwrap_or("Standard"),
        run.effort.as_deref().unwrap_or("Standard"),
        run.max_turns,
        run.timeout_minutes,
        run.why,
    );
    assert_eq!(start, &expected);
    // Dasselbe noch einmal wortgleich, damit auffällt, wenn sich die Vorlage ändert: das
    // Standardprofil der Beispiel-Einstellung
    assert_eq!(
        start,
        &format!(
            "[dispatch] Start: an junior-{id} übergeben – Branch ticket/{id}-kommentar, \
Profil standard (Modell opus, Effort high, max. 80 Schritte, 90 min). Grund: Standardprofil \
standard."
        )
    );
}

#[tokio::test]
async fn ein_ticket_ohne_plan_bleibt_liegen() {
    let f = DispatchFixture::create(json!({})).await;
    let id = f.ready("Ohne Plan", TicketPatch::default()).await;
    f.backend
        .services
        .tickets
        .update(
            &id,
            &TicketPatch {
                plan: Some(String::new()),
                ..TicketPatch::default()
            },
            &f.actor(),
            &WriteOptions::default(),
        )
        .await
        .expect("Plan leeren");
    assert_eq!(skipped(f.start(&id).await.expect("Start")), "Kein Plan");
    assert!(f.ctx.stores.runs.list().expect("Läufe").is_empty());
}

#[tokio::test]
async fn ein_belegter_worktree_laesst_das_ticket_ungeclaimt_warten() {
    // Gebündelt: beide Tickets derselben Spec würden denselben Worktree benutzen
    let f = DispatchFixture::create(json!({ "joinWorktrees": "sameSpec" })).await;
    let first = f
        .ready(
            "Erstes",
            TicketPatch {
                spec: Some(Some("001-x".into())),
                spec_task: Some(Some("T001".into())),
                ..TicketPatch::default()
            },
        )
        .await;
    let second = f
        .ready(
            "Zweites",
            TicketPatch {
                spec: Some(Some("001-x".into())),
                spec_task: Some(Some("T002".into())),
                ..TicketPatch::default()
            },
        )
        .await;
    started(f.start(&first).await.expect("Start"));
    assert_eq!(
        skipped(f.start(&second).await.expect("Start")),
        format!("wartet – Worktree belegt durch {first}")
    );
    // Ungeclaimt: das Ticket bleibt Geplant und ohne Zuweisung
    let ticket = f.ticket(&second).await;
    assert_eq!(ticket.status, TicketStatus::Planned);
    assert_eq!(ticket.assignee, None);
}

#[tokio::test]
async fn ein_ticket_mit_dem_worktree_eines_anderen_startet_auf_eigenem_branch() {
    let f = DispatchFixture::create(json!({})).await;
    let host = f
        .ready(
            "Fremd",
            TicketPatch {
                labels: Some(Vec::new()),
                ..TicketPatch::default()
            },
        )
        .await;
    let id = f.ready("Verirrt", TicketPatch::default()).await;
    let work = f.work().await;
    let foreign = work
        .manager
        .start(
            &host,
            &nexus_core::worktrees::manager::StartOptions::default(),
            f.repo.path(),
            &f.actor(),
        )
        .await
        .expect("fremder Worktree");
    f.repo
        .run_git(&["worktree", "remove", "--force", &foreign.path]);
    f.backend
        .services
        .tickets
        .update(
            &id,
            &TicketPatch {
                branch: Some(Some(foreign.branch.clone())),
                worktree: Some(Some(foreign.path.clone())),
                ..TicketPatch::default()
            },
            &f.actor(),
            &WriteOptions::default(),
        )
        .await
        .expect("Worktree eintragen");
    started(f.start(&id).await.expect("Start"));
    assert!(!Path::new(&foreign.path).exists());
    let ticket = f.ticket(&id).await;
    assert_eq!(ticket.status, TicketStatus::InProgress);
    assert_eq!(
        ticket.branch.as_deref(),
        Some(&*format!("ticket/{id}-verirrt"))
    );
    assert_ne!(ticket.worktree.as_deref(), Some(foreign.path.as_str()));
    let comments = f.comments(&id).await;
    let start = comments
        .iter()
        .find(|body| body.starts_with("[dispatch] Start"))
        .expect("Start-Kommentar");
    assert!(
        start.contains(&format!(
            "Hinweis: Der eingetragene Worktree {} (Branch {}) gehört {host}",
            foreign.path, foreign.branch
        )),
        "{start}"
    );
}

#[tokio::test]
async fn einrichtung_nur_im_neuen_worktree_und_nach_erfolg_nicht_erneut() {
    let steps = json!([{ "cwd": "app", "run": "npm ci --prefer-offline --no-audit" }]);
    let f = DispatchFixture::create(
        json!({ "worktreeSetup": steps, "worktreeSetupTimeoutMinutes": 7 }),
    )
    .await;
    let id = f.ready("Einrichten", TicketPatch::default()).await;
    let first = started(f.start(&id).await.expect("Start"));
    assert_eq!(
        first.setup,
        Some(vec![WorktreeSetupStep {
            cwd: "app".into(),
            run: "npm ci --prefer-offline --no-audit".into(),
        }])
    );
    assert_eq!(first.setup_timeout_minutes, Some(7));
    assert!(first
        .prompt
        .contains("laut Einrichtung des Dispatchers installiert"));
    // Einrichtung gelungen, Claude bricht ab → Fortsetzung ohne Einrichtung
    f.end_with(
        &id,
        RunResult {
            code: Some(1),
            output: "x".into(),
            setup_ok: Some(true),
            ..RunResult::default()
        },
    );
    assert_eq!(
        f.evaluate(&id).await.expect("Auswertung").outcome,
        HandleOutcome::Released
    );
    f.advance(1);
    let work = f.work().await;
    assert!(!work
        .manager
        .setup_pending(&first.branch)
        .expect("Einrichtung"));
    let second = started(f.start(&id).await.expect("Start"));
    assert_ne!(second.id, first.id);
    assert_eq!(second.setup, None);
}

#[tokio::test]
async fn gescheiterte_einrichtung_blockiert_ohne_versuch_und_richtet_danach_erneut_ein() {
    let steps = json!([{ "cwd": ".", "run": "npm ci" }]);
    let f = DispatchFixture::create(json!({ "worktreeSetup": steps })).await;
    let id = f.ready("Kaputt", TicketPatch::default()).await;
    let first = started(f.start(&id).await.expect("Start"));
    f.end_with(
        &id,
        RunResult {
            code: Some(1),
            output: "npm ERR! kaputt".into(),
            setup_failed: Some(SetupFailure {
                cwd: ".".into(),
                run: "npm ci".into(),
                code: Some(1),
                timed_out: false,
                output: "npm ERR! kaputt".into(),
            }),
            ..RunResult::default()
        },
    );
    assert_eq!(
        f.evaluate(&id).await.expect("Auswertung").outcome,
        HandleOutcome::SetupFailed
    );
    let ticket = f.ticket(&id).await;
    assert_eq!(ticket.status, TicketStatus::Blocked);
    assert_eq!(
        ticket.blocked_reason.as_deref(),
        Some("Einrichtung fehlgeschlagen: .$ npm ci (Exit 1)")
    );
    assert_eq!(ticket.assignee, None);
    assert_eq!(ticket.branch.as_deref(), Some(first.branch.as_str()));
    let comments = f.comments(&id).await;
    let last = comments.last().expect("Kommentar");
    assert!(
        last.starts_with("[dispatch] Einrichtung fehlgeschlagen: .$ npm ci (Exit 1)"),
        "{last}"
    );
    assert!(last.contains("npm ERR! kaputt"), "{last}");
    assert!(
        !comments
            .iter()
            .any(|body| body.starts_with("[dispatch] Abbruch")),
        "kein Abbruch-Kommentar: {comments:?}"
    );
    let notice = f.notices.last().expect("Meldung");
    assert_eq!(notice.kind, DispatchNoticeKind::Blocked);
    assert_eq!(notice.ticket_id.as_deref(), Some(id.as_str()));
    // Nach dem Beheben wieder Geplant: Fortsetzung im selben Worktree mit erneuter Einrichtung
    f.backend
        .services
        .tickets
        .update(
            &id,
            &TicketPatch {
                status: Some(TicketStatus::Planned),
                ..TicketPatch::default()
            },
            &f.actor(),
            &WriteOptions {
                force: true,
                ..WriteOptions::default()
            },
        )
        .await
        .expect("wieder Geplant");
    f.advance(1);
    let second = started(f.start(&id).await.expect("Start"));
    assert_eq!(second.branch, first.branch);
    assert_eq!(second.cwd, first.cwd);
    assert_eq!(
        second.setup,
        Some(vec![WorktreeSetupStep {
            cwd: ".".into(),
            run: "npm ci".into(),
        }])
    );
}

#[tokio::test]
async fn ein_erledigtes_ticket_wird_fuer_den_merge_vorgemerkt() {
    let f = DispatchFixture::create(json!({})).await;
    let id = f.ready("Fertig", TicketPatch::default()).await;
    let record = started(f.start(&id).await.expect("Start"));
    f.backend
        .services
        .tickets
        .move_to(
            &id,
            TicketStatus::Done,
            &f.agent(&id),
            &nexus_core::board::ticket_service::MoveOptions::default(),
        )
        .await
        .expect("erledigt melden");
    f.end(&id, OK_OUTPUT, Some(0));
    assert_eq!(
        f.evaluate(&id).await.expect("Auswertung").outcome,
        HandleOutcome::Done
    );
    let state = f.ctx.stores.state.read().expect("Zustand");
    let merge = state.merges.get(&id).expect("Merge-Auftrag");
    assert_eq!(merge.branch, record.branch);
    assert_eq!(merge.phase, nexus_core::dispatch::MergePhase::Pending);
    assert!(!merge.session_used);
}

#[tokio::test]
async fn ein_ticket_auf_blockiert_meldet_den_grund_und_zaehlt_keinen_abbruch() {
    let f = DispatchFixture::create(json!({})).await;
    let id = f.ready("Human", TicketPatch::default()).await;
    started(f.start(&id).await.expect("Start"));
    f.backend
        .services
        .tickets
        .update(
            &id,
            &TicketPatch {
                status: Some(TicketStatus::Blocked),
                blocked_reason: Some(Some("Human Interaction: Oberfläche ansehen".into())),
                ..TicketPatch::default()
            },
            &f.agent(&id),
            &WriteOptions::default(),
        )
        .await
        .expect("blockieren");
    f.end(&id, OK_OUTPUT, Some(0));
    let result = f.evaluate(&id).await.expect("Auswertung");
    assert_eq!(result.outcome, HandleOutcome::Blocked);
    assert_eq!(
        result.blocked_reason.as_deref(),
        Some("Human Interaction: Oberfläche ansehen")
    );
    assert_eq!(
        f.notices.last().expect("Meldung").text,
        "Human Interaction: Oberfläche ansehen"
    );
    assert!(!f
        .comments(&id)
        .await
        .iter()
        .any(|body| body.starts_with("[dispatch] Abbruch")));
}

#[tokio::test]
async fn ein_ticket_mit_offener_frage_geht_zurueck_nach_geplant_und_behaelt_den_worktree() {
    let f = DispatchFixture::create(json!({})).await;
    let id = f.ready("Fragt", TicketPatch::default()).await;
    let record = started(f.start(&id).await.expect("Start"));
    let question = f.question(&id, "Welche Variante?").await;
    f.end(&id, OK_OUTPUT, Some(0));
    assert_eq!(
        f.evaluate(&id).await.expect("Auswertung").outcome,
        HandleOutcome::Question
    );
    let ticket = f.ticket(&id).await;
    assert_eq!(ticket.status, TicketStatus::Planned);
    assert_eq!(ticket.assignee, None);
    assert_eq!(ticket.branch.as_deref(), Some(record.branch.as_str()));
    let comments = f.comments(&id).await;
    let last = comments.last().expect("Kommentar");
    assert_eq!(
        last,
        &format!(
            "[dispatch] Frage: wartet auf {} – Ticket zurück nach Geplant, Branch {} und \
Worktree bleiben. Nach der Entscheidung geht es dort weiter.",
            question, record.branch
        )
    );
}

#[tokio::test]
async fn ein_lauf_ohne_ausgang_bricht_ab_und_geht_zurueck_nach_geplant() {
    let f = DispatchFixture::create(json!({})).await;
    let id = f.ready("Ohne Ausgang", TicketPatch::default()).await;
    let record = started(f.start(&id).await.expect("Start"));
    f.end(&id, "Fehlerausgabe", Some(2));
    let result = f.evaluate(&id).await.expect("Auswertung");
    assert_eq!(result.outcome, HandleOutcome::Released);
    let abort = result.abort.expect("Abbruch");
    assert_eq!(abort.reason, "Claude endete mit Exit-Code 2");
    assert!(!abort.final_attempt);
    let ticket = f.ticket(&id).await;
    assert_eq!(ticket.status, TicketStatus::Planned);
    assert_eq!(ticket.assignee, None);
    let comments = f.comments(&id).await;
    let last = comments.last().expect("Kommentar");
    assert!(
        last.starts_with(&format!(
            "[dispatch] Abbruch Versuch 1/{}: Claude endete mit Exit-Code 2.\nDas Ticket geht \
zurück nach Geplant.",
            f.cfg.max_attempts
        )),
        "{last}"
    );
    assert!(last.contains("Fehlerausgabe"), "{last}");
    assert!(
        last.contains(&format!(
            "Protokoll: {}",
            Path::new(&record.log_file)
                .file_name()
                .expect("Dateiname")
                .to_string_lossy()
        )),
        "{last}"
    );
}

#[tokio::test]
async fn der_letzte_versuch_blockiert_das_ticket_mit_grund() {
    let f = DispatchFixture::create(json!({ "maxAttempts": 1 })).await;
    let id = f.ready("Hoffnungslos", TicketPatch::default()).await;
    started(f.start(&id).await.expect("Start"));
    f.end(&id, "Fehlerausgabe", Some(1));
    let result = f.evaluate(&id).await.expect("Auswertung");
    assert_eq!(result.outcome, HandleOutcome::Blocked);
    assert!(result.abort.expect("Abbruch").final_attempt);
    let ticket = f.ticket(&id).await;
    assert_eq!(ticket.status, TicketStatus::Blocked);
    assert_eq!(
        ticket.blocked_reason.as_deref(),
        Some("Automatische Umsetzung 1× abgebrochen – bitte ansehen")
    );
    assert_eq!(ticket.assignee, None);
    assert!(f
        .comments(&id)
        .await
        .last()
        .expect("Kommentar")
        .contains("Das war der letzte automatische Versuch – das Ticket geht nach Blockiert."));
}

#[tokio::test]
async fn ein_nutzungslimit_pausiert_ohne_versuch() {
    let f = DispatchFixture::create(json!({})).await;
    let id = f.ready("Limit", TicketPatch::default()).await;
    started(f.start(&id).await.expect("Start"));
    f.end(&id, "Claude usage limit reached", Some(1));
    assert_eq!(
        f.evaluate(&id).await.expect("Auswertung").outcome,
        HandleOutcome::Limit
    );
    let ticket = f.ticket(&id).await;
    assert_eq!(ticket.status, TicketStatus::Planned);
    let state = f.ctx.stores.state.read().expect("Zustand");
    let until = state.paused_until.clone().expect("Pause vermerkt");
    // Ohne mehrere Accounts pausiert der ganze Dispatcher bis zum Ende der Account-Pause
    let bis = nexus_core::dispatch::hhmm(&until);
    let comments = f.comments(&id).await;
    assert_eq!(
        comments.last().expect("Kommentar"),
        &format!(
            "[dispatch] Pause: Nutzungslimit erreicht, Ticket zurück nach Geplant. Der \
Dispatcher startet bis {bis} nichts Neues. Zählt nicht als Versuch."
        )
    );
    assert_eq!(
        state.account_pauses.get("standard").map(String::as_str),
        Some(until.as_str()),
        "die Pause gilt auch für den Standard-Account"
    );
    assert_eq!(
        f.notices.last().expect("Meldung").kind,
        DispatchNoticeKind::Paused
    );
    // Eine Limit-Pause zählt nicht als Versuch: der nächste Abbruch ist Versuch 1
    f.advance(1);
    started(f.start(&id).await.expect("Start"));
    f.end(&id, "Fehlerausgabe", Some(1));
    f.evaluate(&id).await.expect("Auswertung");
    assert!(f
        .comments(&id)
        .await
        .iter()
        .any(|body| body.starts_with("[dispatch] Abbruch Versuch 1/")));
}

#[tokio::test]
async fn fehlt_claude_code_pausiert_der_dispatcher_ohne_versuch() {
    let f = DispatchFixture::create(json!({})).await;
    let id = f.ready("Kein Claude", TicketPatch::default()).await;
    let record = started(f.start(&id).await.expect("Start"));
    f.end(&id, "claude-not-found", Some(127));
    assert_eq!(
        f.evaluate(&id).await.expect("Auswertung").outcome,
        HandleOutcome::ClaudeMissing
    );
    let minutes = f.cfg.pause_minutes_on_limit;
    assert_eq!(
        f.comments(&id).await.last().expect("Kommentar"),
        &format!(
            "[dispatch] Pause: Claude Code wurde nicht gefunden (Befehl „{}“), Ticket zurück \
nach Geplant. Der Dispatcher startet {minutes} min lang nichts Neues. Bitte Claude Code \
installieren oder den Befehl in der Einstellung prüfen. Zählt nicht als Versuch.",
            record.command
        )
    );
}

#[tokio::test]
async fn ein_geloeschtes_ticket_gilt_als_verschwunden() {
    let f = DispatchFixture::create(json!({})).await;
    let id = f.ready("Weg", TicketPatch::default()).await;
    started(f.start(&id).await.expect("Start"));
    f.end(&id, OK_OUTPUT, Some(0));
    let record = f.run(&id);
    let end = f
        .ctx
        .stores
        .runs
        .result(&record.id)
        .expect("Ergebnis")
        .expect("Ergebnis");
    f.backend
        .services
        .tickets
        .delete(&id, &f.actor())
        .await
        .expect("löschen");
    let results = nexus_server::dispatch::result_handler::ResultHandler::without_gates(&f.ctx);
    assert_eq!(
        results
            .handle(&record, &end, None)
            .await
            .expect("Auswertung")
            .outcome,
        HandleOutcome::Gone
    );
}

#[tokio::test]
async fn ein_von_hand_uebernommenes_ticket_gilt_als_vergeben() {
    let f = DispatchFixture::create(json!({})).await;
    let id = f.ready("Übernommen", TicketPatch::default()).await;
    started(f.start(&id).await.expect("Start"));
    f.backend
        .services
        .tickets
        .claim(
            &id,
            "Florian",
            &f.actor(),
            &nexus_core::board::ticket_service::ClaimOptions {
                force: true,
                ..Default::default()
            },
        )
        .await
        .expect("übernehmen");
    f.end(&id, OK_OUTPUT, Some(0));
    assert_eq!(
        f.evaluate(&id).await.expect("Auswertung").outcome,
        HandleOutcome::Taken
    );
}

#[tokio::test]
async fn ein_gescheiterter_start_zaehlt_als_abbruch_versuch() {
    let f = DispatchFixture::create(json!({})).await;
    let id = f.ready("Startet nicht", TicketPatch::default()).await;
    f.launcher.fail_with("claude fehlt");
    let outcome = f.start(&id).await.expect("Start");
    let StartOutcome::Error(message) = &outcome else {
        panic!("Fehler erwartet, nicht {outcome:?}");
    };
    assert!(message.contains("claude fehlt"), "{message}");
    // Kein Lauf bleibt zurück, aber eine Zeile für die Auswertung
    assert!(f.ctx.stores.runs.list().expect("Läufe").is_empty());
    let kept = f.kept();
    assert_eq!(kept.len(), 1);
    let row = &kept[0].1;
    assert_eq!(row.abort_category.as_deref(), Some("start-failed"));
    assert!(row
        .abort_reason
        .as_deref()
        .expect("Grund")
        .starts_with("Worker konnte nicht gestartet werden: "));
    let ticket = f.ticket(&id).await;
    assert_eq!(ticket.status, TicketStatus::Planned);
    assert_eq!(ticket.assignee, None);
    assert!(f
        .comments(&id)
        .await
        .iter()
        .any(|body| body.starts_with("[dispatch] Abbruch Versuch 1/")));
}
