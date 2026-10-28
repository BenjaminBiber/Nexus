//! Benutzer, Projekte, Boards, Tickets, Arbeitsschritte und Ticket-Sitzungen auf SQLite;
//! portiert aus `packages/db/tests/sqlite/SqliteUserRepository.test.ts`,
//! `SqliteProjectRepository.test.ts`, `SqliteBoardRepository.test.ts`,
//! `SqliteTicketRepository.test.ts`, `SqliteTicketStepRepository.test.ts` und
//! `SqliteTicketSessionRepository.test.ts`.

mod common;

use common::temp_sqlite::{SeedTicket, TempSqlite, TEST_NOW, TEST_USER_ID, TEST_USER_NAME};

use nexus_core::board::basics::{Priority, TicketStatus};
use nexus_core::board::database::{BoardDatabase, BoardFilter, NewBoard, NewProject, TicketFilter};
use nexus_core::board::types::{Board, BoardUser, StepProgress, Ticket, TicketStep};
use nexus_core::tx;
use nexus_db::sqlite_params;

/// Ein zweiter Benutzer für die Filter der Boards
const USER_B: &str = "22222222-2222-2222-2222-222222222222";

/// Das Projekt, mit dem die Projekt-Tests arbeiten
fn arc_project() -> NewProject {
    NewProject {
        key: "ARC".into(),
        name: "Arches".into(),
        created_at: TEST_NOW.into(),
        created_by: TEST_USER_ID.into(),
    }
}

/// Vollständiges Ticket mit Standardwerten (wie `ticketFixture` in TypeScript)
fn fx(id: &str, project_id: i32, board_id: i32, number: i32) -> Ticket {
    Ticket {
        id: id.into(),
        project_id,
        board_id,
        number,
        title: "Titel".into(),
        description: String::new(),
        status: TicketStatus::Backlog,
        priority: Priority::Normal,
        labels: Vec::new(),
        depends_on: Vec::new(),
        sort_order: 0,
        assignee: None,
        needs_planning: true,
        acceptance: String::new(),
        plan: String::new(),
        handbook: String::new(),
        diagram: String::new(),
        spec: None,
        spec_task: None,
        branch: None,
        worktree: None,
        target_branch: None,
        touches: Vec::new(),
        consumes: Vec::new(),
        produces: Vec::new(),
        blocked_reason: None,
        planned_at: None,
        planned_by: None,
        planning_claim_by: None,
        planning_claim_at: None,
        created_at: TEST_NOW.into(),
        created_by_id: TEST_USER_ID.into(),
        created_by_name: TEST_USER_NAME.into(),
        updated_at: TEST_NOW.into(),
        rev: 1,
    }
}

/// Ein Schritt ohne Anleitung
fn step(text: &str, done: bool) -> TicketStep {
    TicketStep {
        text: text.into(),
        done,
        detail: None,
    }
}

// ---------------------------------------------------------------- Benutzer

#[tokio::test]
async fn upsert_legt_an() {
    let temp = TempSqlite::create().expect("Temp-Datei");
    let user = BoardUser {
        id: TEST_USER_ID.into(),
        name: "Anna".into(),
    };
    tx!(temp.db(), [user], |u| { u.users().upsert(&user).await })
        .await
        .expect("upsert");

    let found = tx!(temp.db(), |u| { u.users().by_id(TEST_USER_ID).await })
        .await
        .expect("by_id");
    assert_eq!(
        found,
        Some(BoardUser {
            id: TEST_USER_ID.into(),
            name: "Anna".into()
        })
    );
    temp.dispose().await;
}

#[tokio::test]
async fn upsert_aktualisiert_den_namen_ohne_duplikat() {
    let temp = TempSqlite::create().expect("Temp-Datei");
    for name in ["Anna", "Berta"] {
        let user = BoardUser {
            id: TEST_USER_ID.into(),
            name: name.into(),
        };
        tx!(temp.db(), [user], |u| { u.users().upsert(&user).await })
            .await
            .expect("upsert");
    }

    let found = tx!(temp.db(), |u| { u.users().by_id(TEST_USER_ID).await })
        .await
        .expect("by_id");
    assert_eq!(found.expect("Benutzer").name, "Berta");
    assert_eq!(temp.count("Users").await.expect("Zahl"), 1);
    temp.dispose().await;
}

#[tokio::test]
async fn speichert_guids_klein() {
    let temp = TempSqlite::create().expect("Temp-Datei");
    const UPPER: &str = "AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA";
    let lower = UPPER.to_lowercase();
    let user = BoardUser {
        id: UPPER.into(),
        name: "Groß".into(),
    };
    tx!(temp.db(), [user], |u| { u.users().upsert(&user).await })
        .await
        .expect("upsert");

    assert_eq!(
        temp.text("SELECT Id FROM Users", Vec::new())
            .await
            .expect("Id"),
        Some(lower.clone())
    );
    let by_upper = tx!(temp.db(), |u| { u.users().by_id(UPPER).await })
        .await
        .expect("by_id");
    let by_lower = tx!(temp.db(), [lower], |u| { u.users().by_id(&lower).await })
        .await
        .expect("by_id");
    assert_eq!(by_upper.expect("Benutzer").id, lower);
    assert_eq!(by_lower.expect("Benutzer").id, lower);
    temp.dispose().await;
}

#[tokio::test]
async fn list_sortiert_nach_name_ohne_gross_klein() {
    let temp = TempSqlite::create().expect("Temp-Datei");
    for (id, name) in [
        ("22222222-2222-2222-2222-222222222222", "berta"),
        ("11111111-1111-1111-1111-111111111111", "Anna"),
        ("33333333-3333-3333-3333-333333333333", "Carl"),
    ] {
        let user = BoardUser {
            id: id.into(),
            name: name.into(),
        };
        tx!(temp.db(), [user], |u| { u.users().upsert(&user).await })
            .await
            .expect("upsert");
    }

    let namen: Vec<String> = tx!(temp.db(), |u| { u.users().list().await })
        .await
        .expect("list")
        .into_iter()
        .map(|user| user.name)
        .collect();
    assert_eq!(namen, ["Anna", "berta", "Carl"]);
    temp.dispose().await;
}

#[tokio::test]
async fn by_id_unbekannt_liefert_nichts() {
    let temp = TempSqlite::create().expect("Temp-Datei");
    let found = tx!(temp.db(), |u| {
        u.users()
            .by_id("99999999-9999-9999-9999-999999999999")
            .await
    })
    .await
    .expect("by_id");
    assert_eq!(found, None);
    temp.dispose().await;
}

// ---------------------------------------------------------------- Projekte

#[tokio::test]
async fn projekt_anlegen_liefert_id() {
    let temp = TempSqlite::create().expect("Temp-Datei");
    temp.seed_user().await.expect("Benutzer");
    let arc = arc_project();
    let p = tx!(temp.db(), [arc], |u| { u.projects().insert(&arc).await })
        .await
        .expect("insert");
    assert!(p.id > 0);
    assert_eq!(p.next_ticket_number, 1);
    assert_eq!(p.next_question_number, 1);

    let id = p.id;
    assert_eq!(
        tx!(temp.db(), |u| { u.projects().by_id(id).await })
            .await
            .expect("by_id"),
        Some(p.clone())
    );
    assert_eq!(
        tx!(temp.db(), |u| { u.projects().by_key("ARC").await })
            .await
            .expect("by_key"),
        Some(p)
    );
    assert_eq!(
        tx!(temp.db(), |u| { u.projects().by_key("NOPE").await })
            .await
            .expect("by_key"),
        None
    );
    assert_eq!(
        tx!(temp.db(), |u| { u.projects().by_id(9999).await })
            .await
            .expect("by_id"),
        None
    );
    temp.dispose().await;
}

#[tokio::test]
async fn doppelter_key_fuehrt_zu_eindeutigkeitsfehler() {
    let temp = TempSqlite::create().expect("Temp-Datei");
    temp.seed_user().await.expect("Benutzer");
    let arc = arc_project();
    tx!(temp.db(), [arc], |u| { u.projects().insert(&arc).await })
        .await
        .expect("insert");

    let fehler = tx!(temp.db(), [arc], |u| { u.projects().insert(&arc).await })
        .await
        .expect_err("zweites Anlegen");
    assert_eq!(fehler.code(), Some("project-exists"));
    assert_eq!(fehler.to_string(), "Projekt ARC gibt es schon.");
    assert_eq!(temp.count("Projects").await.expect("Zahl"), 1);
    temp.dispose().await;
}

#[tokio::test]
async fn zwei_ticketnummern_hintereinander_sind_fortlaufend() {
    let temp = TempSqlite::create().expect("Temp-Datei");
    temp.seed_user().await.expect("Benutzer");
    let arc = arc_project();
    let id = tx!(temp.db(), [arc], |u| { u.projects().insert(&arc).await })
        .await
        .expect("insert")
        .id;

    for erwartet in [1, 2] {
        let nummer = tx!(temp.db(), |u| { u.projects().take_ticket_number(id).await })
            .await
            .expect("Nummer");
        assert_eq!(nummer, erwartet);
    }
    let wieder = tx!(temp.db(), |u| { u.projects().by_id(id).await })
        .await
        .expect("by_id");
    assert_eq!(wieder.expect("Projekt").next_ticket_number, 3);
    temp.dispose().await;
}

#[tokio::test]
async fn fragenummern_zaehlen_unabhaengig_von_ticketnummern() {
    let temp = TempSqlite::create().expect("Temp-Datei");
    temp.seed_user().await.expect("Benutzer");
    let arc = arc_project();
    let id = tx!(temp.db(), [arc], |u| { u.projects().insert(&arc).await })
        .await
        .expect("insert")
        .id;

    for erwartet in [1, 2] {
        let nummer = tx!(temp.db(), |u| {
            u.projects().take_question_number(id).await
        })
        .await
        .expect("Nummer");
        assert_eq!(nummer, erwartet);
    }
    let wieder = tx!(temp.db(), |u| { u.projects().by_id(id).await })
        .await
        .expect("by_id");
    assert_eq!(wieder.expect("Projekt").next_ticket_number, 1);
    temp.dispose().await;
}

#[tokio::test]
async fn rename_aendert_den_namen() {
    let temp = TempSqlite::create().expect("Temp-Datei");
    temp.seed_user().await.expect("Benutzer");
    let arc = arc_project();
    let id = tx!(temp.db(), [arc], |u| { u.projects().insert(&arc).await })
        .await
        .expect("insert")
        .id;
    tx!(temp.db(), |u| { u.projects().rename(id, "Neu").await })
        .await
        .expect("rename");

    let wieder = tx!(temp.db(), |u| { u.projects().by_id(id).await })
        .await
        .expect("by_id");
    assert_eq!(wieder.expect("Projekt").name, "Neu");
    temp.dispose().await;
}

#[tokio::test]
async fn projekte_sortieren_nach_key() {
    let temp = TempSqlite::create().expect("Temp-Datei");
    temp.seed_user().await.expect("Benutzer");
    let zed = NewProject {
        key: "ZED".into(),
        ..arc_project()
    };
    tx!(temp.db(), [zed], |u| { u.projects().insert(&zed).await })
        .await
        .expect("insert");
    let arc = arc_project();
    tx!(temp.db(), [arc], |u| { u.projects().insert(&arc).await })
        .await
        .expect("insert");

    let keys: Vec<String> = tx!(temp.db(), |u| { u.projects().list().await })
        .await
        .expect("list")
        .into_iter()
        .map(|p| p.key)
        .collect();
    assert_eq!(keys, ["ARC", "ZED"]);
    temp.dispose().await;
}

// ---------------------------------------------------------------- Boards

/// Ein neues Board des Projekts mit dem Test-Benutzer als Eigner
fn sprint(project_id: i32) -> NewBoard {
    NewBoard {
        project_id,
        owner_id: TEST_USER_ID.into(),
        name: "Sprint".into(),
        repo_path: None,
        created_at: TEST_NOW.into(),
    }
}

#[tokio::test]
async fn board_anlegen_und_lesen() {
    let temp = TempSqlite::create().expect("Temp-Datei");
    let project_id = temp.seed_project("ARC").await.expect("Projekt");
    let neu = sprint(project_id);

    let b = tx!(temp.db(), [neu], |u| { u.boards().insert(&neu).await })
        .await
        .expect("insert");
    assert!(b.id > 0);
    let id = b.id;
    assert_eq!(
        tx!(temp.db(), |u| { u.boards().by_id(id).await })
            .await
            .expect("by_id"),
        Some(b)
    );
    assert_eq!(
        tx!(temp.db(), |u| { u.boards().by_id(9999).await })
            .await
            .expect("by_id"),
        None
    );
    temp.dispose().await;
}

#[tokio::test]
async fn boards_auflisten_mit_filtern() {
    let temp = TempSqlite::create().expect("Temp-Datei");
    let project_id = temp.seed_project("ARC").await.expect("Projekt");
    temp.execute(
        "INSERT INTO Users (Id, Name) VALUES (@id, @name) ON CONFLICT(Id) DO NOTHING",
        sqlite_params! { "id": USER_B, "name": "Zweiter" },
    )
    .await
    .expect("zweiter Benutzer");

    let eins = sprint(project_id);
    let b1 = tx!(temp.db(), [eins], |u| { u.boards().insert(&eins).await })
        .await
        .expect("insert");
    let zwei = NewBoard {
        owner_id: USER_B.into(),
        repo_path: Some("C:/repo".into()),
        ..sprint(project_id)
    };
    let b2 = tx!(temp.db(), [zwei], |u| { u.boards().insert(&zwei).await })
        .await
        .expect("insert");
    let project_zed = temp.seed_project("ZED").await.expect("Projekt");
    let drei = sprint(project_zed);
    let b3 = tx!(temp.db(), [drei], |u| { u.boards().insert(&drei).await })
        .await
        .expect("insert");

    let alle = boards(&temp, BoardFilter::default()).await;
    assert_eq!(
        alle.iter().map(|b| b.id).collect::<Vec<_>>(),
        [b1.id, b2.id, b3.id]
    );
    assert_eq!(
        boards(
            &temp,
            BoardFilter {
                project_id: Some(project_id),
                owner_id: None
            }
        )
        .await,
        [b1.clone(), b2.clone()]
    );
    assert_eq!(
        boards(
            &temp,
            BoardFilter {
                project_id: None,
                owner_id: Some(USER_B.into())
            }
        )
        .await,
        std::slice::from_ref(&b2)
    );
    assert_eq!(
        boards(
            &temp,
            BoardFilter {
                project_id: None,
                owner_id: Some(USER_B.to_uppercase())
            }
        )
        .await,
        [b2]
    );
    assert_eq!(
        boards(
            &temp,
            BoardFilter {
                project_id: Some(project_id),
                owner_id: Some(TEST_USER_ID.into())
            }
        )
        .await,
        [b1]
    );
    temp.dispose().await;
}

/// Die Boards zu einem Filter
async fn boards(temp: &TempSqlite, filter: BoardFilter) -> Vec<Board> {
    tx!(temp.db(), [filter], |u| { u.boards().list(&filter).await })
        .await
        .expect("list")
}

#[tokio::test]
async fn board_aendern_wie_im_mssql_repository() {
    let temp = TempSqlite::create().expect("Temp-Datei");
    let project_id = temp.seed_project("ARC").await.expect("Projekt");
    let neu = sprint(project_id);
    let b1 = tx!(temp.db(), [neu], |u| { u.boards().insert(&neu).await })
        .await
        .expect("insert");

    let m0 = temp.db().change_marker(b1.id).await.expect("Kennwert");
    let geaendert = Board {
        name: "Neu".into(),
        repo_path: Some("D:/x".into()),
        ..b1.clone()
    };
    tx!(temp.db(), [geaendert], |u| {
        u.boards().update(&geaendert).await
    })
    .await
    .expect("update");

    let id = b1.id;
    let wieder = tx!(temp.db(), |u| { u.boards().by_id(id).await })
        .await
        .expect("by_id")
        .expect("Board");
    assert_eq!(wieder.name, "Neu");
    assert_eq!(wieder.repo_path.as_deref(), Some("D:/x"));
    assert_eq!(wieder.id, b1.id);
    assert_eq!(wieder.project_id, b1.project_id);
    assert_eq!(wieder.owner_id, b1.owner_id);
    assert_eq!(wieder.created_at, b1.created_at);
    assert_ne!(temp.db().change_marker(id).await.expect("Kennwert"), m0);
    temp.dispose().await;
}

#[tokio::test]
async fn board_loeschen_entfernt_notizen() {
    let temp = TempSqlite::create().expect("Temp-Datei");
    let project_id = temp.seed_project("ARC").await.expect("Projekt");
    let neu = sprint(project_id);
    let b1 = tx!(temp.db(), [neu], |u| { u.boards().insert(&neu).await })
        .await
        .expect("insert");
    temp.execute(
        "INSERT INTO Notes (BoardId, AuthorId, AuthorName, Body, State, CreatedAt) VALUES (@b, @u, 'Test', 'Notiz', 'open', @now)",
        sqlite_params! { "b": b1.id, "u": TEST_USER_ID, "now": TEST_NOW },
    )
    .await
    .expect("Notiz");

    let id = b1.id;
    tx!(temp.db(), |u| { u.boards().delete(id).await })
        .await
        .expect("delete");
    assert_eq!(
        tx!(temp.db(), |u| { u.boards().by_id(id).await })
            .await
            .expect("by_id"),
        None
    );
    assert_eq!(temp.count("Notes").await.expect("Zahl"), 0);
    temp.dispose().await;
}

// ---------------------------------------------------------------- Tickets

/// Temp-Datei mit Projekt, Board „Sprint“ und einem zweiten Board „Andere“
async fn mit_zwei_boards() -> (TempSqlite, i32, i32, i32) {
    let temp = TempSqlite::create().expect("Temp-Datei");
    let board = temp.seed_board(None, "Sprint").await.expect("Board");
    let andere = temp
        .seed_board(Some(board.project_id), "Andere")
        .await
        .expect("Board")
        .board_id;
    (temp, board.project_id, board.board_id, andere)
}

#[tokio::test]
async fn legt_ein_ticket_an_und_liest_es_mit_allen_feldern() {
    let (temp, project_id, board_id, _) = mit_zwei_boards().await;
    let t1 = Ticket {
        labels: vec!["zwei".into(), "eins".into()],
        touches: vec!["packages/db".into()],
        consumes: vec!["A".into()],
        produces: vec!["B".into()],
        spec: Some("001".into()),
        spec_task: Some("T001".into()),
        branch: Some("ticket/ARC-1".into()),
        worktree: Some("C:/wt".into()),
        assignee: Some("claude".into()),
        needs_planning: false,
        planned_at: Some("2026-10-14T08:30:00.000Z".into()),
        planned_by: Some("claude".into()),
        blocked_reason: Some("Grund".into()),
        plan: "# Plan".into(),
        handbook: "H".into(),
        diagram: "flowchart LR".into(),
        ..fx("ARC-1", project_id, board_id, 1)
    };
    tx!(temp.db(), [t1], |u| { u.tickets().insert(&t1).await })
        .await
        .expect("insert");

    let gelesen = tx!(temp.db(), |u| { u.tickets().by_id("ARC-1", false).await })
        .await
        .expect("by_id");
    assert_eq!(gelesen, Some(t1.clone()));
    // for_update ist ein No-op und liefert dasselbe Ticket
    let gesperrt = tx!(temp.db(), |u| { u.tickets().by_id("ARC-1", true).await })
        .await
        .expect("by_id");
    assert_eq!(gesperrt, Some(t1));
    let fehlt = tx!(temp.db(), |u| { u.tickets().by_id("ARC-9", false).await })
        .await
        .expect("by_id");
    assert_eq!(fehlt, None);
    temp.dispose().await;
}

#[tokio::test]
async fn behaelt_die_reihenfolge_der_voraussetzungen() {
    let (temp, project_id, board_id, _) = mit_zwei_boards().await;
    let eins = fx("ARC-1", project_id, board_id, 1);
    let zwei = fx("ARC-2", project_id, board_id, 2);
    let drei = Ticket {
        depends_on: vec!["ARC-2".into(), "ARC-1".into()],
        ..fx("ARC-3", project_id, board_id, 3)
    };
    tx!(temp.db(), [eins, zwei, drei], |u| {
        u.tickets().insert(&eins).await?;
        u.tickets().insert(&zwei).await?;
        u.tickets().insert(&drei).await
    })
    .await
    .expect("insert");

    let gelesen = tx!(temp.db(), |u| { u.tickets().by_id("ARC-3", false).await })
        .await
        .expect("by_id")
        .expect("Ticket");
    assert_eq!(gelesen.depends_on, ["ARC-2", "ARC-1"]);
    assert_eq!(
        tx!(temp.db(), |u| { u.tickets().dependents("ARC-1").await })
            .await
            .expect("dependents"),
        ["ARC-3"]
    );
    temp.dispose().await;
}

#[tokio::test]
async fn filtert_und_sortiert_in_list() {
    let (temp, project_id, board_id, andere) = mit_zwei_boards().await;
    let eins = Ticket {
        labels: vec!["a".into()],
        ..fx("ARC-1", project_id, board_id, 1)
    };
    let zwei = fx("ARC-2", project_id, andere, 2);
    let drei = Ticket {
        labels: vec!["b".into()],
        ..fx("ARC-3", project_id, board_id, 3)
    };
    tx!(temp.db(), [eins, zwei, drei], |u| {
        u.tickets().insert(&eins).await?;
        u.tickets().insert(&zwei).await?;
        u.tickets().insert(&drei).await
    })
    .await
    .expect("insert");

    let alle = tickets(&temp, TicketFilter::default()).await;
    assert_eq!(ids(&alle), ["ARC-1", "ARC-2", "ARC-3"]);

    let im_board = tickets(
        &temp,
        TicketFilter {
            board_ids: Some(vec![board_id]),
            ..TicketFilter::default()
        },
    )
    .await;
    assert_eq!(ids(&im_board), ["ARC-1", "ARC-3"]);
    assert_eq!(
        im_board
            .iter()
            .map(|t| t.labels.clone())
            .collect::<Vec<_>>(),
        [vec!["a".to_string()], vec!["b".to_string()]]
    );

    let nach_ids = tickets(
        &temp,
        TicketFilter {
            ids: Some(vec!["ARC-3".into(), "ARC-1".into()]),
            ..TicketFilter::default()
        },
    )
    .await;
    assert_eq!(ids(&nach_ids), ["ARC-1", "ARC-3"]);

    assert!(tickets(
        &temp,
        TicketFilter {
            board_ids: Some(Vec::new()),
            ..TicketFilter::default()
        }
    )
    .await
    .is_empty());
    assert!(tickets(
        &temp,
        TicketFilter {
            ids: Some(Vec::new()),
            ..TicketFilter::default()
        }
    )
    .await
    .is_empty());
    temp.dispose().await;
}

#[tokio::test]
async fn ohne_lange_texte_bleibt_nur_das_kennzeichen() {
    let (temp, project_id, board_id, _) = mit_zwei_boards().await;
    let voll = Ticket {
        description: "lang".into(),
        acceptance: "   ".into(),
        plan: "# Plan".into(),
        handbook: "H".into(),
        diagram: String::new(),
        ..fx("ARC-1", project_id, board_id, 1)
    };
    tx!(temp.db(), [voll], |u| { u.tickets().insert(&voll).await })
        .await
        .expect("insert");

    let leicht = tickets(
        &temp,
        TicketFilter {
            without_texts: true,
            ..TicketFilter::default()
        },
    )
    .await;
    let t = leicht.first().expect("Ticket");
    assert_eq!(t.description, "x");
    assert_eq!(t.acceptance, "");
    assert_eq!(t.plan, "x");
    assert_eq!(t.handbook, "x");
    assert_eq!(t.diagram, "");
    temp.dispose().await;
}

/// Die Tickets zu einem Filter
async fn tickets(temp: &TempSqlite, filter: TicketFilter) -> Vec<Ticket> {
    tx!(temp.db(), [filter], |u| { u.tickets().list(&filter).await })
        .await
        .expect("list")
}

/// Die IDs einer Ticketliste
fn ids(liste: &[Ticket]) -> Vec<String> {
    liste.iter().map(|t| t.id.clone()).collect()
}

#[tokio::test]
async fn aendert_ein_ticket_und_lehnt_eine_veraltete_version_als_conflict_ab() {
    let (temp, project_id, board_id, _) = mit_zwei_boards().await;
    let t = Ticket {
        labels: vec!["a".into()],
        ..fx("ARC-1", project_id, board_id, 1)
    };
    tx!(temp.db(), [t], |u| { u.tickets().insert(&t).await })
        .await
        .expect("insert");

    let neu = Ticket {
        title: "Neu".into(),
        labels: vec!["x".into()],
        rev: 2,
        ..t.clone()
    };
    tx!(temp.db(), [neu], |u| { u.tickets().update(&neu).await })
        .await
        .expect("update");
    let gelesen = tx!(temp.db(), |u| { u.tickets().by_id("ARC-1", false).await })
        .await
        .expect("by_id")
        .expect("Ticket");
    assert_eq!(gelesen.title, "Neu");
    assert_eq!(gelesen.labels, ["x"]);
    assert_eq!(gelesen.rev, 2);

    let alt = Ticket {
        title: "Alt".into(),
        rev: 2,
        ..t
    };
    let fehler = tx!(temp.db(), [alt], |u| { u.tickets().update(&alt).await })
        .await
        .expect_err("veraltete Version");
    assert_eq!(fehler.code(), Some("conflict"));
    assert_eq!(
        fehler.to_string(),
        "Ticket ARC-1 wurde inzwischen geändert. Bitte neu lesen."
    );
    let wieder = tx!(temp.db(), |u| { u.tickets().by_id("ARC-1", false).await })
        .await
        .expect("by_id")
        .expect("Ticket");
    assert_eq!(wieder.title, "Neu");
    temp.dispose().await;
}

#[tokio::test]
async fn verschiebt_ein_ticket_in_ein_anderes_board() {
    let (temp, project_id, board_id, andere) = mit_zwei_boards().await;
    let t = fx("ARC-1", project_id, board_id, 1);
    tx!(temp.db(), [t], |u| { u.tickets().insert(&t).await })
        .await
        .expect("insert");

    let verschoben = Ticket {
        board_id: andere,
        status: TicketStatus::InProgress,
        sort_order: 5,
        rev: 2,
        ..t
    };
    tx!(temp.db(), [verschoben], |u| {
        u.tickets().update(&verschoben).await
    })
    .await
    .expect("update");

    let im_anderen = tickets(
        &temp,
        TicketFilter {
            board_ids: Some(vec![andere]),
            ..TicketFilter::default()
        },
    )
    .await;
    assert_eq!(ids(&im_anderen), ["ARC-1"]);
    assert_eq!(im_anderen[0].status, TicketStatus::InProgress);
    assert_eq!(
        tx!(temp.db(), |u| {
            u.tickets()
                .max_sort_order(andere, TicketStatus::InProgress)
                .await
        })
        .await
        .expect("max_sort_order"),
        Some(5)
    );
    assert_eq!(
        tx!(temp.db(), |u| {
            u.tickets()
                .max_sort_order(board_id, TicketStatus::Backlog)
                .await
        })
        .await
        .expect("max_sort_order"),
        None
    );
    temp.dispose().await;
}

#[tokio::test]
async fn zaehlt_die_offenen_tickets_je_board() {
    let (temp, project_id, board_id, andere) = mit_zwei_boards().await;
    let offen = fx("ARC-1", project_id, board_id, 1);
    let erledigt = Ticket {
        status: TicketStatus::Done,
        ..fx("ARC-2", project_id, board_id, 2)
    };
    let im_anderen = fx("ARC-3", project_id, andere, 3);
    tx!(temp.db(), [offen, erledigt, im_anderen], |u| {
        u.tickets().insert(&offen).await?;
        u.tickets().insert(&erledigt).await?;
        u.tickets().insert(&im_anderen).await
    })
    .await
    .expect("insert");

    let zahlen = tx!(temp.db(), |u| {
        u.tickets().open_count_by_board(&[board_id, andere]).await
    })
    .await
    .expect("open_count_by_board");
    assert_eq!(zahlen.get(&board_id), Some(&1));
    assert_eq!(zahlen.get(&andere), Some(&1));
    assert!(tx!(temp.db(), |u| {
        u.tickets().open_count_by_board(&[]).await
    })
    .await
    .expect("open_count_by_board")
    .is_empty());
    temp.dispose().await;
}

#[tokio::test]
async fn entfernt_verweise_auf_ein_ticket_und_liefert_deren_ids() {
    let (temp, project_id, board_id, _) = mit_zwei_boards().await;
    let eins = fx("ARC-1", project_id, board_id, 1);
    let zwei = Ticket {
        depends_on: vec!["ARC-1".into()],
        ..fx("ARC-2", project_id, board_id, 2)
    };
    let drei = Ticket {
        depends_on: vec!["ARC-1".into()],
        ..fx("ARC-3", project_id, board_id, 3)
    };
    tx!(temp.db(), [eins, zwei, drei], |u| {
        u.tickets().insert(&eins).await?;
        u.tickets().insert(&zwei).await?;
        u.tickets().insert(&drei).await
    })
    .await
    .expect("insert");

    assert_eq!(
        tx!(temp.db(), |u| {
            u.tickets().remove_dependency_on("ARC-1").await
        })
        .await
        .expect("remove_dependency_on"),
        ["ARC-2", "ARC-3"]
    );
    assert!(
        tx!(temp.db(), |u| { u.tickets().dependents("ARC-1").await })
            .await
            .expect("dependents")
            .is_empty()
    );
    let zwei = tx!(temp.db(), |u| { u.tickets().by_id("ARC-2", false).await })
        .await
        .expect("by_id")
        .expect("Ticket");
    assert!(zwei.depends_on.is_empty());
    temp.dispose().await;
}

#[tokio::test]
async fn loescht_ein_ticket_samt_verweisen_und_kindzeilen() {
    let (temp, project_id, board_id, _) = mit_zwei_boards().await;
    let eins = Ticket {
        labels: vec!["a".into()],
        ..fx("ARC-1", project_id, board_id, 1)
    };
    let zwei = Ticket {
        depends_on: vec!["ARC-1".into()],
        ..fx("ARC-2", project_id, board_id, 2)
    };
    tx!(temp.db(), [eins, zwei], |u| {
        u.tickets().insert(&eins).await?;
        u.tickets().insert(&zwei).await
    })
    .await
    .expect("insert");

    tx!(temp.db(), |u| { u.tickets().delete("ARC-1").await })
        .await
        .expect("delete");

    assert_eq!(
        tx!(temp.db(), |u| { u.tickets().by_id("ARC-1", false).await })
            .await
            .expect("by_id"),
        None
    );
    let zwei = tx!(temp.db(), |u| { u.tickets().by_id("ARC-2", false).await })
        .await
        .expect("by_id")
        .expect("Ticket");
    assert!(zwei.depends_on.is_empty());
    assert_eq!(
        temp.number(
            "SELECT COUNT(*) AS N FROM TicketLabels WHERE TicketId = 'ARC-1'",
            Vec::new()
        )
        .await
        .expect("Zahl"),
        Some(0)
    );
    temp.dispose().await;
}

// ---------------------------------------------------------------- Arbeitsschritte

/// Temp-Datei mit Projekt, Board und den genannten Tickets
async fn mit_tickets(anzahl: i32) -> TempSqlite {
    let temp = TempSqlite::create().expect("Temp-Datei");
    let board = temp.seed_board(None, "Sprint").await.expect("Board");
    for number in 1..=anzahl {
        temp.seed_ticket(&SeedTicket::new(&format!("ARC-{number}"), &board, number))
            .await
            .expect("Ticket");
    }
    temp
}

/// Die Positionen der Schritte von ARC-1 in der Datenbank
async fn positions(temp: &TempSqlite) -> Vec<i64> {
    temp.rows(
        "SELECT Position FROM TicketSteps WHERE TicketId = 'ARC-1' ORDER BY Position",
        Vec::new(),
        |row| row.get::<_, i64>(0),
    )
    .await
    .expect("Positionen")
}

/// Die Texte der Schritte von ARC-1 über das Repository
async fn texts(temp: &TempSqlite) -> Vec<String> {
    tx!(temp.db(), |u| { u.steps().list("ARC-1").await })
        .await
        .expect("list")
        .into_iter()
        .map(|s| s.text)
        .collect()
}

/// Setzt die Schritte von ARC-1
async fn replace(temp: &TempSqlite, steps: Vec<TicketStep>) {
    tx!(temp.db(), [steps], |u| {
        u.steps().replace("ARC-1", &steps).await
    })
    .await
    .expect("replace");
}

#[tokio::test]
async fn legt_schritte_an() {
    let temp = mit_tickets(1).await;
    let c = TicketStep {
        detail: Some("npm test".into()),
        ..step("c", false)
    };
    replace(&temp, vec![step("a", false), step("b", true), c.clone()]).await;

    assert_eq!(
        tx!(temp.db(), |u| { u.steps().list("ARC-1").await })
            .await
            .expect("list"),
        [step("a", false), step("b", true), c]
    );
    assert_eq!(positions(&temp).await, [0, 1, 2]);
    temp.dispose().await;
}

#[tokio::test]
async fn fuegt_an_einer_position_ein() {
    let temp = mit_tickets(1).await;
    let c = TicketStep {
        detail: Some("npm test".into()),
        ..step("c", false)
    };
    replace(&temp, vec![step("a", false), step("b", true), c.clone()]).await;

    replace(
        &temp,
        vec![step("a", false), step("x", false), step("b", true), c],
    )
    .await;

    assert_eq!(texts(&temp).await, ["a", "x", "b", "c"]);
    assert_eq!(positions(&temp).await, [0, 1, 2, 3]);
    let liste = tx!(temp.db(), |u| { u.steps().list("ARC-1").await })
        .await
        .expect("list");
    assert!(liste[2].done);
    temp.dispose().await;
}

#[tokio::test]
async fn verschiebt_schritte() {
    let temp = mit_tickets(1).await;
    let c = step("c", false);
    replace(
        &temp,
        vec![
            step("a", false),
            step("x", false),
            step("b", true),
            c.clone(),
        ],
    )
    .await;

    replace(
        &temp,
        vec![c, step("a", false), step("x", false), step("b", true)],
    )
    .await;

    assert_eq!(texts(&temp).await, ["c", "a", "x", "b"]);
    assert_eq!(positions(&temp).await, [0, 1, 2, 3]);
    temp.dispose().await;
}

#[tokio::test]
async fn loescht_schritte_nummern_bleiben_lueckenlos() {
    let temp = mit_tickets(1).await;
    replace(
        &temp,
        vec![
            step("c", false),
            step("a", false),
            step("x", false),
            step("b", true),
        ],
    )
    .await;

    replace(&temp, vec![step("c", false), step("b", true)]).await;
    assert_eq!(texts(&temp).await, ["c", "b"]);
    assert_eq!(positions(&temp).await, [0, 1]);

    replace(&temp, Vec::new()).await;
    assert!(tx!(temp.db(), |u| { u.steps().list("ARC-1").await })
        .await
        .expect("list")
        .is_empty());
    assert!(positions(&temp).await.is_empty());
    temp.dispose().await;
}

#[tokio::test]
async fn setzt_den_haken_eines_schritts() {
    let temp = mit_tickets(1).await;
    replace(&temp, vec![step("a", false), step("b", true)]).await;

    for done in [true, false] {
        assert!(tx!(temp.db(), |u| {
            u.steps().set_done("ARC-1", 0, done).await
        })
        .await
        .expect("set_done"));
        let liste = tx!(temp.db(), |u| { u.steps().list("ARC-1").await })
            .await
            .expect("list");
        assert_eq!(liste[0].done, done);
    }

    assert!(!tx!(temp.db(), |u| {
        u.steps().set_done("ARC-1", 9, true).await
    })
    .await
    .expect("set_done"));
    temp.dispose().await;
}

#[tokio::test]
async fn liefert_den_fortschritt_je_ticket() {
    let temp = mit_tickets(3).await;
    replace(&temp, vec![step("b", true), step("a", false)]).await;
    let eins = vec![step("b", true)];
    tx!(temp.db(), [eins], |u| {
        u.steps().replace("ARC-2", &eins).await
    })
    .await
    .expect("replace");

    let fortschritt = tx!(temp.db(), |u| {
        u.steps()
            .progress(&["ARC-1".to_string(), "ARC-2".into(), "ARC-3".into()])
            .await
    })
    .await
    .expect("progress");
    assert_eq!(fortschritt.len(), 2);
    assert_eq!(
        fortschritt.get("ARC-1"),
        Some(&StepProgress { done: 1, total: 2 })
    );
    assert_eq!(
        fortschritt.get("ARC-2"),
        Some(&StepProgress { done: 1, total: 1 })
    );
    assert!(tx!(temp.db(), |u| { u.steps().progress(&[]).await })
        .await
        .expect("progress")
        .is_empty());
    temp.dispose().await;
}

#[tokio::test]
async fn schritte_fallen_mit_dem_ticket() {
    let temp = mit_tickets(1).await;
    replace(&temp, vec![step("a", false)]).await;

    temp.execute("DELETE FROM Tickets WHERE Id = 'ARC-1'", Vec::new())
        .await
        .expect("delete");

    assert_eq!(temp.count("TicketSteps").await.expect("Zahl"), 0);
    temp.dispose().await;
}

// ---------------------------------------------------------------- Ticket-Sitzungen

/// Eine Stunde vor [`TEST_NOW`]
const T9: &str = "2026-10-13T09:00:00.000Z";
/// Eine Stunde nach [`TEST_NOW`]
const T11: &str = "2026-10-13T11:00:00.000Z";

#[tokio::test]
async fn erzeugt_bei_doppeltem_eintragen_derselben_session_keine_zweite_zeile() {
    let temp = mit_tickets(1).await;

    assert!(tx!(temp.db(), |u| {
        u.ticket_sessions().add("ARC-1", "s1", TEST_NOW).await
    })
    .await
    .expect("add"));
    assert!(!tx!(temp.db(), |u| {
        u.ticket_sessions().add("ARC-1", "s1", T11).await
    })
    .await
    .expect("add"));

    assert_eq!(
        temp.number(
            "SELECT COUNT(*) AS N FROM TicketSessions WHERE TicketId = 'ARC-1' AND SessionId = 's1'",
            Vec::new()
        )
        .await
        .expect("Zahl"),
        Some(1)
    );
    assert_eq!(
        temp.text("SELECT AddedAt FROM TicketSessions", Vec::new())
            .await
            .expect("AddedAt"),
        Some(TEST_NOW.to_string())
    );
    temp.dispose().await;
}

/// Verknüpft s1 und s2 mit ARC-1 und s1 mit ARC-2
async fn seed_sessions(temp: &TempSqlite) {
    for (ticket, session, at) in [
        ("ARC-1", "s1", TEST_NOW),
        ("ARC-1", "s2", T9),
        ("ARC-2", "s1", T11),
    ] {
        tx!(temp.db(), [ticket, session, at], |u| {
            u.ticket_sessions().add(ticket, session, at).await
        })
        .await
        .expect("add");
    }
}

#[tokio::test]
async fn liefert_by_tickets_in_der_reihenfolge_der_verknuepfung() {
    let temp = mit_tickets(3).await;
    seed_sessions(&temp).await;

    let result = tx!(temp.db(), |u| {
        u.ticket_sessions()
            .by_tickets(&["ARC-1".to_string(), "ARC-2".into(), "ARC-3".into()])
            .await
    })
    .await
    .expect("by_tickets");
    assert_eq!(result.len(), 2);
    assert_eq!(
        result.get("ARC-1"),
        Some(&vec!["s2".to_string(), "s1".to_string()])
    );
    assert_eq!(result.get("ARC-2"), Some(&vec!["s1".to_string()]));

    assert!(
        tx!(temp.db(), |u| { u.ticket_sessions().by_tickets(&[]).await })
            .await
            .expect("by_tickets")
            .is_empty()
    );
    temp.dispose().await;
}

#[tokio::test]
async fn liefert_tickets_of_sortiert() {
    let temp = mit_tickets(3).await;
    seed_sessions(&temp).await;

    assert_eq!(
        tx!(temp.db(), |u| {
            u.ticket_sessions().tickets_of("s1").await
        })
        .await
        .expect("tickets_of"),
        ["ARC-1", "ARC-2"]
    );
    assert!(tx!(temp.db(), |u| {
        u.ticket_sessions().tickets_of("unbekannt").await
    })
    .await
    .expect("tickets_of")
    .is_empty());
    temp.dispose().await;
}

#[tokio::test]
async fn sitzung_aendert_den_aenderungskennwert() {
    let temp = TempSqlite::create().expect("Temp-Datei");
    let board = temp.seed_board(None, "Sprint").await.expect("Board");
    temp.seed_ticket(&SeedTicket::new("ARC-3", &board, 3))
        .await
        .expect("Ticket");

    let m0 = temp
        .db()
        .change_marker(board.board_id)
        .await
        .expect("Kennwert");
    tx!(temp.db(), |u| {
        u.ticket_sessions().add("ARC-3", "s9", TEST_NOW).await
    })
    .await
    .expect("add");
    let m1 = temp
        .db()
        .change_marker(board.board_id)
        .await
        .expect("Kennwert");
    assert_ne!(m1, m0);
    temp.dispose().await;
}

#[tokio::test]
async fn laesst_verknuepfungen_mit_dem_ticket_entfallen() {
    let temp = mit_tickets(2).await;
    tx!(temp.db(), |u| {
        u.ticket_sessions().add("ARC-1", "s1", TEST_NOW).await
    })
    .await
    .expect("add");
    tx!(temp.db(), |u| {
        u.ticket_sessions().add("ARC-2", "s1", T11).await
    })
    .await
    .expect("add");

    temp.execute("DELETE FROM Tickets WHERE Id = 'ARC-1'", Vec::new())
        .await
        .expect("delete");

    assert_eq!(
        tx!(temp.db(), |u| {
            u.ticket_sessions().tickets_of("s1").await
        })
        .await
        .expect("tickets_of"),
        ["ARC-2"]
    );
    temp.dispose().await;
}
