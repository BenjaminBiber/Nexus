//! Tickets von einem anderen Board desselben Projekts holen (eine Transaktion, alles oder
//! nichts). Portiert aus `packages/core/tests/unit/board/TicketMoveToBoard.test.ts`.

use crate::board::basics::TicketStatus;
use crate::board::database::BoardDatabase;
use crate::board::ticket_ordering::BoardColumn;
use crate::board::ticket_service::{NewTicketInput, TicketListFilter, TicketView, WriteOptions};
use crate::testing::BoardFixture;

/// Umgebung mit zweitem Board „Kollege“ im Projekt ARC
async fn setup() -> (BoardFixture, i32) {
    let f = BoardFixture::create().await;
    let source = f.boards.create("ARC", "Kollege", &f.actor).await.unwrap();
    let id = source.board.id;
    (f, id)
}

/// Legt ein Ticket auf diesem Board an
async fn add_on(f: &BoardFixture, board_id: i32, input: NewTicketInput) -> TicketView {
    f.tickets
        .create(board_id, &input, &f.actor, &WriteOptions::default())
        .await
        .unwrap()
}

/// Die Tickets eines Boards in Anzeige-Reihenfolge
async fn ids_of(f: &BoardFixture, board_id: i32) -> Vec<String> {
    f.tickets
        .list(&TicketListFilter {
            board_ids: vec![board_id],
            ..TicketListFilter::default()
        })
        .await
        .unwrap()
        .into_iter()
        .map(|view| view.ticket.id)
        .collect()
}

#[tokio::test]
async fn holt_mehrere_tickets_hangt_sie_ans_ende_ihrer_spalte_und_erhoht_die_version() {
    let (f, source_id) = setup().await;
    add_on(&f, f.board_id, NewTicketInput::titled("Schon da")).await;
    add_on(
        &f,
        f.board_id,
        NewTicketInput {
            status: Some(TicketStatus::Planned),
            ..NewTicketInput::titled("Auch da")
        },
    )
    .await;
    add_on(&f, source_id, NewTicketInput::titled("A")).await;
    add_on(&f, source_id, NewTicketInput::titled("B")).await;
    add_on(
        &f,
        source_id,
        NewTicketInput {
            status: Some(TicketStatus::Planned),
            ..NewTicketInput::titled("C")
        },
    )
    .await;

    let moved = f
        .tickets
        .move_to_board(
            &["arc-3".into(), "ARC-0004".into(), "ARC-0005".into()],
            f.board_id,
            &f.actor,
        )
        .await
        .unwrap();

    assert_eq!(
        moved
            .iter()
            .map(|t| (
                t.ticket.id.as_str(),
                t.ticket.board_id,
                t.ticket.sort_order,
                t.ticket.rev
            ))
            .collect::<Vec<_>>(),
        [
            ("ARC-0003", f.board_id, 1, 2),
            ("ARC-0004", f.board_id, 2, 2),
            ("ARC-0005", f.board_id, 1, 2),
        ]
    );
    assert_eq!(moved[0].column, BoardColumn::Backlog);
    assert!(!moved[0].waiting);
    assert!(ids_of(&f, source_id).await.is_empty());
    assert_eq!(
        ids_of(&f, f.board_id).await,
        ["ARC-0001", "ARC-0003", "ARC-0004", "ARC-0002", "ARC-0005"].map(String::from)
    );
}

#[tokio::test]
async fn schreibt_einen_verlaufseintrag_boardid_mit_dem_namen_des_quell_boards() {
    let (f, source_id) = setup().await;
    add_on(&f, source_id, NewTicketInput::titled("A")).await;

    f.tickets
        .move_to_board(&["ARC-0001".into()], f.board_id, &f.actor)
        .await
        .unwrap();

    let history = f.tickets.get("ARC-0001").await.unwrap().history;
    assert_eq!(history.len(), 2);
    assert_eq!(history[1].field, "boardId");
    assert_eq!(
        history[1].from.as_deref(),
        Some(source_id.to_string().as_str())
    );
    assert_eq!(
        history[1].to.as_deref(),
        Some(f.board_id.to_string().as_str())
    );
    assert_eq!(history[1].note.as_deref(), Some("von Board Kollege geholt"));
    assert_eq!(history[1].by_name, "claude");
}

#[tokio::test]
async fn nimmt_nichts_wenn_eine_id_unbekannt_ist() {
    let (f, source_id) = setup().await;
    add_on(&f, source_id, NewTicketInput::titled("A")).await;

    assert_eq!(
        f.tickets
            .move_to_board(
                &["ARC-0001".into(), "ARC-0099".into()],
                f.board_id,
                &f.actor
            )
            .await
            .unwrap_err()
            .code(),
        Some("ticket-not-found")
    );

    let t = f.tickets.get("ARC-0001").await.unwrap();
    assert_eq!(t.view.ticket.board_id, source_id);
    assert_eq!(t.view.ticket.rev, 1);
    assert_eq!(t.history.len(), 1);
}

#[tokio::test]
async fn lehnt_tickets_ab_die_schon_auf_dem_ziel_board_liegen() {
    let (f, source_id) = setup().await;
    add_on(&f, source_id, NewTicketInput::titled("A")).await;
    add_on(&f, f.board_id, NewTicketInput::titled("B")).await;

    let error = f
        .tickets
        .move_to_board(
            &["ARC-0001".into(), "ARC-0002".into()],
            f.board_id,
            &f.actor,
        )
        .await
        .unwrap_err();

    assert_eq!(error.code(), Some("ticket-already-on-board"));
    assert!(error.to_string().contains("ARC-0002"));
    assert_eq!(
        f.tickets
            .get("ARC-0001")
            .await
            .unwrap()
            .view
            .ticket
            .board_id,
        source_id
    );
}

#[tokio::test]
async fn lehnt_tickets_aus_einem_anderen_projekt_ab() {
    let (f, source_id) = setup().await;
    f.projects
        .create("INC", "Incidents", &f.actor)
        .await
        .unwrap();
    let foreign = f.boards.create("INC", "Fremd", &f.actor).await.unwrap();
    add_on(&f, source_id, NewTicketInput::titled("A")).await;
    add_on(&f, foreign.board.id, NewTicketInput::titled("Fremd")).await;

    assert_eq!(
        f.tickets
            .move_to_board(
                &["ARC-0001".into(), "INC-0001".into()],
                f.board_id,
                &f.actor
            )
            .await
            .unwrap_err()
            .code(),
        Some("board-other-project")
    );
    assert_eq!(
        f.tickets
            .get("ARC-0001")
            .await
            .unwrap()
            .view
            .ticket
            .board_id,
        source_id
    );
    assert_eq!(
        f.tickets
            .get("INC-0001")
            .await
            .unwrap()
            .view
            .ticket
            .board_id,
        foreign.board.id
    );
}

#[tokio::test]
async fn meldet_leere_liste_und_unbekanntes_ziel_board() {
    let (f, source_id) = setup().await;
    add_on(&f, source_id, NewTicketInput::titled("A")).await;

    assert_eq!(
        f.tickets
            .move_to_board(&[], f.board_id, &f.actor)
            .await
            .unwrap_err()
            .code(),
        Some("nothing-changed")
    );
    assert_eq!(
        f.tickets
            .move_to_board(&["ARC-0001".into()], 999, &f.actor)
            .await
            .unwrap_err()
            .code(),
        Some("board-not-found")
    );
}

#[tokio::test]
async fn nimmt_doppelt_genannte_tickets_nur_einmal() {
    let (f, source_id) = setup().await;
    add_on(&f, source_id, NewTicketInput::titled("A")).await;

    let moved = f
        .tickets
        .move_to_board(&["ARC-0001".into(), "arc-1".into()], f.board_id, &f.actor)
        .await
        .unwrap();

    assert_eq!(
        moved
            .iter()
            .map(|t| t.ticket.id.as_str())
            .collect::<Vec<_>>(),
        ["ARC-0001"]
    );
    assert_eq!(f.tickets.get("ARC-0001").await.unwrap().view.ticket.rev, 2);
}

#[tokio::test]
async fn andert_den_anderungskennwert_beider_boards() {
    let (f, source_id) = setup().await;
    add_on(&f, source_id, NewTicketInput::titled("A")).await;
    let before = [
        f.db.change_marker(source_id).await.unwrap(),
        f.db.change_marker(f.board_id).await.unwrap(),
    ];

    f.tickets
        .move_to_board(&["ARC-0001".into()], f.board_id, &f.actor)
        .await
        .unwrap();

    assert_ne!(f.db.change_marker(source_id).await.unwrap(), before[0]);
    assert_ne!(f.db.change_marker(f.board_id).await.unwrap(), before[1]);
}
