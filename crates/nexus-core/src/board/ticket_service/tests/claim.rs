//! Übernehmen und Freigeben.
//! Portiert aus `packages/core/tests/unit/board/TicketClaim.test.ts`.

use super::{add, create, set};
use crate::board::basics::TicketStatus;
use crate::board::ticket_service::{ClaimOptions, NewTicketInput, TicketPatch};
use crate::testing::BoardFixture;

/// Übernimmt mit den Standard-Optionen
async fn claim(f: &BoardFixture, id: &str, agent: &str) -> crate::error::Result<()> {
    f.tickets
        .claim(id, agent, &f.actor, &ClaimOptions::default())
        .await
        .map(|_| ())
}

#[tokio::test]
async fn ubernimmt_setzt_zuweisung_und_in_arbeit_schreibt_ubernommen() {
    let f = BoardFixture::create().await;
    add(&f, "A").await;

    let t = f
        .tickets
        .claim("ARC-0001", "claude", &f.actor, &ClaimOptions::default())
        .await
        .unwrap();

    assert_eq!(t.ticket.assignee.as_deref(), Some("claude"));
    assert_eq!(t.ticket.status, TicketStatus::InProgress);
    assert_eq!(
        f.tickets
            .get("ARC-0001")
            .await
            .unwrap()
            .history
            .iter()
            .skip(1)
            .map(|entry| (entry.field.as_str(), entry.note.as_deref()))
            .collect::<Vec<_>>(),
        [
            ("status", Some("übernommen")),
            ("assignee", Some("übernommen"))
        ]
    );
}

#[tokio::test]
async fn lehnt_die_ubernahme_eines_zugewiesenen_tickets_ab_auch_fur_denselben_agenten() {
    let f = BoardFixture::create().await;
    add(&f, "A").await;
    claim(&f, "ARC-0001", "claude-1").await.unwrap();

    assert_eq!(
        claim(&f, "ARC-0001", "claude-2").await.unwrap_err().code(),
        Some("conflict")
    );
    assert_eq!(
        claim(&f, "ARC-0001", "claude-1").await.unwrap_err().code(),
        Some("conflict")
    );

    let force = ClaimOptions {
        force: true,
        ..ClaimOptions::default()
    };
    for agent in ["claude-1", "claude-2"] {
        assert_eq!(
            f.tickets
                .claim("ARC-0001", agent, &f.actor, &force)
                .await
                .unwrap()
                .ticket
                .assignee
                .as_deref(),
            Some(agent)
        );
    }
}

#[tokio::test]
async fn lehnt_die_ubernahme_bei_offener_voraussetzung_ab_auch_mit_anderem_zielstatus() {
    let f = BoardFixture::create().await;
    add(&f, "A").await;
    create(
        &f,
        NewTicketInput {
            depends_on: Some(vec!["ARC-0001".into()]),
            ..NewTicketInput::titled("B")
        },
    )
    .await
    .unwrap();

    assert_eq!(
        f.tickets
            .claim(
                "ARC-0002",
                "claude",
                &f.actor,
                &ClaimOptions {
                    status: Some(TicketStatus::Planned),
                    ..ClaimOptions::default()
                }
            )
            .await
            .unwrap_err()
            .code(),
        Some("dependency-open")
    );
}

#[tokio::test]
async fn lehnt_die_ubernahme_mit_veralteter_version_ab() {
    let f = BoardFixture::create().await;
    let created = add(&f, "A").await;
    set(
        &f,
        "ARC-0001",
        TicketPatch {
            title: Some("B".into()),
            ..TicketPatch::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(
        f.tickets
            .claim(
                "ARC-0001",
                "claude",
                &f.actor,
                &ClaimOptions {
                    rev: Some(created.ticket.rev),
                    ..ClaimOptions::default()
                }
            )
            .await
            .unwrap_err()
            .code(),
        Some("conflict")
    );
    assert_eq!(
        f.tickets
            .get("ARC-0001")
            .await
            .unwrap()
            .view
            .ticket
            .assignee,
        None
    );
    assert_eq!(
        f.tickets
            .claim(
                "ARC-0001",
                "claude",
                &f.actor,
                &ClaimOptions {
                    rev: Some(created.ticket.rev + 1),
                    ..ClaimOptions::default()
                }
            )
            .await
            .unwrap()
            .ticket
            .assignee
            .as_deref(),
        Some("claude")
    );
}

#[tokio::test]
async fn verlangt_einen_agentennamen() {
    let f = BoardFixture::create().await;
    add(&f, "A").await;

    assert_eq!(
        claim(&f, "ARC-0001", " ").await.unwrap_err().code(),
        Some("agent-missing")
    );
}

#[tokio::test]
async fn gibt_frei_und_andert_den_status_nur_auf_wunsch() {
    let f = BoardFixture::create().await;
    add(&f, "A").await;
    claim(&f, "ARC-0001", "claude").await.unwrap();

    let released = f.tickets.release("ARC-0001", &f.actor, None).await.unwrap();
    assert_eq!(released.ticket.assignee, None);
    assert_eq!(released.ticket.status, TicketStatus::InProgress);
    assert_eq!(
        f.tickets
            .release("ARC-0001", &f.actor, Some(TicketStatus::Planned))
            .await
            .unwrap()
            .ticket
            .status,
        TicketStatus::Planned
    );
}
