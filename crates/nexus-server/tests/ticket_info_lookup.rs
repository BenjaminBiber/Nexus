//! Die Ticketangaben mit Zeitlimit; portiert aus
//! `packages/server/tests/unit/TicketInfoLookup.test.ts`.

mod common;

use std::sync::Arc;

use nexus_core::board::basics::TicketStatus;
use nexus_core::board::ticket_service::TicketSummary;
use nexus_server::sessions::{TicketInfoLookup, TicketSummaries};

/// Eine Abfrage, die diese Kurzangaben liefert
fn answers(list: Vec<TicketSummary>) -> TicketSummaries {
    Arc::new(move |_ids| {
        let list = list.clone();
        Box::pin(async move { Ok(list) })
    })
}

/// Eine Abfrage, die nie antwortet (hängende Board-Datenbank)
fn never() -> TicketSummaries {
    Arc::new(|_ids| Box::pin(std::future::pending()))
}

#[tokio::test]
async fn liefert_die_angaben_aus_der_board_db() {
    let lookup = TicketInfoLookup::new(answers(vec![TicketSummary {
        id: "ARC-0001".into(),
        title: "Login".into(),
        status: TicketStatus::Planned,
        board_id: 1,
    }]));
    let infos = lookup.infos(&["ARC-0001".to_string()]).await;
    let found = infos.get("ARC-0001").expect("Angaben");
    assert_eq!(found.id, "ARC-0001");
    assert_eq!(found.title, "Login");
    assert_eq!(found.status, "planned");
}

#[tokio::test]
async fn wartet_nicht_ewig_auf_eine_hangende_board_db() {
    let lookup = TicketInfoLookup::with_timeout(never(), 50);
    let started = std::time::Instant::now();
    assert!(lookup.infos(&["ARC-0001".to_string()]).await.is_empty());
    assert!(started.elapsed() < std::time::Duration::from_secs(1));
}

#[tokio::test]
async fn begrenzt_die_wartezeit_standardmasig_auf_etwa_eine_sekunde() {
    let lookup = TicketInfoLookup::new(never());
    let started = std::time::Instant::now();
    assert!(lookup.infos(&["ARC-0001".to_string()]).await.is_empty());
    assert!(started.elapsed() < std::time::Duration::from_secs(3));
}

#[tokio::test]
async fn fragt_ohne_ticket_ids_gar_nicht_und_fasst_doppelte_zusammen() {
    let lookup = TicketInfoLookup::new(never());
    assert!(lookup.infos(&[]).await.is_empty());

    let asked: Arc<std::sync::Mutex<Vec<Vec<String>>>> =
        Arc::new(std::sync::Mutex::new(Vec::new()));
    let seen = asked.clone();
    let summaries: TicketSummaries = Arc::new(move |ids: Vec<String>| {
        seen.lock().expect("Sperre").push(ids);
        Box::pin(async { Ok(Vec::new()) })
    });
    TicketInfoLookup::new(summaries)
        .infos(&["ARC-0001".into(), "ARC-0001".into(), "ARC-0002".into()])
        .await;
    assert_eq!(
        asked.lock().expect("Sperre").clone(),
        [vec!["ARC-0001".to_string(), "ARC-0002".to_string()]]
    );
}
