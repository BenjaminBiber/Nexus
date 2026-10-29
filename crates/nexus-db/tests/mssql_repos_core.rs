//! Benutzer, Projekte, Boards, Tickets, Arbeitsschritte und Ticket-Sitzungen auf SQL Server.
//!
//! Zwei Teile:
//!
//! * **ohne Datenbank** – mit dem `FakeRunner` aus `nexus_db::mssql::testing`: die Ticket-Fälle
//!   aus `packages/db/tests/MssqlFeatureGate.test.ts` (Spalte `TargetBranch` mit und ohne
//!   Erweiterung) sowie die SQL-Texte, die sich nicht am Inhalt der Datenbank zeigen
//!   (`UPDLOCK` nur bei `for_update`, leere ID-Listen setzen kein SQL ab);
//! * **mit Datenbank** – nur mit gesetztem `HARNESS_TEST_DB`: je Repository eine Rundreise
//!   gegen `AiHarness_Test`, wie die SQLite-Fassungen in `sqlite_repos_core.rs`. Ohne die
//!   Variable melden sie „übersprungen“ und kehren zurück.
//!
//! Aufruf der Rundreisen:
//! `HARNESS_TEST_DB='<json>' cargo test -p nexus-db --test mssql_repos_core -- --test-threads=1`
//! (seriell, weil alle Tests dieselbe Datenbank leeren).

mod common;

use common::test_db;

use nexus_core::board::basics::{Priority, TicketStatus};
use nexus_core::board::database::{
    BoardDatabase, BoardFilter, NewBoard, NewProject, TicketFilter, TicketRepository,
    TicketSessionRepository, TicketStepRepository,
};
use nexus_core::board::types::{Board, BoardUser, StepProgress, Ticket, TicketStep};
use nexus_core::tx;
use nexus_db::mssql::testing::FakeRunner;
use nexus_db::mssql::{MssqlBoardDatabase, MssqlUnitOfWork};
use nexus_db::ConnectOptions;

/// Zeitpunkt, mit dem die Tests schreiben
const NOW: &str = "2026-01-01T00:00:00.000Z";

/// Ein zweiter Benutzer für die Filter der Boards
const USER_B: &str = "22222222-2222-2222-2222-222222222222";

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
        created_at: NOW.into(),
        created_by_id: test_db::USER_ID.into(),
        created_by_name: "Test".into(),
        updated_at: NOW.into(),
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

// =============================================================================================
// Ohne Datenbank: Feature-Gate und SQL-Texte
// =============================================================================================

/// Unit of Work über dem Ersatz-Runner; `missing` nennt die fehlenden Erweiterungen
fn uow<'a>(runner: &'a mut FakeRunner, missing: &[&str]) -> MssqlUnitOfWork<'a> {
    MssqlUnitOfWork::new(
        runner,
        missing.iter().map(|name| (*name).to_string()).collect(),
        "AiHarness",
    )
}

#[tokio::test]
async fn liest_tickets_ohne_erweiterung_mit_null_statt_der_spalte_targetbranch() {
    let mut runner = FakeRunner::new();
    let tickets = TicketRepository::list(
        &mut uow(&mut runner, &["target-branch"]),
        &TicketFilter::default(),
    )
    .await
    .expect("list");

    assert_eq!(tickets, Vec::new());
    let text = &runner.texts[0];
    assert!(
        text.contains("CAST(NULL AS nvarchar(400)) AS TargetBranch"),
        "{text}"
    );
    assert!(!text.contains("t.TargetBranch"), "{text}");
}

#[tokio::test]
async fn schreibt_tickets_ohne_erweiterung_ohne_die_spalte_targetbranch() {
    let mut runner = FakeRunner::new();
    uow(&mut runner, &["target-branch"])
        .insert(&fx("ARC-0001", 1, 1, 1))
        .await
        .expect("insert");

    let text = runner.last().expect("Anweisung");
    assert!(!text.contains("TargetBranch"), "{text}");
}

#[tokio::test]
async fn ein_gesetzter_zielbranch_verlangt_ohne_erweiterung_das_skript() {
    let mut ticket = fx("ARC-0001", 1, 1, 1);
    ticket.target_branch = Some("release/1".into());

    for (fall, ticket) in [
        ("insert", ticket.clone()),
        ("update", {
            let mut spater = ticket.clone();
            spater.rev = 2;
            spater
        }),
    ] {
        let mut runner = FakeRunner::new();
        let mut unit = uow(&mut runner, &["target-branch"]);
        let fehler = if fall == "insert" {
            unit.insert(&ticket).await.expect_err(fall)
        } else {
            unit.update(&ticket).await.expect_err(fall)
        };
        assert_eq!(fehler.code(), Some("board-schema-outdated"), "{fall}");
        assert!(
            fehler
                .to_string()
                .contains("crates/nexus-db/sql/007_target_branch.sql"),
            "{fall}: {fehler}"
        );
    }
}

#[tokio::test]
async fn liest_die_spalte_targetbranch_sobald_die_erweiterung_da_ist() {
    let mut runner = FakeRunner::new();
    TicketRepository::list(&mut uow(&mut runner, &[]), &TicketFilter::default())
        .await
        .expect("list");

    assert!(
        runner.texts[0].contains("t.TargetBranch"),
        "{}",
        runner.texts[0]
    );
}

#[tokio::test]
async fn nur_for_update_setzt_den_sperr_hinweis_und_nie_in_der_unterabfrage() {
    // Eine Zeile, damit die Unterabfragen für Labels und Voraussetzungen überhaupt laufen
    let mut runner = FakeRunner::with_rows(vec![ticket_zeile()]);
    uow(&mut runner, &[])
        .by_id("ARC-0001", true)
        .await
        .expect("by_id");

    assert!(
        runner.texts[0].contains("WITH (UPDLOCK, ROWLOCK)"),
        "{}",
        runner.texts[0]
    );
    for text in &runner.texts[1..] {
        assert!(!text.contains("UPDLOCK"), "{text}");
    }

    let mut ohne = FakeRunner::new();
    uow(&mut ohne, &[])
        .by_id("ARC-0001", false)
        .await
        .expect("by_id");
    assert!(!ohne.texts[0].contains("UPDLOCK"), "{}", ohne.texts[0]);
}

#[tokio::test]
async fn eine_leere_id_liste_setzt_kein_sql_ab() {
    let mut runner = FakeRunner::new();
    let mut unit = uow(&mut runner, &[]);
    let leer_ids = TicketFilter {
        ids: Some(Vec::new()),
        ..TicketFilter::default()
    };
    let leere_boards = TicketFilter {
        board_ids: Some(Vec::new()),
        ..TicketFilter::default()
    };
    assert!(TicketRepository::list(&mut unit, &leer_ids)
        .await
        .expect("list")
        .is_empty());
    assert!(TicketRepository::list(&mut unit, &leere_boards)
        .await
        .expect("list")
        .is_empty());
    assert!(unit
        .open_count_by_board(&[])
        .await
        .expect("open_count")
        .is_empty());
    assert!(unit.progress(&[]).await.expect("progress").is_empty());
    assert!(unit.by_tickets(&[]).await.expect("by_tickets").is_empty());

    assert_eq!(runner.texts, Vec::<String>::new());
}

#[tokio::test]
async fn ohne_texte_liefert_die_langen_texte_nur_als_kennzeichen() {
    let mut runner = FakeRunner::new();
    TicketRepository::list(
        &mut uow(&mut runner, &[]),
        &TicketFilter {
            without_texts: true,
            ..TicketFilter::default()
        },
    )
    .await
    .expect("list");

    let text = &runner.texts[0];
    assert!(
        text.contains(
            "CASE WHEN LEN(LTRIM(t.Description)) > 0 THEN N'x' ELSE N'' END AS Description"
        ),
        "{text}"
    );
    assert!(
        text.contains("CASE WHEN LEN(LTRIM(t.[Plan])) > 0 THEN N'x' ELSE N'' END AS [Plan]"),
        "{text}"
    );
}

/// Eine gelesene Ticket-Zeile, wie sie `load` erwartet
fn ticket_zeile() -> nexus_db::mssql::MsRow {
    use nexus_db::mssql::MsCell;
    let text = |value: &str| MsCell::Text(value.to_string());
    nexus_db::mssql::MsRow::from_cells(
        [
            ("Id", text("ARC-0001")),
            ("ProjectId", MsCell::Int(1)),
            ("BoardId", MsCell::Int(1)),
            ("Number", MsCell::Int(1)),
            ("Title", text("Titel")),
            ("Description", text("")),
            ("Status", text("backlog")),
            ("Priority", text("normal")),
            ("SortOrder", MsCell::Int(0)),
            ("Assignee", MsCell::Null),
            ("NeedsPlanning", MsCell::Bool(true)),
            ("Acceptance", text("")),
            ("Plan", text("")),
            ("Handbook", text("")),
            ("Diagram", text("")),
            ("BlockedReason", MsCell::Null),
            ("PlannedAt", MsCell::Null),
            ("PlannedBy", MsCell::Null),
            ("PlanningClaimBy", MsCell::Null),
            ("PlanningClaimAt", MsCell::Null),
            ("CreatedAt", text(NOW)),
            ("CreatedById", text(test_db::USER_ID)),
            ("CreatedByName", text("Test")),
            ("UpdatedAt", text(NOW)),
            ("Rev", MsCell::Int(1)),
            ("Spec", MsCell::Null),
            ("SpecTask", MsCell::Null),
            ("Branch", MsCell::Null),
            ("Worktree", MsCell::Null),
            ("TargetBranch", MsCell::Null),
            ("Touches", text("[]")),
            ("Consumes", text("[]")),
            ("Produces", text("[]")),
            ("TicketId", text("ARC-0001")),
            ("Label", text("eilig")),
            ("DependsOnId", text("ARC-0002")),
        ]
        .into_iter()
        .map(|(name, cell)| (name.to_string(), cell)),
    )
}

// =============================================================================================
// Mit Datenbank: Rundreisen je Repository
// =============================================================================================

/// Verbindet mit der Test-Datenbank und leert sie; `None`, solange es keine gibt
async fn datenbank(test: &str) -> Option<MssqlBoardDatabase> {
    let config = test_db::config_or_skip(test)?;
    test_db::reset(&config).await;
    Some(
        MssqlBoardDatabase::connect(&config, ConnectOptions::default())
            .await
            .expect("Verbindung zur Test-Datenbank"),
    )
}

/// Benutzer, Projekt und Board, mit denen die Rundreisen arbeiten
async fn grundgeruest(db: &MssqlBoardDatabase) -> (i32, i32) {
    let user = BoardUser {
        id: test_db::USER_ID.into(),
        name: "Test".into(),
    };
    let project = NewProject {
        key: "ARC".into(),
        name: "Arches".into(),
        created_at: NOW.into(),
        created_by: test_db::USER_ID.into(),
    };
    tx!(db, [user, project], |u| {
        u.users().upsert(&user).await?;
        let project = u.projects().insert(&project).await?;
        let board = u
            .boards()
            .insert(&NewBoard {
                project_id: project.id,
                owner_id: test_db::USER_ID.into(),
                name: "Haupt".into(),
                repo_path: Some("/repo".into()),
                created_at: NOW.into(),
            })
            .await?;
        Ok((project.id, board.id))
    })
    .await
    .expect("Grundgerüst")
}

#[tokio::test]
async fn benutzer_werden_angelegt_aktualisiert_und_gelesen() {
    let Some(db) = datenbank("benutzer_werden_angelegt_aktualisiert_und_gelesen").await else {
        return;
    };
    for name in ["Anna", "Berta"] {
        let user = BoardUser {
            id: test_db::USER_ID.into(),
            name: name.into(),
        };
        tx!(db, [user], |u| { u.users().upsert(&user).await })
            .await
            .expect("upsert");
    }

    let found = tx!(db, |u| { u.users().by_id(test_db::USER_ID).await })
        .await
        .expect("by_id");
    assert_eq!(
        found,
        Some(BoardUser {
            id: test_db::USER_ID.into(),
            name: "Berta".into()
        })
    );
    let alle = tx!(db, |u| { u.users().list().await }).await.expect("list");
    assert_eq!(alle.len(), 1);
    db.close().await.unwrap();
}

#[tokio::test]
async fn projekte_vergeben_ticket_und_fragenummern_fortlaufend() {
    let Some(db) = datenbank("projekte_vergeben_ticket_und_fragenummern_fortlaufend").await else {
        return;
    };
    let (project_id, _) = grundgeruest(&db).await;

    let nummern = tx!(db, |u| {
        let erste = u.projects().take_ticket_number(project_id).await?;
        let zweite = u.projects().take_ticket_number(project_id).await?;
        let frage = u.projects().take_question_number(project_id).await?;
        Ok((erste, zweite, frage))
    })
    .await
    .expect("Nummern");
    assert_eq!(nummern, (1, 2, 1));

    let projekt = tx!(db, |u| { u.projects().by_key("ARC").await })
        .await
        .expect("by_key")
        .expect("Projekt");
    assert_eq!(projekt.next_ticket_number, 3);
    assert_eq!(projekt.next_question_number, 2);
    assert_eq!(projekt.created_at, NOW);
    assert_eq!(projekt.created_by, test_db::USER_ID);

    tx!(db, |u| { u.projects().rename(project_id, "Bögen").await })
        .await
        .expect("rename");
    let umbenannt = tx!(db, |u| { u.projects().by_id(project_id).await })
        .await
        .expect("by_id")
        .expect("Projekt");
    assert_eq!(umbenannt.name, "Bögen");
    db.close().await.unwrap();
}

#[tokio::test]
async fn ein_doppelter_projektschluessel_meldet_project_exists() {
    let Some(db) = datenbank("ein_doppelter_projektschluessel_meldet_project_exists").await else {
        return;
    };
    grundgeruest(&db).await;

    let fehler = tx!(db, |u| {
        u.projects()
            .insert(&NewProject {
                key: "ARC".into(),
                name: "Noch mal".into(),
                created_at: NOW.into(),
                created_by: test_db::USER_ID.into(),
            })
            .await
    })
    .await
    .expect_err("doppelter Schlüssel");
    assert_eq!(fehler.code(), Some("project-exists"));
    db.close().await.unwrap();
}

#[tokio::test]
async fn boards_werden_gefiltert_geaendert_und_geloescht() {
    let Some(db) = datenbank("boards_werden_gefiltert_geaendert_und_geloescht").await else {
        return;
    };
    let (project_id, board_id) = grundgeruest(&db).await;
    let zweiter = BoardUser {
        id: USER_B.into(),
        name: "Berta".into(),
    };
    let fremdes = tx!(db, [zweiter], |u| {
        u.users().upsert(&zweiter).await?;
        u.boards()
            .insert(&NewBoard {
                project_id,
                owner_id: USER_B.into(),
                name: "Fremd".into(),
                repo_path: None,
                created_at: NOW.into(),
            })
            .await
    })
    .await
    .expect("zweites Board");

    let eigene = tx!(db, |u| {
        u.boards()
            .list(&BoardFilter {
                project_id: Some(project_id),
                owner_id: Some(test_db::USER_ID.into()),
            })
            .await
    })
    .await
    .expect("list");
    assert_eq!(eigene.len(), 1);
    assert_eq!(eigene[0].id, board_id);
    assert_eq!(eigene[0].repo_path.as_deref(), Some("/repo"));
    assert_eq!(eigene[0].owner_id, test_db::USER_ID);

    let geaendert = Board {
        name: "Neu".into(),
        repo_path: None,
        ..eigene[0].clone()
    };
    tx!(db, [geaendert], |u| { u.boards().update(&geaendert).await })
        .await
        .expect("update");
    let gelesen = tx!(db, |u| { u.boards().by_id(board_id).await })
        .await
        .expect("by_id")
        .expect("Board");
    assert_eq!(gelesen.name, "Neu");
    assert_eq!(gelesen.repo_path, None);

    let weg = fremdes.id;
    tx!(db, |u| { u.boards().delete(weg).await })
        .await
        .expect("delete");
    let rest = tx!(db, |u| { u.boards().list(&BoardFilter::default()).await })
        .await
        .expect("list");
    assert_eq!(rest.len(), 1);
    db.close().await.unwrap();
}

#[tokio::test]
async fn tickets_behalten_labels_voraussetzungen_und_zielbranch() {
    let Some(db) = datenbank("tickets_behalten_labels_voraussetzungen_und_zielbranch").await else {
        return;
    };
    let (project_id, board_id) = grundgeruest(&db).await;
    let erstes = fx("ARC-0001", project_id, board_id, 1);
    let mut zweites = fx("ARC-0002", project_id, board_id, 2);
    zweites.labels = vec!["eilig".into(), "db".into()];
    zweites.depends_on = vec!["ARC-0001".into()];
    zweites.target_branch = Some("release/1".into());
    zweites.touches = vec!["a.rs".into()];
    zweites.status = TicketStatus::InProgress;
    zweites.sort_order = 5;

    tx!(db, [erstes, zweites], |u| {
        u.tickets().insert(&erstes).await?;
        u.tickets().insert(&zweites).await
    })
    .await
    .expect("insert");

    let gelesen = tx!(db, |u| { u.tickets().by_id("ARC-0002", true).await })
        .await
        .expect("by_id")
        .expect("Ticket");
    assert_eq!(gelesen.labels, ["eilig", "db"]);
    assert_eq!(gelesen.depends_on, ["ARC-0001"]);
    assert_eq!(gelesen.target_branch.as_deref(), Some("release/1"));
    assert_eq!(gelesen.touches, ["a.rs"]);
    assert_eq!(gelesen.created_at, NOW);

    // Sortiert nach Projekt und Nummer
    let alle = tx!(db, |u| {
        u.tickets()
            .list(&TicketFilter {
                board_ids: Some(vec![board_id]),
                ..TicketFilter::default()
            })
            .await
    })
    .await
    .expect("list");
    assert_eq!(
        alle.iter().map(|t| t.id.as_str()).collect::<Vec<_>>(),
        ["ARC-0001", "ARC-0002"]
    );

    let offen = tx!(db, |u| {
        u.tickets().open_count_by_board(&[board_id]).await
    })
    .await
    .expect("open_count");
    assert_eq!(offen.get(&board_id), Some(&2));
    let max = tx!(db, |u| {
        u.tickets()
            .max_sort_order(board_id, TicketStatus::InProgress)
            .await
    })
    .await
    .expect("max_sort_order");
    assert_eq!(max, Some(5));
    db.close().await.unwrap();
}

#[tokio::test]
async fn ohne_texte_bleibt_leer_leer_und_gefuelltes_ein_kennzeichen() {
    let Some(db) = datenbank("ohne_texte_bleibt_leer_leer_und_gefuelltes_ein_kennzeichen").await
    else {
        return;
    };
    let (project_id, board_id) = grundgeruest(&db).await;
    let mut ticket = fx("ARC-0001", project_id, board_id, 1);
    ticket.description = "Ein langer Text".into();

    tx!(db, [ticket], |u| { u.tickets().insert(&ticket).await })
        .await
        .expect("insert");

    let leicht = tx!(db, |u| {
        u.tickets()
            .list(&TicketFilter {
                without_texts: true,
                ..TicketFilter::default()
            })
            .await
    })
    .await
    .expect("list");
    assert_eq!(leicht[0].description, "x");
    assert_eq!(leicht[0].acceptance, "");
    db.close().await.unwrap();
}

#[tokio::test]
async fn ein_veralteter_stand_meldet_conflict() {
    let Some(db) = datenbank("ein_veralteter_stand_meldet_conflict").await else {
        return;
    };
    let (project_id, board_id) = grundgeruest(&db).await;
    let ticket = fx("ARC-0001", project_id, board_id, 1);
    tx!(db, [ticket], |u| { u.tickets().insert(&ticket).await })
        .await
        .expect("insert");

    let mut neu = ticket.clone();
    neu.rev = 2;
    neu.title = "Geändert".into();
    neu.labels = vec!["eilig".into()];
    tx!(db, [neu], |u| { u.tickets().update(&neu).await })
        .await
        .expect("update");

    let gelesen = tx!(db, |u| { u.tickets().by_id("ARC-0001", false).await })
        .await
        .expect("by_id")
        .expect("Ticket");
    assert_eq!(gelesen.title, "Geändert");
    assert_eq!(gelesen.labels, ["eilig"]);
    assert_eq!(gelesen.rev, 2);

    // Dieselbe Fassung noch einmal schreiben: Rev 1 steht nicht mehr in der DB
    let fehler = tx!(db, [neu], |u| { u.tickets().update(&neu).await })
        .await
        .expect_err("conflict");
    assert_eq!(fehler.code(), Some("conflict"));
    db.close().await.unwrap();
}

#[tokio::test]
async fn voraussetzungen_werden_gefunden_und_geloest() {
    let Some(db) = datenbank("voraussetzungen_werden_gefunden_und_geloest").await else {
        return;
    };
    let (project_id, board_id) = grundgeruest(&db).await;
    let erstes = fx("ARC-0001", project_id, board_id, 1);
    let mut zweites = fx("ARC-0002", project_id, board_id, 2);
    zweites.depends_on = vec!["ARC-0001".into()];
    let mut drittes = fx("ARC-0003", project_id, board_id, 3);
    drittes.depends_on = vec!["ARC-0001".into()];

    tx!(db, [erstes, zweites, drittes], |u| {
        u.tickets().insert(&erstes).await?;
        u.tickets().insert(&zweites).await?;
        u.tickets().insert(&drittes).await
    })
    .await
    .expect("insert");

    let abhaengig = tx!(db, |u| { u.tickets().dependents("ARC-0001").await })
        .await
        .expect("dependents");
    assert_eq!(abhaengig, ["ARC-0002", "ARC-0003"]);

    let geloest = tx!(db, |u| {
        u.tickets().remove_dependency_on("ARC-0001").await
    })
    .await
    .expect("remove_dependency_on");
    assert_eq!(geloest, ["ARC-0002", "ARC-0003"]);
    assert!(tx!(db, |u| { u.tickets().dependents("ARC-0001").await })
        .await
        .expect("dependents")
        .is_empty());

    tx!(db, |u| { u.tickets().delete("ARC-0001").await })
        .await
        .expect("delete");
    assert_eq!(
        tx!(db, |u| { u.tickets().by_id("ARC-0001", false).await })
            .await
            .expect("by_id"),
        None
    );
    db.close().await.unwrap();
}

#[tokio::test]
async fn schritte_werden_ersetzt_abgehakt_und_gezaehlt() {
    let Some(db) = datenbank("schritte_werden_ersetzt_abgehakt_und_gezaehlt").await else {
        return;
    };
    let (project_id, board_id) = grundgeruest(&db).await;
    let ticket = fx("ARC-0001", project_id, board_id, 1);
    tx!(db, [ticket], |u| { u.tickets().insert(&ticket).await })
        .await
        .expect("insert");

    let schritte = vec![
        step("Erst lesen", false),
        TicketStep {
            text: "Dann schreiben".into(),
            done: false,
            detail: Some("mit Test".into()),
        },
    ];
    tx!(db, [schritte], |u| {
        u.steps().replace("ARC-0001", &schritte).await
    })
    .await
    .expect("replace");

    let abgehakt = tx!(db, |u| { u.steps().set_done("ARC-0001", 0, true).await })
        .await
        .expect("set_done");
    assert!(abgehakt);
    assert!(
        !tx!(db, |u| { u.steps().set_done("ARC-0001", 9, true).await })
            .await
            .expect("set_done")
    );

    let gelesen = tx!(db, |u| { u.steps().list("ARC-0001").await })
        .await
        .expect("list");
    assert_eq!(gelesen.len(), 2);
    assert!(gelesen[0].done);
    assert_eq!(gelesen[1].detail.as_deref(), Some("mit Test"));

    let fortschritt = tx!(db, |u| {
        u.steps().progress(&["ARC-0001".to_string()]).await
    })
    .await
    .expect("progress");
    assert_eq!(
        fortschritt.get("ARC-0001"),
        Some(&StepProgress { done: 1, total: 2 })
    );

    // Ersetzen wirft die alten Schritte weg
    let eins = vec![step("Nur einer", false)];
    tx!(db, [eins], |u| {
        u.steps().replace("ARC-0001", &eins).await
    })
    .await
    .expect("replace");
    assert_eq!(
        tx!(db, |u| { u.steps().list("ARC-0001").await })
            .await
            .expect("list")
            .len(),
        1
    );
    db.close().await.unwrap();
}

#[tokio::test]
async fn eine_sitzung_wird_nur_einmal_verknuepft() {
    let Some(db) = datenbank("eine_sitzung_wird_nur_einmal_verknuepft").await else {
        return;
    };
    let (project_id, board_id) = grundgeruest(&db).await;
    let ticket = fx("ARC-0001", project_id, board_id, 1);
    tx!(db, [ticket], |u| { u.tickets().insert(&ticket).await })
        .await
        .expect("insert");

    let erste = tx!(db, |u| {
        u.ticket_sessions().add("ARC-0001", "sess-1", NOW).await
    })
    .await
    .expect("add");
    let zweite = tx!(db, |u| {
        u.ticket_sessions().add("ARC-0001", "sess-1", NOW).await
    })
    .await
    .expect("add");
    assert!(erste);
    assert!(!zweite);

    let je_ticket = tx!(db, |u| {
        u.ticket_sessions()
            .by_tickets(&["ARC-0001".to_string()])
            .await
    })
    .await
    .expect("by_tickets");
    assert_eq!(
        je_ticket.get("ARC-0001").map(Vec::as_slice),
        Some(["sess-1".to_string()].as_slice())
    );
    assert_eq!(
        tx!(db, |u| { u.ticket_sessions().tickets_of("sess-1").await })
            .await
            .expect("tickets_of"),
        ["ARC-0001"]
    );
    db.close().await.unwrap();
}
