//! Der Durchlauf des Dispatchers als Ganzes: WIP-Limit, Zeitfenster, Pause, verschwundene
//! Läufe, hängende Übernahmen, Zustand und Lauf-Listen. Portiert aus den Tick-Fällen von
//! `packages/server/tests/unit/dispatch/DispatchService.test.ts`.
//!
//! Start und Auswertung eines einzelnen Tickets stehen in `dispatch_per_ticket.rs`; die Fälle
//! am Merge-Auftrag bringt P4-15b.

mod common;
mod dispatch_support;

use nexus_core::board::basics::TicketStatus;
use nexus_core::board::config_store::BoardConfigStore;
use nexus_core::board::ticket_service::{ClaimOptions, TicketPatch};
use nexus_core::dispatch::DispatchConfigStore;
use nexus_server::dispatch::status::DispatchStateKind;
use serde_json::json;

use dispatch_support::{DispatchFixture, OK_OUTPUT};

/// Ein Ticket mit Titel und sonst dem Standard
async fn ready(f: &DispatchFixture, title: &str) -> String {
    f.ready(title, TicketPatch::default()).await
}

/// Die Ticket-Kennungen der ausgewerteten Läufe samt Ausgang
fn evaluated(report: &nexus_server::dispatch::service::DispatchReport) -> Vec<(String, String)> {
    report
        .evaluated
        .iter()
        .map(|run| (run.ticket_id.clone(), run.outcome.clone()))
        .collect()
}

#[tokio::test]
async fn ein_durchlauf_startet_ein_bereites_ticket_und_haelt_das_wip_limit_ein() {
    let f = DispatchFixture::create(json!({ "wip": 1 })).await;
    let first = f
        .ready(
            "Erstes",
            TicketPatch {
                priority: Some(nexus_core::board::basics::Priority::High),
                ..TicketPatch::default()
            },
        )
        .await;
    let second = ready(&f, "Zweites").await;
    let manual = f
        .ready(
            "Ohne Label",
            TicketPatch {
                labels: Some(Vec::new()),
                ..TicketPatch::default()
            },
        )
        .await;

    let report = f.tick().await;
    assert_eq!(report.started, vec![first.clone()]);
    assert_eq!(report.queued, vec![second.clone()]);
    assert_eq!(report.errors, Vec::<String>::new());
    let ticket = f.ticket(&first).await;
    assert_eq!(ticket.status, TicketStatus::InProgress);
    assert_eq!(
        ticket.assignee.as_deref(),
        Some(&*format!("junior-{first}"))
    );

    let status = f.service.status(Some(f.board.id)).await.expect("Zustand");
    assert!(status.enabled);
    assert_eq!(status.state, DispatchStateKind::Running);
    assert_eq!((status.active, status.wip), (1, 1));
    assert!(status.board_in_scope);
    assert_eq!(status.message, "läuft · 1/1");
    assert_eq!(status.runs.len(), 1);
    assert_eq!(status.runs[0].ticket_id, first);
    let rows: Vec<(String, String)> = status
        .tickets
        .iter()
        .map(|row| (row.id.clone(), row.reason.clone()))
        .collect();
    assert_eq!(
        rows,
        vec![
            (second, "bereit – wartet auf freien Platz".to_string()),
            (manual, "Label „auto“ fehlt".to_string()),
        ]
    );
}

#[tokio::test]
async fn startet_nichts_wenn_ausgeschaltet_pausiert_oder_ausserhalb_des_zeitfensters() {
    let f = DispatchFixture::create(json!({ "enabled": false })).await;
    ready(&f, "Erstes").await;
    assert_eq!(
        f.tick().await.idle.as_deref(),
        Some("Dispatcher ist ausgeschaltet")
    );
    assert_eq!(
        f.service.status(None).await.expect("Zustand").state,
        DispatchStateKind::Off
    );

    f.service.set_enabled(true).await.expect("einschalten");
    f.service.pause(30).await.expect("pausieren");
    let idle = f.tick().await.idle.expect("Grund für den Stillstand");
    assert!(idle.starts_with("Pausiert bis "), "{idle}");
    let status = f.service.status(None).await.expect("Zustand");
    assert_eq!(status.state, DispatchStateKind::Paused);
    assert!(status.paused_until.is_some());
    assert!(
        status.message.starts_with("pausiert bis "),
        "{}",
        status.message
    );

    f.service.resume().await.expect("Pause aufheben");
    // Mittags, bei einem Fenster von 18:00 bis 07:00 (über Mitternacht): draußen
    f.set_local_time(12, 0);
    f.write_config(json!({ "window": { "from": "18:00", "to": "07:00" } }));
    assert_eq!(
        f.tick().await.idle.as_deref(),
        Some("Außerhalb des Zeitfensters 18:00–07:00")
    );
    let status = f.service.status(None).await.expect("Zustand");
    assert_eq!(status.state, DispatchStateKind::Outside);
    assert_eq!(status.message, "außerhalb 18:00–07:00");

    // Abends im Fenster startet der Durchlauf wieder
    f.set_local_time(19, 0);
    assert_eq!(f.tick().await.idle, None);
}

#[tokio::test]
async fn gibt_tickets_zurueck_deren_worker_ohne_ergebnis_verschwunden_ist() {
    let f = DispatchFixture::create(json!({})).await;
    let id = ready(&f, "Absturz").await;
    assert_eq!(f.tick().await.started, vec![id.clone()]);

    f.set_alive(false);
    let report = f.tick().await;
    assert_eq!(evaluated(&report), vec![(id.clone(), "released".into())]);
    let abort = f
        .comments(&id)
        .await
        .into_iter()
        .find(|body| body.starts_with("[dispatch] Abbruch"))
        .expect("Abbruch-Kommentar");
    assert!(
        abort.contains("Worker-Prozess endete ohne Ergebnis"),
        "{abort}"
    );
}

#[tokio::test]
async fn ein_lauf_weit_ueber_dem_zeitlimit_gilt_als_verschwunden_auch_wenn_die_pid_lebt() {
    let f = DispatchFixture::create(json!({})).await;
    let id = ready(&f, "Hänger").await;
    f.tick().await;
    let record = f.run(&id);

    // Noch innerhalb der Nachfrist von 10 min: der Lauf bleibt
    f.advance(record.run.timeout_minutes + 9);
    assert_eq!(evaluated(&f.tick().await), Vec::new());
    f.advance(2);
    let report = f.tick().await;
    assert_eq!(evaluated(&report), vec![(id.clone(), "released".into())]);
    let abort = f
        .comments(&id)
        .await
        .into_iter()
        .find(|body| body.starts_with("[dispatch] Abbruch"))
        .expect("Abbruch-Kommentar");
    assert!(
        abort.contains("nach dem Zeitlimit noch kein Ergebnis"),
        "{abort}"
    );
}

#[tokio::test]
async fn gibt_eine_haengende_uebernahme_nach_fuenf_minuten_frei_ohne_versuch_zu_zaehlen() {
    let f = DispatchFixture::create(json!({ "wip": 0 })).await;
    let id = ready(&f, "Hängt").await;
    f.backend
        .services
        .tickets
        .claim(
            &id,
            &format!("junior-{id}"),
            &f.agent(&id),
            &ClaimOptions {
                status: Some(TicketStatus::Planned),
                ..ClaimOptions::default()
            },
        )
        .await
        .expect("übernehmen");

    f.advance(4);
    f.tick().await;
    assert_eq!(
        f.ticket(&id).await.assignee.as_deref(),
        Some(&*format!("junior-{id}")),
        "nach 4 min hängt die Übernahme noch nicht lange genug"
    );

    f.advance(2);
    let report = f.tick().await;
    let ticket = f.ticket(&id).await;
    assert_eq!(ticket.status, TicketStatus::Planned);
    assert_eq!(ticket.assignee, None);
    assert_eq!(report.queued, vec![id.clone()]);
    assert!(
        !f.comments(&id)
            .await
            .iter()
            .any(|body| body.starts_with("[dispatch] Abbruch")),
        "eine hängende Übernahme zählt nicht als Versuch"
    );
}

#[tokio::test]
async fn gibt_ein_agent_ticket_ohne_lebenszeichen_nach_der_eingestellten_zeit_zurueck() {
    let f = DispatchFixture::create(json!({})).await;
    let id = ready(&f, "Kein Lebenszeichen").await;
    f.tick().await;
    // Lauf-Datei verloren: das Ticket hängt beim Agenten, ohne dass ein Lauf davon weiß
    f.drop_all_runs();
    f.write_config(json!({ "wip": 0 }));

    f.advance(f.cfg.stale_minutes + 1);
    let report = f.tick().await;
    assert_eq!(
        evaluated(&report),
        vec![(id.clone(), "released".into())],
        "der erste Abbruch gibt das Ticket zurück; blockiert wird erst der letzte Versuch"
    );
    let abort = f
        .comments(&id)
        .await
        .into_iter()
        .rfind(|body| body.starts_with("[dispatch] Abbruch"))
        .expect("Abbruch-Kommentar");
    assert!(
        abort.contains(&format!(
            "Kein Lebenszeichen seit mehr als {} min",
            f.cfg.stale_minutes
        )),
        "{abort}"
    );
}

#[tokio::test]
async fn laesst_tickets_auf_fremden_boards_in_ruhe() {
    let f = DispatchFixture::create(json!({})).await;
    let id = ready(&f, "Fremd").await;
    f.tick().await;
    f.drop_all_runs();
    // Ein anderer Benutzer ist am Board angemeldet: das Board gehört ihm nicht mehr
    BoardConfigStore::new(f.home.clone())
        .write(&nexus_core::board::config::BoardConfig {
            user_id: "22222222-2222-2222-2222-222222222222".into(),
            active_board: Some(f.board.id),
            ..common::memory_config()
        })
        .expect("board.yaml schreiben");

    f.advance(f.cfg.stale_minutes + 50);
    assert_eq!(evaluated(&f.tick().await), Vec::new());
    let ticket = f.ticket(&id).await;
    assert_eq!(ticket.status, TicketStatus::InProgress);
    assert_eq!(ticket.assignee.as_deref(), Some(&*format!("junior-{id}")));
}

#[tokio::test]
async fn ein_verbindungsfehler_bricht_den_durchlauf_ab_und_laesst_den_lauf_liegen() {
    let f = DispatchFixture::create(json!({})).await;
    let id = ready(&f, "Netz").await;
    f.tick().await;
    f.end(&id, "x", Some(1));

    f.backend.break_connection();
    let report = f.tick().await;
    assert_eq!(evaluated(&report), Vec::new());
    assert_eq!(
        report.errors,
        vec!["Verbindung zur Board-Datenbank verloren: Datenbank weg".to_string()]
    );
    assert_eq!(
        f.ctx.stores.runs.list().expect("Läufe").len(),
        1,
        "der Lauf bleibt für den nächsten Durchlauf"
    );

    f.backend.mend_connection();
    assert_eq!(
        evaluated(&f.tick().await),
        vec![(id, "released".into())],
        "nach der Datenbank wird der Lauf ausgewertet"
    );
}

#[tokio::test]
async fn ein_durchlauf_ohne_arbeit_braucht_keine_datenbank() {
    let f = DispatchFixture::create(json!({ "enabled": false })).await;
    f.backend.break_connection();
    let report = f.tick().await;
    assert_eq!(
        report.idle.as_deref(),
        Some("Dispatcher ist ausgeschaltet"),
        "ausgeschaltet und ohne offene Läufe gibt es nichts zu holen"
    );
    assert_eq!(report.errors, Vec::<String>::new());
}

#[tokio::test]
async fn zwei_gleichzeitige_durchlaeufe_teilen_sich_ein_ergebnis() {
    let f = DispatchFixture::create(json!({})).await;
    let id = ready(&f, "Nur einmal").await;

    // Eine Arbeit von Hand hält die Sperre, damit beide Durchläufe sich wirklich überlappen
    let (open, gate) = tokio::sync::oneshot::channel::<()>();
    let (holds, held) = tokio::sync::oneshot::channel::<()>();
    let service = f.service.clone();
    let holding = tokio::spawn(async move {
        service
            .exclusive(|| async {
                let _ = holds.send(());
                let _ = gate.await;
            })
            .await;
    });
    held.await.expect("die Sperre ist genommen");

    let first = tokio::spawn({
        let service = f.service.clone();
        async move { service.tick().await }
    });
    // Der erste Aufruf muss sich eingetragen haben, bevor der zweite dazukommt
    tokio::task::yield_now().await;
    let second = tokio::spawn({
        let service = f.service.clone();
        async move { service.tick().await }
    });
    tokio::task::yield_now().await;

    let _ = open.send(());
    holding.await.expect("Arbeit von Hand");
    let first = first.await.expect("erster Durchlauf");
    let second = second.await.expect("zweiter Durchlauf");

    assert_eq!(first, second, "beide Aufrufe bekommen denselben Bericht");
    // Der zweite Aufruf startet das Ticket nicht ein zweites Mal
    assert_eq!(first.started, vec![id]);
    assert_eq!(f.launcher.launched().len(), 1);
}

#[tokio::test]
async fn ein_durchlauf_merkt_sich_seinen_bericht_fuer_den_zustand() {
    let f = DispatchFixture::create(json!({})).await;
    assert!(f.service.last_report().is_none());
    let report = f.tick().await;
    assert_eq!(f.service.last_report().as_ref(), Some(&report));
    let status = f.service.status(None).await.expect("Zustand");
    assert_eq!(status.last_tick.as_deref(), Some(&*report.at));
    assert_eq!(status.last_errors, report.errors);
}

#[tokio::test]
async fn laeufe_eines_tickets_und_der_text_eines_lauf_protokolls() {
    let f = DispatchFixture::create(json!({})).await;
    let id = ready(&f, "Protokoll").await;
    f.tick().await;
    let record = f.run(&id);

    // Das Protokoll schreibt im Betrieb der Worker-Prozess; die Attrappe startet keinen
    let name = f.write_log(&record.id, "Du bist junior-…", "fertig");

    let runs = f.service.ticket_runs(&id).await.expect("Läufe des Tickets");
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].name, name);
    assert!(runs[0].running, "ohne Ergebnis läuft der Worker noch");

    // Mit Ergebnis gilt der Lauf nicht mehr als laufend
    f.end(&id, OK_OUTPUT, Some(0));
    let runs = f.service.ticket_runs(&id).await.expect("Läufe des Tickets");
    assert!(!runs[0].running);

    let text = f.service.run_log(&runs[0].name).await.expect("Protokoll");
    assert_eq!(text.name, runs[0].name);
    assert!(
        text.parts.prompt.is_some(),
        "der Kopf des Protokolls trennt die Arbeitsanweisung ab"
    );
}

#[tokio::test]
async fn pause_und_resume_schreiben_den_zustand_und_lehnen_arbeiten_von_hand_ab() {
    let f = DispatchFixture::create(json!({})).await;
    f.service.pause(45).await.expect("pausieren");
    let state = f.ctx.stores.state.read().expect("Zustand");
    assert!(state.paused_until.is_some());

    let error = f
        .service
        .start_planning("ARC-0001")
        .await
        .expect_err("Planung von Hand bei Pause");
    assert_eq!(error.code(), Some("dispatch-paused"));
    let error = f
        .service
        .start_research("ARC-F0001")
        .await
        .expect_err("Recherche von Hand bei Pause");
    assert_eq!(error.code(), Some("dispatch-paused"));

    f.service.resume().await.expect("Pause aufheben");
    assert_eq!(
        f.ctx.stores.state.read().expect("Zustand").paused_until,
        None
    );
    assert_eq!(
        f.service.status(None).await.expect("Zustand").paused_until,
        None
    );
}

#[tokio::test]
async fn set_enabled_schreibt_die_einstellung() {
    let f = DispatchFixture::create(json!({ "enabled": false })).await;
    assert!(!f.service.status(None).await.expect("Zustand").enabled);
    f.service.set_enabled(true).await.expect("einschalten");
    let store = DispatchConfigStore::new(f.home.clone());
    assert!(store.read().expect("Einstellung").config.enabled);
    assert!(f.service.status(None).await.expect("Zustand").enabled);
    f.service.set_enabled(false).await.expect("ausschalten");
    assert!(!store.read().expect("Einstellung").config.enabled);
}

#[tokio::test]
async fn eine_kaputte_einstellung_meldet_sich_im_zustand_und_im_bericht() {
    let f = DispatchFixture::create(json!({})).await;
    std::fs::write(
        DispatchConfigStore::new(f.home.clone()).file(),
        "wip: [nicht, eine, zahl]\n",
    )
    .expect("dispatch.yaml verbiegen");

    let status = f.service.status(None).await.expect("Zustand");
    assert_eq!(status.state, DispatchStateKind::Error);
    assert!(status.config_error.is_some());
    assert!(
        status.message.starts_with("Einstellung fehlerhaft: "),
        "{}",
        status.message
    );
    assert_eq!(f.tick().await.errors.len(), 1);
}

#[tokio::test]
async fn require_board_meldet_nur_ein_fehlendes_board() {
    let f = DispatchFixture::create(json!({})).await;
    f.service
        .require_board(Some(f.board.id))
        .await
        .expect("das eigene Board gibt es");
    f.service
        .require_board(None)
        .await
        .expect("ohne Board ist nichts zu prüfen");
    let error = f
        .service
        .require_board(Some(999_999))
        .await
        .expect_err("ein Board, das es nicht gibt");
    assert_eq!(error.code(), Some("board-not-found"));

    // Ist die Datenbank weg, prüft erst der Zustand (Meldung in `ticketsError`)
    f.backend.break_connection();
    f.service
        .require_board(Some(f.board.id))
        .await
        .expect("kein Fehler ohne Datenbank");
    let status = f.service.status(Some(f.board.id)).await.expect("Zustand");
    assert_eq!(
        status.tickets_error.as_deref(),
        Some("Verbindung zur Board-Datenbank verloren: Datenbank weg")
    );
    assert_eq!(status.tickets, Vec::new());
}
