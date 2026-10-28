//! Anlegen, Lesen, Ändern, Verschieben und Löschen von Tickets.
//! Portiert aus `packages/core/tests/unit/board/TicketService.test.ts`.

use super::{add, create, history, set};
use crate::board::basics::{Priority, TicketStatus};
use crate::board::database::BoardDatabase;
use crate::board::ticket_ordering::BoardColumn;
use crate::board::ticket_service::{
    MoveOptions, NewTicketInput, TicketListFilter, TicketPatch, WriteOptions,
};
use crate::testing::BoardFixture;
use crate::tx;

// ---------------------------------------------------------------------------------------------
// anlegen
// ---------------------------------------------------------------------------------------------

#[tokio::test]
async fn vergibt_die_id_aus_dem_projekt_und_setzt_standardwerte() {
    let f = BoardFixture::create().await;

    let t = add(&f, "  Login bauen ").await;

    assert_eq!(t.ticket.id, "ARC-0001");
    assert_eq!(t.ticket.title, "Login bauen");
    assert_eq!(t.ticket.status, TicketStatus::Backlog);
    assert_eq!(t.ticket.priority, Priority::Normal);
    assert!(t.ticket.needs_planning);
    assert_eq!(t.ticket.assignee, None);
    assert_eq!(t.ticket.rev, 1);
    assert_eq!(t.ticket.created_by_name, "claude");
    assert_eq!(t.column, BoardColumn::Backlog);
    assert!(!t.waiting);
    assert_eq!(t.ticket.sort_order, 0);

    let second = create(
        &f,
        NewTicketInput {
            labels: Some(vec!["ui, api".into(), "ui".into()]),
            ..NewTicketInput::titled("Zweites")
        },
    )
    .await
    .unwrap();

    assert_eq!(second.ticket.id, "ARC-0002");
    assert_eq!(second.ticket.labels, ["ui", "api"]);
    assert_eq!(second.ticket.sort_order, 1);
}

#[tokio::test]
async fn schreibt_angelegt_in_den_verlauf() {
    let f = BoardFixture::create().await;

    create(
        &f,
        NewTicketInput {
            status: Some(TicketStatus::Planned),
            ..NewTicketInput::titled("A")
        },
    )
    .await
    .unwrap();

    let entries = f.tickets.get("ARC-0001").await.unwrap().history;
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].field, "status");
    assert_eq!(entries[0].from, None);
    assert_eq!(entries[0].to.as_deref(), Some("planned"));
    assert_eq!(entries[0].note.as_deref(), Some("angelegt"));
    assert_eq!(entries[0].by_name, "claude");
}

#[tokio::test]
async fn lehnt_leeren_titel_und_unbekannte_voraussetzungen_ab() {
    let f = BoardFixture::create().await;

    assert_eq!(
        create(&f, NewTicketInput::titled(" "))
            .await
            .unwrap_err()
            .code(),
        Some("title-empty")
    );
    assert_eq!(
        create(
            &f,
            NewTicketInput {
                depends_on: Some(vec!["ARC-0042".into()]),
                ..NewTicketInput::titled("A")
            }
        )
        .await
        .unwrap_err()
        .code(),
        Some("dependency-unknown")
    );
}

#[tokio::test]
async fn lehnt_ein_offenes_ticket_mit_gleichem_titel_auf_demselben_board_ab() {
    let f = BoardFixture::create().await;
    add(&f, "Login bauen").await;

    let error = create(&f, NewTicketInput::titled("  login BAUEN "))
        .await
        .unwrap_err();
    assert_eq!(error.code(), Some("ticket-duplicate"));
    assert_eq!(
        error.to_string(),
        "Ein offenes Ticket mit diesem Titel gibt es schon: ARC-0001."
    );

    // Die Dublettenprüfung hat eine eigene Option; force allein übergeht sie nicht
    assert_eq!(
        f.tickets
            .create(
                f.board_id,
                &NewTicketInput::titled("Login bauen"),
                &f.actor,
                &WriteOptions {
                    force: true,
                    ..WriteOptions::default()
                },
            )
            .await
            .unwrap_err()
            .code(),
        Some("ticket-duplicate")
    );
    let allowed = WriteOptions {
        allow_duplicate: true,
        ..WriteOptions::default()
    };
    assert_eq!(
        f.tickets
            .create(
                f.board_id,
                &NewTicketInput::titled("Login bauen"),
                &f.actor,
                &allowed,
            )
            .await
            .unwrap()
            .ticket
            .id,
        "ARC-0002"
    );
    // allowDuplicate übergeht keine Sperre
    assert_eq!(
        f.tickets
            .create(
                f.board_id,
                &NewTicketInput {
                    status: Some(TicketStatus::NotNeeded),
                    ..NewTicketInput::titled("Login bauen")
                },
                &f.actor,
                &allowed,
            )
            .await
            .unwrap_err()
            .code(),
        Some("not-needed-user")
    );
    assert_eq!(add(&f, "Login bauen (neu)").await.ticket.id, "ARC-0003");

    let other = f.boards.create("ARC", "Anderes", &f.actor).await.unwrap();
    assert_eq!(
        f.tickets
            .create(
                other.board.id,
                &NewTicketInput::titled("Login bauen"),
                &f.actor,
                &WriteOptions::default(),
            )
            .await
            .unwrap()
            .ticket
            .board_id,
        other.board.id
    );

    // Erledigt und „Nicht mehr nötig“ zählen nicht
    create(
        &f,
        NewTicketInput {
            status: Some(TicketStatus::Done),
            acceptance: Some("ok".into()),
            ..NewTicketInput::titled("Alt")
        },
    )
    .await
    .unwrap();
    f.tickets
        .create(
            f.board_id,
            &NewTicketInput {
                status: Some(TicketStatus::NotNeeded),
                ..NewTicketInput::titled("Weg")
            },
            &f.actor,
            &WriteOptions {
                by_user: true,
                ..WriteOptions::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(add(&f, "alt").await.ticket.title, "alt");
    assert_eq!(add(&f, "Weg").await.ticket.title, "Weg");
}

#[tokio::test]
async fn pruft_die_status_sperren_auch_beim_anlegen() {
    let f = BoardFixture::create().await;

    assert_eq!(
        create(
            &f,
            NewTicketInput {
                status: Some(TicketStatus::Done),
                ..NewTicketInput::titled("A")
            }
        )
        .await
        .unwrap_err()
        .code(),
        Some("acceptance-missing")
    );
    assert_eq!(
        create(
            &f,
            NewTicketInput {
                status: Some(TicketStatus::Done),
                acceptance: Some("läuft".into()),
                ..NewTicketInput::titled("B")
            }
        )
        .await
        .unwrap()
        .ticket
        .status,
        TicketStatus::Done
    );

    add(&f, "Basis").await;
    let folgt = NewTicketInput {
        status: Some(TicketStatus::InProgress),
        depends_on: Some(vec!["ARC-0002".into()]),
        ..NewTicketInput::titled("C")
    };
    assert_eq!(
        create(&f, folgt.clone()).await.unwrap_err().code(),
        Some("dependency-open")
    );
    assert_eq!(
        f.tickets
            .create(
                f.board_id,
                &NewTicketInput {
                    title: "D".into(),
                    ..folgt
                },
                &f.actor,
                &WriteOptions {
                    force: true,
                    ..WriteOptions::default()
                },
            )
            .await
            .unwrap()
            .ticket
            .status,
        TicketStatus::InProgress
    );
    assert_eq!(
        create(
            &f,
            NewTicketInput {
                status: Some(TicketStatus::NotNeeded),
                ..NewTicketInput::titled("E")
            }
        )
        .await
        .unwrap_err()
        .code(),
        Some("not-needed-user")
    );
    assert_eq!(
        f.tickets
            .create(
                f.board_id,
                &NewTicketInput {
                    status: Some(TicketStatus::NotNeeded),
                    ..NewTicketInput::titled("F")
                },
                &f.actor,
                &WriteOptions {
                    by_user: true,
                    ..WriteOptions::default()
                },
            )
            .await
            .unwrap()
            .ticket
            .status,
        TicketStatus::NotNeeded
    );
}

// ---------------------------------------------------------------------------------------------
// lesen
// ---------------------------------------------------------------------------------------------

#[tokio::test]
async fn zeigt_offene_voraussetzungen_abhangige_und_wartet() {
    let f = BoardFixture::create().await;
    add(&f, "Basis").await;
    create(
        &f,
        NewTicketInput {
            depends_on: Some(vec!["arc-1".into()]),
            ..NewTicketInput::titled("Aufbau")
        },
    )
    .await
    .unwrap();

    let detail = f.tickets.get("arc-2").await.unwrap();

    assert_eq!(detail.view.ticket.id, "ARC-0002");
    assert_eq!(detail.view.ticket.depends_on, ["ARC-0001"]);
    assert_eq!(detail.view.depends_open, ["ARC-0001"]);
    assert!(detail.view.waiting);
    assert_eq!(detail.view.column, BoardColumn::Waiting);
    assert_eq!(
        f.tickets.get("ARC-0001").await.unwrap().view.dependents,
        ["ARC-0002"]
    );
}

#[tokio::test]
async fn nennt_in_unlocks_die_tickets_die_ihre_letzte_offene_voraussetzung_verlieren() {
    let f = BoardFixture::create().await;
    add(&f, "Basis").await;
    add(&f, "Zweite").await;
    create(
        &f,
        NewTicketInput {
            depends_on: Some(vec!["ARC-0001".into()]),
            ..NewTicketInput::titled("Nur Basis")
        },
    )
    .await
    .unwrap();
    create(
        &f,
        NewTicketInput {
            depends_on: Some(vec!["ARC-0001".into(), "ARC-0002".into()]),
            ..NewTicketInput::titled("Beide")
        },
    )
    .await
    .unwrap();

    assert_eq!(
        f.tickets.get("ARC-0001").await.unwrap().view.unlocks,
        ["ARC-0003"]
    );
    assert!(f
        .tickets
        .get("ARC-0003")
        .await
        .unwrap()
        .view
        .unlocks
        .is_empty());
}

#[tokio::test]
async fn accept_nur_blockiert_mit_grund_human_interaction() {
    let f = BoardFixture::create().await;
    create(
        &f,
        NewTicketInput {
            acceptance: Some("ok".into()),
            ..NewTicketInput::titled("Ansehen")
        },
    )
    .await
    .unwrap();
    add(&f, "Ohne Abnahme").await;

    assert_eq!(
        f.tickets
            .accept("ARC-0001", &f.actor, &WriteOptions::default())
            .await
            .unwrap_err()
            .code(),
        Some("accept-not-possible")
    );
    set(
        &f,
        "ARC-0001",
        TicketPatch {
            status: Some(TicketStatus::Blocked),
            blocked_reason: Some(Some("Merge: kaputt".into())),
            ..TicketPatch::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(
        f.tickets
            .accept("ARC-0001", &f.actor, &WriteOptions::default())
            .await
            .unwrap_err()
            .code(),
        Some("accept-not-possible")
    );
    set(
        &f,
        "ARC-0001",
        TicketPatch {
            blocked_reason: Some(Some("Human Interaction: Oberfläche ansehen".into())),
            ..TicketPatch::default()
        },
    )
    .await
    .unwrap();

    let accepted = f
        .tickets
        .accept("ARC-0001", &f.actor, &WriteOptions::default())
        .await
        .unwrap();
    assert_eq!(accepted.ticket.status, TicketStatus::Done);
    assert_eq!(accepted.ticket.blocked_reason, None);
    assert!(f
        .tickets
        .get("ARC-0001")
        .await
        .unwrap()
        .history
        .iter()
        .any(|entry| entry.field == "status"
            && entry.to.as_deref() == Some("done")
            && entry.note.as_deref() == Some("abgenommen")));

    set(
        &f,
        "ARC-0002",
        TicketPatch {
            status: Some(TicketStatus::Blocked),
            blocked_reason: Some(Some("Human Interaction".into())),
            ..TicketPatch::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(
        f.tickets
            .accept("ARC-0002", &f.actor, &WriteOptions::default())
            .await
            .unwrap_err()
            .code(),
        Some("acceptance-missing")
    );
}

#[tokio::test]
async fn nennt_erledigte_voraussetzungen_die_noch_einen_branch_haben() {
    let f = BoardFixture::create().await;
    let mit_abnahme = |title: &str| NewTicketInput {
        acceptance: Some("ok".into()),
        ..NewTicketInput::titled(title)
    };
    create(&f, mit_abnahme("Basis")).await.unwrap();
    create(&f, mit_abnahme("Zweite")).await.unwrap();
    create(
        &f,
        NewTicketInput {
            depends_on: Some(vec!["ARC-0001".into(), "ARC-0002".into()]),
            ..NewTicketInput::titled("Aufbau")
        },
    )
    .await
    .unwrap();

    assert!(f
        .tickets
        .get("ARC-0003")
        .await
        .unwrap()
        .view
        .depends_unmerged
        .is_empty());

    set(
        &f,
        "ARC-0001",
        TicketPatch {
            status: Some(TicketStatus::Done),
            branch: Some(Some("ticket/ARC-0001-basis".into())),
            ..TicketPatch::default()
        },
    )
    .await
    .unwrap();
    set(
        &f,
        "ARC-0002",
        TicketPatch {
            status: Some(TicketStatus::Done),
            ..TicketPatch::default()
        },
    )
    .await
    .unwrap();

    let aufbau = f.tickets.get("ARC-0003").await.unwrap();
    assert!(aufbau.view.depends_open.is_empty());
    assert_eq!(aufbau.view.depends_unmerged, ["ARC-0001"]);
    assert!(!aufbau.view.waiting);

    set(
        &f,
        "ARC-0001",
        TicketPatch {
            branch: Some(None),
            ..TicketPatch::default()
        },
    )
    .await
    .unwrap();
    assert!(f
        .tickets
        .get("ARC-0003")
        .await
        .unwrap()
        .view
        .depends_unmerged
        .is_empty());
}

#[tokio::test]
async fn meldet_ein_unbekanntes_ticket() {
    let f = BoardFixture::create().await;

    assert_eq!(
        f.tickets.get("ARC-0099").await.unwrap_err().code(),
        Some("ticket-not-found")
    );
}

#[tokio::test]
async fn filtert_nach_spalte_suchtext_label_zuweisung_und_planungsbedarf_und_sortiert() {
    let f = BoardFixture::create().await;
    create(
        &f,
        NewTicketInput {
            labels: Some(vec!["ui".into()]),
            priority: Some(Priority::Low),
            ..NewTicketInput::titled("Login")
        },
    )
    .await
    .unwrap();
    create(
        &f,
        NewTicketInput {
            description: Some("login-Token".into()),
            priority: Some(Priority::Urgent),
            needs_planning: Some(false),
            ..NewTicketInput::titled("Cache")
        },
    )
    .await
    .unwrap();
    create(
        &f,
        NewTicketInput {
            depends_on: Some(vec!["ARC-0001".into()]),
            ..NewTicketInput::titled("Wartet")
        },
    )
    .await
    .unwrap();

    let ids = |filter: TicketListFilter| {
        let tickets = &f.tickets;
        async move {
            tickets
                .list(&filter)
                .await
                .unwrap()
                .into_iter()
                .map(|view| view.ticket.id)
                .collect::<Vec<_>>()
        }
    };
    let board = || TicketListFilter {
        board_ids: vec![f.board_id],
        ..TicketListFilter::default()
    };

    assert_eq!(
        ids(board()).await,
        ["ARC-0002", "ARC-0001", "ARC-0003"].map(String::from)
    );
    assert_eq!(
        ids(TicketListFilter {
            q: Some("LOGIN".into()),
            ..board()
        })
        .await,
        ["ARC-0002", "ARC-0001"].map(String::from)
    );
    assert_eq!(
        ids(TicketListFilter {
            label: Some("ui".into()),
            ..board()
        })
        .await,
        ["ARC-0001"].map(String::from)
    );
    assert_eq!(
        ids(TicketListFilter {
            columns: Some(vec![BoardColumn::Waiting]),
            ..board()
        })
        .await,
        ["ARC-0003"].map(String::from)
    );
    assert_eq!(
        ids(TicketListFilter {
            columns: Some(vec![BoardColumn::Backlog]),
            ..board()
        })
        .await,
        ["ARC-0002", "ARC-0001"].map(String::from)
    );
    assert_eq!(
        ids(TicketListFilter {
            needs_planning: Some(true),
            ..board()
        })
        .await,
        ["ARC-0001", "ARC-0003"].map(String::from)
    );
}

// ---------------------------------------------------------------------------------------------
// ändern
// ---------------------------------------------------------------------------------------------

#[tokio::test]
async fn behandelt_den_blockiert_grund_beim_andern_des_status_uber_update() {
    let f = BoardFixture::create().await;
    for title in ["A", "B", "C"] {
        add(&f, title).await;
    }
    for id in ["ARC-0001", "ARC-0002", "ARC-0003"] {
        set(
            &f,
            id,
            TicketPatch {
                status: Some(TicketStatus::Blocked),
                blocked_reason: Some(Some("Zugang fehlt".into())),
                ..TicketPatch::default()
            },
        )
        .await
        .unwrap();
    }

    assert_eq!(
        set(
            &f,
            "ARC-0001",
            TicketPatch {
                status: Some(TicketStatus::Planned),
                ..TicketPatch::default()
            }
        )
        .await
        .unwrap()
        .ticket
        .blocked_reason,
        None
    );
    assert_eq!(
        set(
            &f,
            "ARC-0002",
            TicketPatch {
                status: Some(TicketStatus::Planned),
                blocked_reason: Some(Some("Rest".into())),
                ..TicketPatch::default()
            }
        )
        .await
        .unwrap()
        .ticket
        .blocked_reason
        .as_deref(),
        Some("Rest")
    );
    assert_eq!(
        set(
            &f,
            "ARC-0003",
            TicketPatch {
                blocked_reason: Some(Some("Neu".into())),
                ..TicketPatch::default()
            }
        )
        .await
        .unwrap()
        .ticket
        .blocked_reason
        .as_deref(),
        Some("Neu")
    );
}

#[tokio::test]
async fn andert_felder_erhoht_die_version_und_schreibt_den_verlauf() {
    let f = BoardFixture::create().await;
    add(&f, "A").await;

    let t = set(
        &f,
        "ARC-0001",
        TicketPatch {
            title: Some("B".into()),
            plan: Some("Plan".into()),
            priority: Some(Priority::High),
            assignee: Some(Some("claude".into())),
            ..TicketPatch::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(t.ticket.title, "B");
    assert_eq!(t.ticket.plan, "Plan");
    assert_eq!(t.ticket.priority, Priority::High);
    assert_eq!(t.ticket.assignee.as_deref(), Some("claude"));
    assert_eq!(t.ticket.rev, 2);
    assert_eq!(t.ticket.status, TicketStatus::Backlog);
    assert!(t.ticket.needs_planning);
    assert_eq!(
        history(&f, "ARC-0001")
            .await
            .into_iter()
            .map(|(field, _, _)| field)
            .collect::<Vec<_>>(),
        ["status", "title", "priority", "assignee", "plan"]
    );
}

#[tokio::test]
async fn lehnt_eine_leere_anderung_und_eine_veraltete_version_ab() {
    let f = BoardFixture::create().await;
    add(&f, "A").await;

    assert_eq!(
        set(&f, "ARC-0001", TicketPatch::default())
            .await
            .unwrap_err()
            .code(),
        Some("nothing-changed")
    );
    let mit_rev = WriteOptions {
        rev: Some(1),
        ..WriteOptions::default()
    };
    f.tickets
        .update(
            "ARC-0001",
            &TicketPatch {
                title: Some("B".into()),
                ..TicketPatch::default()
            },
            &f.actor,
            &mit_rev,
        )
        .await
        .unwrap();
    assert_eq!(
        f.tickets
            .update(
                "ARC-0001",
                &TicketPatch {
                    title: Some("C".into()),
                    ..TicketPatch::default()
                },
                &f.actor,
                &mit_rev,
            )
            .await
            .unwrap_err()
            .code(),
        Some("conflict")
    );
}

#[tokio::test]
async fn erganzt_und_entfernt_voraussetzungen_und_lehnt_kreise_ab() {
    let f = BoardFixture::create().await;
    for title in ["A", "B", "C"] {
        add(&f, title).await;
    }

    set(
        &f,
        "ARC-0003",
        TicketPatch {
            depends_on_add: Some(vec!["ARC-0001".into(), "ARC-0002".into()]),
            ..TicketPatch::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(
        set(
            &f,
            "ARC-0003",
            TicketPatch {
                depends_on_remove: Some(vec!["ARC-0001".into()]),
                ..TicketPatch::default()
            }
        )
        .await
        .unwrap()
        .ticket
        .depends_on,
        ["ARC-0002"]
    );
    assert_eq!(
        set(
            &f,
            "ARC-0002",
            TicketPatch {
                depends_on: Some(vec!["ARC-0003".into()]),
                ..TicketPatch::default()
            }
        )
        .await
        .unwrap_err()
        .code(),
        Some("dependency-cycle")
    );
}

#[tokio::test]
async fn verschiebt_auf_ein_anderes_board_desselben_projekts_nicht_in_ein_anderes_projekt() {
    let f = BoardFixture::create().await;
    let other = f.boards.create("ARC", "Kollege", &f.actor).await.unwrap();
    f.projects
        .create("INC", "Incidents", &f.actor)
        .await
        .unwrap();
    let foreign = f.boards.create("INC", "Fremd", &f.actor).await.unwrap();
    add(&f, "A").await;

    let moved = set(
        &f,
        "ARC-0001",
        TicketPatch {
            board_id: Some(other.board.id),
            ..TicketPatch::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(moved.ticket.id, "ARC-0001");
    assert_eq!(moved.ticket.board_id, other.board.id);
    assert_eq!(
        set(
            &f,
            "ARC-0001",
            TicketPatch {
                board_id: Some(foreign.board.id),
                ..TicketPatch::default()
            }
        )
        .await
        .unwrap_err()
        .code(),
        Some("board-other-project")
    );
}

// ---------------------------------------------------------------------------------------------
// verschieben
// ---------------------------------------------------------------------------------------------

#[tokio::test]
async fn verlangt_fur_erledigt_eine_abnahme_nimmt_sie_aber_im_selben_aufruf_an() {
    let f = BoardFixture::create().await;
    add(&f, "A").await;

    assert_eq!(
        f.tickets
            .move_to(
                "ARC-0001",
                TicketStatus::Done,
                &f.actor,
                &MoveOptions::default()
            )
            .await
            .unwrap_err()
            .code(),
        Some("acceptance-missing")
    );
    let t = f
        .tickets
        .move_to(
            "ARC-0001",
            TicketStatus::Done,
            &f.actor,
            &MoveOptions {
                acceptance: Some("Test grün".into()),
                ..MoveOptions::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(t.ticket.status, TicketStatus::Done);
    assert_eq!(t.ticket.acceptance, "Test grün");
    assert_eq!(
        f.tickets
            .get("ARC-0001")
            .await
            .unwrap()
            .history
            .last()
            .unwrap()
            .note
            .as_deref(),
        Some("verschoben")
    );
}

#[tokio::test]
async fn sperrt_in_arbeit_und_erledigt_bei_offener_voraussetzung_ausser_mit_force() {
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

    let error = f
        .tickets
        .move_to(
            "ARC-0002",
            TicketStatus::InProgress,
            &f.actor,
            &MoveOptions::default(),
        )
        .await
        .unwrap_err();
    assert_eq!(error.code(), Some("dependency-open"));
    assert!(error.to_string().contains("ARC-0001"));
    // Die Meldung bleibt neutral; den --force-Hinweis ergänzt erst die CLI
    assert!(!error.to_string().contains("--force"));

    assert_eq!(
        f.tickets
            .move_to(
                "ARC-0002",
                TicketStatus::Planned,
                &f.actor,
                &MoveOptions::default()
            )
            .await
            .unwrap()
            .ticket
            .status,
        TicketStatus::Planned
    );
    assert_eq!(
        f.tickets
            .move_to(
                "ARC-0002",
                TicketStatus::InProgress,
                &f.actor,
                &MoveOptions {
                    force: true,
                    ..MoveOptions::default()
                }
            )
            .await
            .unwrap()
            .ticket
            .status,
        TicketStatus::InProgress
    );
}

#[tokio::test]
async fn uberlasst_nicht_mehr_notig_dem_nutzer() {
    let f = BoardFixture::create().await;
    add(&f, "A").await;

    assert_eq!(
        f.tickets
            .move_to(
                "ARC-0001",
                TicketStatus::NotNeeded,
                &f.actor,
                &MoveOptions::default()
            )
            .await
            .unwrap_err()
            .code(),
        Some("not-needed-user")
    );
    assert_eq!(
        f.tickets
            .move_to(
                "ARC-0001",
                TicketStatus::NotNeeded,
                &f.actor,
                &MoveOptions {
                    by_user: true,
                    ..MoveOptions::default()
                }
            )
            .await
            .unwrap()
            .ticket
            .status,
        TicketStatus::NotNeeded
    );
}

#[tokio::test]
async fn loscht_den_blockiert_grund_beim_verlassen_von_blockiert_und_hangt_hinten_an() {
    let f = BoardFixture::create().await;
    create(
        &f,
        NewTicketInput {
            status: Some(TicketStatus::Planned),
            ..NewTicketInput::titled("A")
        },
    )
    .await
    .unwrap();
    add(&f, "B").await;
    set(
        &f,
        "ARC-0002",
        TicketPatch {
            status: Some(TicketStatus::Blocked),
            blocked_reason: Some(Some("Zugang fehlt".into())),
            ..TicketPatch::default()
        },
    )
    .await
    .unwrap();

    let t = f
        .tickets
        .move_to(
            "ARC-0002",
            TicketStatus::Planned,
            &f.actor,
            &MoveOptions::default(),
        )
        .await
        .unwrap();

    assert_eq!(t.ticket.blocked_reason, None);
    assert_eq!(t.ticket.sort_order, 1);
}

// ---------------------------------------------------------------------------------------------
// löschen
// ---------------------------------------------------------------------------------------------

#[tokio::test]
async fn protokolliert_die_loschung_und_entfernt_verweise() {
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

    f.tickets.delete("ARC-0001", &f.actor).await.unwrap();

    assert_eq!(
        f.tickets.get("ARC-0001").await.unwrap_err().code(),
        Some("ticket-not-found")
    );
    assert!(f
        .tickets
        .get("ARC-0002")
        .await
        .unwrap()
        .view
        .ticket
        .depends_on
        .is_empty());
    let deletions = tx!(f.db, |u| { u.deletions().list().await }).await.unwrap();
    assert_eq!(deletions.len(), 1);
    assert_eq!(deletions[0].ticket_id, "ARC-0001");
    assert_eq!(deletions[0].ticket_title, "A");
    assert_eq!(deletions[0].from_status, Some(TicketStatus::Backlog));
    assert_eq!(deletions[0].by_name, "claude");
}

// ---------------------------------------------------------------------------------------------
// Längen und Labels
// ---------------------------------------------------------------------------------------------

#[tokio::test]
async fn lehnt_zu_lange_titel_ab_beim_anlegen_und_andern() {
    let f = BoardFixture::create().await;

    let error = create(&f, NewTicketInput::titled("x".repeat(401)))
        .await
        .unwrap_err();
    assert_eq!(error.code(), Some("value-too-long"));
    assert!(error.to_string().contains("Titel"));

    add(&f, &"x".repeat(400)).await;
    assert_eq!(
        set(
            &f,
            "ARC-0001",
            TicketPatch {
                title: Some("y".repeat(401)),
                ..TicketPatch::default()
            }
        )
        .await
        .unwrap_err()
        .code(),
        Some("value-too-long")
    );
}

#[tokio::test]
async fn lehnt_zu_lange_labels_grunde_zustandige_und_handelnde_ab() {
    let f = BoardFixture::create().await;

    let error = create(
        &f,
        NewTicketInput {
            labels: Some(vec!["l".repeat(101)]),
            ..NewTicketInput::titled("A")
        },
    )
    .await
    .unwrap_err();
    assert_eq!(error.code(), Some("value-too-long"));
    assert!(error.to_string().contains("Label"));

    create(
        &f,
        NewTicketInput {
            labels: Some(vec!["l".repeat(100)]),
            ..NewTicketInput::titled("A")
        },
    )
    .await
    .unwrap();

    for patch in [
        TicketPatch {
            labels: Some(vec!["ok".into(), "l".repeat(101)]),
            ..TicketPatch::default()
        },
        TicketPatch {
            blocked_reason: Some(Some("r".repeat(501))),
            ..TicketPatch::default()
        },
        TicketPatch {
            assignee: Some(Some("a".repeat(101))),
            ..TicketPatch::default()
        },
    ] {
        assert_eq!(
            set(&f, "ARC-0001", patch).await.unwrap_err().code(),
            Some("value-too-long")
        );
    }
    assert_eq!(
        f.tickets
            .claim(
                "ARC-0001",
                &"a".repeat(101),
                &f.actor,
                &crate::board::ticket_service::ClaimOptions::default()
            )
            .await
            .unwrap_err()
            .code(),
        Some("value-too-long")
    );

    let zu_langer_name = crate::board::basics::BoardActor {
        name: "n".repeat(101),
        ..f.actor.clone()
    };
    let error = f
        .tickets
        .update(
            "ARC-0001",
            &TicketPatch {
                title: Some("B".into()),
                ..TicketPatch::default()
            },
            &zu_langer_name,
            &WriteOptions::default(),
        )
        .await
        .unwrap_err();
    assert_eq!(error.code(), Some("value-too-long"));
    assert!(error.to_string().contains("Handelnden"));
    assert_eq!(
        f.comments
            .add("ARC-0001", "Hallo", &zu_langer_name, false)
            .await
            .unwrap_err()
            .code(),
        Some("value-too-long")
    );
    assert_eq!(
        f.tickets.get("ARC-0001").await.unwrap().view.ticket.title,
        "A"
    );
}

#[tokio::test]
async fn entfernt_doppelte_labels_ohne_rucksicht_auf_gross_kleinschreibung() {
    let f = BoardFixture::create().await;

    let t = create(
        &f,
        NewTicketInput {
            labels: Some(
                ["UI", "ui, Api", "api", "Ui"]
                    .into_iter()
                    .map(String::from)
                    .collect(),
            ),
            ..NewTicketInput::titled("A")
        },
    )
    .await
    .unwrap();

    assert_eq!(t.ticket.labels, ["UI", "Api"]);
    assert_eq!(
        set(
            &f,
            &t.ticket.id,
            TicketPatch {
                labels: Some(vec!["cache".into(), "CACHE".into()]),
                ..TicketPatch::default()
            }
        )
        .await
        .unwrap()
        .ticket
        .labels,
        ["cache"]
    );
}
