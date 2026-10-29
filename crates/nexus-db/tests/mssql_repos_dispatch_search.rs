//! Die SQL-Server-Repositories für Dispatcher-Läufe, die Rohdaten der Auswertung und die
//! Suche. Portiert aus `packages/db/tests/MssqlFeatureGate.test.ts` (die Fälle der Läufe und
//! der Suche) und aus den Rundreisen von `Sqlite{DispatchRun,DispatchStats,Search}Repository`.
//!
//! Die Prüfungen am Anfang laufen ohne Datenbank gegen den [`FakeRunner`]. Die Rundreisen
//! brauchen eine Test-Datenbank aus `HARNESS_TEST_DB`; ohne die Variable melden sie
//! „übersprungen“ und kehren zurück:
//!
//! `HARNESS_TEST_DB='<json>' cargo test -p nexus-db --test mssql_repos_dispatch_search -- --test-threads=1`
//!
//! Seriell, weil alle Rundreisen dieselbe Datenbank leeren.

use nexus_core::board::config::BoardConfig;
use nexus_core::board::database::{
    BoardSearchRepository, DispatchRunRepository, DispatchStatsRepository,
};
use nexus_core::board::dispatch_run_types::{
    DispatchConfigVersion, DispatchProfileReason, DispatchRun, DispatchRunFilter, DispatchRunKind,
    DispatchRunOutcome, DispatchRunSource,
};
use nexus_core::board::search_types::{SearchField, SearchKind, SearchRequest, SEARCH_WINDOW};
use nexus_db::mssql::testing::FakeRunner;
use nexus_db::mssql::{MsValue, MssqlRunner, MssqlUnitOfWork, TiberiusRunner};
use nexus_db::SchemaVersionGuard;

mod common;

use common::test_db;

/// Die Datenbank, auf die sich die Meldungen des Gates beziehen
const DATABASE: &str = "AiHarness";

// ---------------------------------------------------------------------------------------------
// Ohne Datenbank: Zugriffe ohne eingespielte Erweiterung
// ---------------------------------------------------------------------------------------------

/// Ein Lauf mit allen Feldern gesetzt; `id` und `started_at` unterscheiden die Läufe
fn lauf(id: &str, started_at: &str) -> DispatchRun {
    DispatchRun {
        id: id.to_string(),
        kind: DispatchRunKind::Work,
        subject_id: "TST-0001".to_string(),
        project_id: 1,
        board_id: Some(1),
        agent: "junior".to_string(),
        attempt: Some(2),
        profile: "standard".to_string(),
        escalated_from: Some("klein".to_string()),
        requested_model: Some("opus".to_string()),
        model: Some("claude-opus-5".to_string()),
        effort: Some("low".to_string()),
        started_at: started_at.to_string(),
        ended_at: Some("2026-02-01T10:05:00.000Z".to_string()),
        duration_ms: Some(300_000),
        num_turns: Some(42),
        exit_code: Some(0),
        timed_out: false,
        outcome: DispatchRunOutcome::Done,
        abort_category: Some("none".to_string()),
        abort_reason: Some("Größe überschritten – ß".to_string()),
        cost_usd: Some(1.234567),
        input_tokens: Some(9_007_199_254_740_993),
        output_tokens: Some(2),
        cache_read_tokens: Some(3),
        cache_write_tokens: Some(4),
        model_usage: Some("{\"claude-opus-5\":1}".to_string()),
        source: DispatchRunSource::Dispatcher,
        max_turns: Some(250),
        timeout_minutes: Some(90),
        profile_reason: Some(DispatchProfileReason::Label),
        rule_index: Some(3),
        config_hash: Some("abc123".to_string()),
        denied_count: Some(1),
        denied_tools: Some("Bash".to_string()),
    }
}

/// Anfrage an die Suche
fn anfrage(terms: &[&str], kinds: &[SearchKind], per_source: i32) -> SearchRequest {
    SearchRequest {
        terms: terms.iter().map(|term| (*term).to_string()).collect(),
        kinds: kinds.to_vec(),
        per_source,
    }
}

#[tokio::test]
async fn verweigert_die_laufe_des_dispatchers_mit_hinweis_auf_das_skript() {
    let mut runner = FakeRunner::new();
    let mut uow = MssqlUnitOfWork::new(&mut runner, vec!["dispatch-runs".to_string()], DATABASE);
    let filter = DispatchRunFilter {
        project_id: 1,
        board_id: None,
        from: "2026-01-01T00:00:00.000Z".to_string(),
        to: "2026-02-01T00:00:00.000Z".to_string(),
    };
    let fehler = [
        uow.list(&filter).await.err(),
        uow.first_started(1, DispatchRunSource::Dispatcher)
            .await
            .err(),
        uow.existing(&["x".to_string()]).await.err(),
        uow.insert(&lauf("r1", "2026-01-01T10:00:00.000Z"))
            .await
            .err(),
        uow.list_all("2026-01-01T00:00:00.000Z", "2026-02-01T00:00:00.000Z")
            .await
            .err(),
    ];
    for fehler in fehler {
        let fehler = fehler.expect("der Zugriff muss sich verweigern");
        assert_eq!(fehler.code(), Some("board-schema-outdated"));
        assert!(
            fehler
                .to_string()
                .contains("crates/nexus-db/sql/004_dispatch_runs.sql"),
            "{fehler}"
        );
        // Die Meldung nennt die Datenbank und verspricht, dass alles andere weiterläuft
        assert!(fehler.to_string().contains(DATABASE), "{fehler}");
        assert!(fehler.to_string().contains("laufen weiter"), "{fehler}");
    }
    assert!(runner.texts.is_empty(), "ohne Erweiterung kein SQL");
}

#[tokio::test]
async fn verweigert_die_laufe_auch_wenn_nur_die_lauf_details_fehlen() {
    // Die Abfragen lesen die Spalten aus 006 mit; ohne die Erweiterung schlüge der Zugriff
    // sonst als SQL-Fehler fehl
    let mut runner = FakeRunner::new();
    let mut uow = MssqlUnitOfWork::new(
        &mut runner,
        vec!["dispatch-run-details".to_string()],
        DATABASE,
    );
    let fehler = uow
        .existing(&["x".to_string()])
        .await
        .expect_err("der Zugriff muss sich verweigern");
    assert_eq!(fehler.code(), Some("board-schema-outdated"));
    assert!(
        fehler
            .to_string()
            .contains("crates/nexus-db/sql/006_dispatch_run_details.sql"),
        "{fehler}"
    );
}

#[tokio::test]
async fn fragt_die_laufe_mit_eingespielter_erweiterung_ganz_normal_ab() {
    let mut runner = FakeRunner::new();
    let mut uow = MssqlUnitOfWork::new(&mut runner, Vec::new(), DATABASE);
    assert_eq!(
        uow.existing(&["x".to_string()]).await.unwrap(),
        Vec::<String>::new()
    );
    assert!(runner.texts[0].contains("board.DispatchRuns"));
    assert!(runner.texts[0].contains("Id IN (@P1)"));
}

#[tokio::test]
async fn lasst_notizen_aus_der_suche_weg_statt_die_ganze_suche_scheitern_zu_lassen() {
    let mut runner = FakeRunner::new();
    let mut uow = MssqlUnitOfWork::new(&mut runner, vec!["note-structure".to_string()], DATABASE);
    let ergebnis = uow
        .find(&anfrage(&["x"], &[SearchKind::Ticket, SearchKind::Note], 5))
        .await
        .unwrap();
    assert!(!runner.texts[0].contains("board.Notes"));
    assert!(runner.texts[0].contains("board.Tickets"));
    assert!(ergebnis.rows.is_empty());
    assert!(!ergebnis.truncated);
}

#[tokio::test]
async fn sucht_gar_nicht_wenn_nur_notizen_gefragt_sind_und_die_erweiterung_fehlt() {
    let mut runner = FakeRunner::new();
    let mut uow = MssqlUnitOfWork::new(&mut runner, vec!["note-structure".to_string()], DATABASE);
    let ergebnis = uow
        .find(&anfrage(&["x"], &[SearchKind::Note], 5))
        .await
        .unwrap();
    assert!(ergebnis.rows.is_empty());
    assert!(!ergebnis.truncated);
    assert!(runner.texts.is_empty());
}

#[tokio::test]
async fn durchsucht_mit_eingespielter_erweiterung_auch_die_notizen() {
    let mut runner = FakeRunner::new();
    let mut uow = MssqlUnitOfWork::new(&mut runner, Vec::new(), DATABASE);
    uow.find(&anfrage(&["x"], &[SearchKind::Ticket, SearchKind::Note], 5))
        .await
        .unwrap();
    let sql = &runner.texts[0];
    assert!(sql.contains("board.Notes"));
    // Gebunden wird in der Reihenfolge des Textes: die Teilabfragen mit @take und @t0 stehen
    // vor dem äußeren SELECT mit @win
    assert_eq!(runner.bound[0], ["take", "t0", "win"]);
    assert!(sql.contains("COLLATE Latin1_General_100_CI_AS"));
    assert!(sql.contains("UNION ALL"));
}

#[tokio::test]
async fn sucht_ohne_begriffe_gar_nicht() {
    let mut runner = FakeRunner::new();
    let mut uow = MssqlUnitOfWork::new(&mut runner, Vec::new(), DATABASE);
    let ergebnis = uow
        .find(&anfrage(&[], &[SearchKind::Ticket], 5))
        .await
        .unwrap();
    assert!(ergebnis.rows.is_empty());
    assert!(runner.texts.is_empty());
}

// ---------------------------------------------------------------------------------------------
// Mit Test-Datenbank: Rundreisen
// ---------------------------------------------------------------------------------------------

/// IDs des angelegten Projekts und Boards
struct Stamm {
    project_id: i32,
    board_id: i32,
    zweites_board: i32,
}

/// Leert die Datenbank, prüft die Erweiterungen und legt Benutzer, Projekt und zwei Boards an;
/// `None`, solange es keine Test-Datenbank gibt oder ihr Erweiterungen fehlen
async fn vorbereitet(test: &str) -> Option<BoardConfig> {
    let config = test_db::config_or_skip(test)?;
    test_db::reset(&config).await;
    let mut client = test_db::client(&config).await;
    let mut runner = TiberiusRunner::new(&mut client);
    let features: Vec<String> = runner
        .query_rows(
            "IF OBJECT_ID(N'board.SchemaFeatures') IS NULL SELECT CAST(NULL AS varchar(50)) AS Name WHERE 1 = 0 ELSE SELECT Name FROM board.SchemaFeatures;",
            &[],
        )
        .await
        .expect("die eingespielten Erweiterungen")
        .iter()
        .map(|row| row.get_str("Name").expect("Name"))
        .collect();
    let fehlend = SchemaVersionGuard::missing(&features);
    if !fehlend.is_empty() {
        println!("{test} übersprungen: der Test-Datenbank fehlen {fehlend:?}");
        return None;
    }
    // board.DispatchConfigVersions bleibt beim Leeren stehen (keine Fremdschlüssel)
    runner
        .execute("DELETE FROM board.DispatchConfigVersions;", &[])
        .await
        .expect("Konfig-Versionen leeren");
    Some(config)
}

/// Legt Benutzer, Projekt und zwei Boards an und liefert ihre IDs
async fn stamm(sql: &mut dyn MssqlRunner) -> Stamm {
    let rows = sql
        .query_rows(
            "INSERT INTO board.Users (Id, Name, CreatedAt) VALUES (@user, N'Test', SYSUTCDATETIME());
             INSERT INTO board.Projects ([Key], Name, CreatedAt, CreatedBy) VALUES (N'TST', N'Test', SYSUTCDATETIME(), @user);
             DECLARE @p int = SCOPE_IDENTITY();
             INSERT INTO board.Boards (ProjectId, OwnerId, Name, CreatedAt) VALUES (@p, @user, N'Board A', SYSUTCDATETIME());
             DECLARE @b1 int = SCOPE_IDENTITY();
             INSERT INTO board.Boards (ProjectId, OwnerId, Name, CreatedAt) VALUES (@p, @user, N'Board B', SYSUTCDATETIME());
             SELECT @p AS ProjectId, @b1 AS BoardA, SCOPE_IDENTITY() AS BoardB;",
            &[("user", MsValue::text(test_db::USER_ID))],
        )
        .await
        .expect("Stammdaten anlegen");
    let row = &rows[0];
    Stamm {
        project_id: row.get_i32("ProjectId").unwrap(),
        board_id: row.get_i32("BoardA").unwrap(),
        zweites_board: row.get_i32("BoardB").unwrap(),
    }
}

/// Legt ein Ticket an; `number` muss je Projekt eindeutig sein
async fn ticket(sql: &mut dyn MssqlRunner, stamm: &Stamm, id: &str, number: i32, title: &str) {
    sql.execute(
        "INSERT INTO board.Tickets (Id, ProjectId, BoardId, Number, Title, Description, Status, Priority, SortOrder, NeedsPlanning,
           Acceptance, [Plan], Handbook, Diagram, CreatedAt, CreatedById, CreatedByName, UpdatedAt, Rev)
         VALUES (@id, @projectId, @boardId, @number, @title, @description, 'backlog', 'normal', 0, 0,
           N'', N'', N'', N'', SYSUTCDATETIME(), @user, N'Test', SYSUTCDATETIME(), 1)",
        &[
            ("id", MsValue::text(id)),
            ("projectId", MsValue::Int(stamm.project_id)),
            ("boardId", MsValue::Int(stamm.board_id)),
            ("number", MsValue::Int(number)),
            ("title", MsValue::text(title)),
            ("description", MsValue::text(format!("Beschreibung zu {title}"))),
            ("user", MsValue::text(test_db::USER_ID)),
        ],
    )
    .await
    .expect("Ticket anlegen");
}

#[tokio::test]
async fn fugt_einen_lauf_ein_und_liest_ihn_unverandert_zuruck() {
    let Some(config) = vorbereitet("fugt_einen_lauf_ein_und_liest_ihn_unverandert_zuruck").await
    else {
        return;
    };
    let mut client = test_db::client(&config).await;
    let mut runner = TiberiusRunner::new(&mut client);
    let mut uow = MssqlUnitOfWork::new(&mut runner, Vec::new(), config.database.clone());
    let stamm = stamm(uow.sql()).await;

    let mut run = lauf("r1", "2026-02-01T10:00:00.000Z");
    run.project_id = stamm.project_id;
    run.board_id = Some(stamm.board_id);
    assert!(uow.insert(&run).await.unwrap());
    // Dieselbe ID ein zweites Mal ändert nichts
    assert!(!uow.insert(&run).await.unwrap());

    let filter = DispatchRunFilter {
        project_id: stamm.project_id,
        board_id: None,
        from: "2026-02-01T00:00:00.000Z".to_string(),
        to: "2026-03-01T00:00:00.000Z".to_string(),
    };
    let gelesen = uow.list(&filter).await.unwrap();
    assert_eq!(gelesen.len(), 1);
    let gelesen = &gelesen[0];
    // Die Kosten stehen als decimal(12,6) in der DB; der Vergleich lässt die Rundung zu
    assert!((gelesen.cost_usd.unwrap() - 1.234567).abs() < 1e-9);
    assert_eq!(gelesen.input_tokens, Some(9_007_199_254_740_993));
    assert_eq!(
        gelesen.abort_reason.as_deref(),
        Some("Größe überschritten – ß")
    );
    assert_eq!(gelesen.profile_reason, Some(DispatchProfileReason::Label));
    assert!(!gelesen.timed_out);
    assert_eq!(
        DispatchRun {
            cost_usd: run.cost_usd,
            ..gelesen.clone()
        },
        run
    );

    assert_eq!(
        uow.existing(&["r1".to_string(), "fehlt".to_string()])
            .await
            .unwrap(),
        ["r1"]
    );
    assert_eq!(
        uow.first_started(stamm.project_id, DispatchRunSource::Dispatcher)
            .await
            .unwrap()
            .as_deref(),
        Some("2026-02-01T10:00:00.000Z")
    );
    assert_eq!(
        uow.first_started(stamm.project_id, DispatchRunSource::Comments)
            .await
            .unwrap(),
        None
    );
}

#[tokio::test]
async fn filtert_laufe_nach_board_und_zeitraum() {
    let Some(config) = vorbereitet("filtert_laufe_nach_board_und_zeitraum").await else {
        return;
    };
    let mut client = test_db::client(&config).await;
    let mut runner = TiberiusRunner::new(&mut client);
    let mut uow = MssqlUnitOfWork::new(&mut runner, Vec::new(), config.database.clone());
    let stamm = stamm(uow.sql()).await;

    for (id, start, board) in [
        ("a", "2026-02-01T10:00:00.000Z", stamm.board_id),
        ("b", "2026-02-02T10:00:00.000Z", stamm.zweites_board),
        ("c", "2026-03-01T10:00:00.000Z", stamm.board_id),
    ] {
        let mut run = lauf(id, start);
        run.project_id = stamm.project_id;
        run.board_id = Some(board);
        assert!(uow.insert(&run).await.unwrap());
    }

    let filter = |board_id| DispatchRunFilter {
        project_id: stamm.project_id,
        board_id,
        from: "2026-02-01T00:00:00.000Z".to_string(),
        to: "2026-03-01T00:00:00.000Z".to_string(),
    };
    // Ohne Board alle im Zeitraum, nach Start und ID sortiert; `to` zählt nicht mit
    let ids: Vec<String> = uow
        .list(&filter(None))
        .await
        .unwrap()
        .into_iter()
        .map(|run| run.id)
        .collect();
    assert_eq!(ids, ["a", "b"]);
    let ids: Vec<String> = uow
        .list(&filter(Some(stamm.zweites_board)))
        .await
        .unwrap()
        .into_iter()
        .map(|run| run.id)
        .collect();
    assert_eq!(ids, ["b"]);
    // list_all geht über alle Projekte
    let ids: Vec<String> = uow
        .list_all("2026-02-01T00:00:00.000Z", "2026-04-01T00:00:00.000Z")
        .await
        .unwrap()
        .into_iter()
        .map(|run| run.id)
        .collect();
    assert_eq!(ids, ["a", "b", "c"]);
}

#[tokio::test]
async fn merkt_jede_konfig_version_einmal_und_liest_sie_sortiert() {
    let Some(config) = vorbereitet("merkt_jede_konfig_version_einmal_und_liest_sie_sortiert").await
    else {
        return;
    };
    let mut client = test_db::client(&config).await;
    let mut runner = TiberiusRunner::new(&mut client);
    let mut uow = MssqlUnitOfWork::new(&mut runner, Vec::new(), config.database.clone());

    let version = |hash: &str, at: &str| DispatchConfigVersion {
        hash: hash.to_string(),
        dispatcher: "dispatcher-1".to_string(),
        first_seen_at: at.to_string(),
        content: "{\"profile\":\"standard\"}".to_string(),
    };
    assert!(uow
        .record_config_version(&version("h2", "2026-02-02T00:00:00.000Z"))
        .await
        .unwrap());
    assert!(uow
        .record_config_version(&version("h1", "2026-02-01T00:00:00.000Z"))
        .await
        .unwrap());
    // Derselbe Hash ein zweites Mal ändert nichts
    assert!(!uow
        .record_config_version(&version("h1", "2026-02-05T00:00:00.000Z"))
        .await
        .unwrap());

    let gelesen = uow
        .list_config_versions("2026-02-01T00:00:00.000Z", "2026-03-01T00:00:00.000Z")
        .await
        .unwrap();
    assert_eq!(
        gelesen,
        [
            version("h1", "2026-02-01T00:00:00.000Z"),
            version("h2", "2026-02-02T00:00:00.000Z"),
        ]
    );
    // Außerhalb des Zeitraums kommt nichts
    assert!(uow
        .list_config_versions("2026-01-01T00:00:00.000Z", "2026-02-01T00:00:00.000Z")
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn liefert_die_rohdaten_der_dispatch_statistik() {
    let Some(config) = vorbereitet("liefert_die_rohdaten_der_dispatch_statistik").await else {
        return;
    };
    let mut client = test_db::client(&config).await;
    let mut runner = TiberiusRunner::new(&mut client);
    let mut uow = MssqlUnitOfWork::new(&mut runner, Vec::new(), config.database.clone());
    let stamm = stamm(uow.sql()).await;
    ticket(uow.sql(), &stamm, "TST-0001", 1, "Erstes Ticket").await;
    ticket(uow.sql(), &stamm, "TST-0002", 2, "Zweites Ticket").await;

    for (ticket_id, at, field, to) in [
        (
            "TST-0001",
            "2026-02-01T10:00:00.000Z",
            "status",
            "in_progress",
        ),
        ("TST-0001", "2026-02-02T10:00:00.000Z", "status", "done"),
        ("TST-0002", "2026-01-01T10:00:00.000Z", "status", "done"),
        (
            "TST-0001",
            "2026-02-03T10:00:00.000Z",
            "blockedReason",
            "wartet",
        ),
        ("TST-0001", "2026-02-04T10:00:00.000Z", "title", "egal"),
    ] {
        uow.sql()
            .execute(
                "INSERT INTO board.History (TicketId, At, ById, ByName, Field, FromValue, ToValue, Note)
                 VALUES (@ticketId, @at, @user, N'Test', @field, NULL, @to, NULL)",
                &[
                    ("ticketId", MsValue::text(ticket_id)),
                    ("at", MsValue::date(at).unwrap()),
                    ("user", MsValue::text(test_db::USER_ID)),
                    ("field", MsValue::text(field)),
                    ("to", MsValue::text(to)),
                ],
            )
            .await
            .expect("Verlauf anlegen");
    }
    for (ticket_id, body) in [
        ("TST-0001", "[dispatch] Start: an junior übergeben"),
        ("TST-0001", "[dispatch] Abbruch: Schrittlimit"),
        ("TST-0001", "[dispatch] Abbruch: Zeitlimit"),
        ("TST-0001", "Ein ganz normaler Kommentar"),
        ("TST-0002", "[dispatch] Pause: Nutzungslimit"),
    ] {
        uow.sql()
            .execute(
                "INSERT INTO board.Comments (TicketId, AuthorId, AuthorName, Body, CreatedAt)
                 VALUES (@ticketId, @user, N'Test', @body, SYSUTCDATETIME())",
                &[
                    ("ticketId", MsValue::text(ticket_id)),
                    ("user", MsValue::text(test_db::USER_ID)),
                    ("body", MsValue::text(body)),
                ],
            )
            .await
            .expect("Kommentar anlegen");
    }

    // Nur Tickets, deren letzter Wechsel auf „done“ im Zeitraum liegt
    let filter = DispatchRunFilter {
        project_id: stamm.project_id,
        board_id: Some(stamm.board_id),
        from: "2026-02-01T00:00:00.000Z".to_string(),
        to: "2026-03-01T00:00:00.000Z".to_string(),
    };
    assert_eq!(uow.done_ticket_ids(&filter).await.unwrap(), ["TST-0001"]);
    let anderes_board = DispatchRunFilter {
        board_id: Some(stamm.zweites_board),
        ..filter.clone()
    };
    assert!(uow
        .done_ticket_ids(&anderes_board)
        .await
        .unwrap()
        .is_empty());

    // Nur status und blockedReason, in der Reihenfolge der Einträge
    let verlauf = uow
        .status_history(&["TST-0001".to_string(), "TST-0002".to_string()])
        .await
        .unwrap();
    let felder: Vec<(&str, &str)> = verlauf
        .iter()
        .map(|entry| (entry.ticket_id.as_str(), entry.field.as_str()))
        .collect();
    assert_eq!(
        felder,
        [
            ("TST-0001", "status"),
            ("TST-0001", "status"),
            ("TST-0002", "status"),
            ("TST-0001", "blockedReason"),
        ]
    );
    assert_eq!(verlauf[0].by_id, test_db::USER_ID);
    assert_eq!(verlauf[1].at, "2026-02-02T10:00:00.000Z");

    let zahlen = uow
        .dispatch_comment_counts(&["TST-0001".to_string(), "TST-0002".to_string()])
        .await
        .unwrap();
    assert_eq!(zahlen["TST-0001"].starts, 1);
    assert_eq!(zahlen["TST-0001"].aborts, 2);
    assert!(!zahlen.contains_key("TST-0002"));

    // Alle [dispatch]-Kommentare des Projekts, nach ID sortiert
    let kommentare = uow.dispatch_comments(stamm.project_id).await.unwrap();
    let texte: Vec<&str> = kommentare.iter().map(|c| c.body.as_str()).collect();
    assert_eq!(
        texte,
        [
            "[dispatch] Start: an junior übergeben",
            "[dispatch] Abbruch: Schrittlimit",
            "[dispatch] Abbruch: Zeitlimit",
            "[dispatch] Pause: Nutzungslimit",
        ]
    );
    assert_eq!(kommentare[0].author_id, test_db::USER_ID);
}

#[tokio::test]
async fn durchsucht_umlaute_und_nimmt_prozent_und_unterstrich_wortlich() {
    let Some(config) =
        vorbereitet("durchsucht_umlaute_und_nimmt_prozent_und_unterstrich_wortlich").await
    else {
        return;
    };
    let mut client = test_db::client(&config).await;
    let mut runner = TiberiusRunner::new(&mut client);
    let mut uow = MssqlUnitOfWork::new(&mut runner, Vec::new(), config.database.clone());
    let stamm = stamm(uow.sql()).await;
    ticket(uow.sql(), &stamm, "TST-0001", 1, "Ärger mit der Größe").await;
    ticket(uow.sql(), &stamm, "TST-0002", 2, "100% erledigt").await;
    ticket(uow.sql(), &stamm, "TST-0003", 3, "a_b getrennt").await;

    // Groß/Klein egal (Collation), Umlaute zählen
    let treffer = uow
        .find(&anfrage(&["ärger"], &[SearchKind::Ticket], 10))
        .await
        .unwrap();
    let ids: Vec<&str> = treffer.rows.iter().map(|row| row.id.as_str()).collect();
    assert_eq!(ids, ["TST-0001"]);
    assert_eq!(treffer.rows[0].field, SearchField::Title);
    assert_eq!(treffer.rows[0].kind, SearchKind::Ticket);
    assert_eq!(treffer.rows[0].board_id, Some(stamm.board_id));
    assert_eq!(treffer.rows[0].text, "Ärger mit der Größe");
    assert_eq!(treffer.rows[0].text_start, 0);
    assert_eq!(treffer.rows[0].text_length, 19);
    assert!(!treffer.truncated);

    // `%` und `_` sind keine Platzhalter: „100%“ findet nur das eine Ticket
    let treffer = uow
        .find(&anfrage(&["100%"], &[SearchKind::Ticket], 10))
        .await
        .unwrap();
    let ids: Vec<&str> = treffer
        .rows
        .iter()
        .filter(|row| row.field == SearchField::Title)
        .map(|row| row.id.as_str())
        .collect();
    assert_eq!(ids, ["TST-0002"]);
    let treffer = uow
        .find(&anfrage(&["a_b"], &[SearchKind::Ticket], 10))
        .await
        .unwrap();
    let ids: Vec<&str> = treffer
        .rows
        .iter()
        .filter(|row| row.field == SearchField::Title)
        .map(|row| row.id.as_str())
        .collect();
    assert_eq!(ids, ["TST-0003"]);
    // Ein Platzhalter-Muster findet nichts, weil CHARINDEX wörtlich sucht
    assert!(uow
        .find(&anfrage(&["100%erledigt"], &[SearchKind::Ticket], 10))
        .await
        .unwrap()
        .rows
        .is_empty());

    // Alle Begriffe müssen im selben Feld stehen
    assert!(uow
        .find(&anfrage(&["Ärger", "erledigt"], &[SearchKind::Ticket], 10))
        .await
        .unwrap()
        .rows
        .is_empty());
}

#[tokio::test]
async fn liefert_ein_fenster_um_eine_fundstelle_hinter_8000_zeichen() {
    let Some(config) =
        vorbereitet("liefert_ein_fenster_um_eine_fundstelle_hinter_8000_zeichen").await
    else {
        return;
    };
    let mut client = test_db::client(&config).await;
    let mut runner = TiberiusRunner::new(&mut client);
    let mut uow = MssqlUnitOfWork::new(&mut runner, Vec::new(), config.database.clone());
    let stamm = stamm(uow.sql()).await;
    ticket(uow.sql(), &stamm, "TST-0001", 1, "Langer Text").await;

    // Die Fundstelle steht hinter 8000 Zeichen – nvarchar(max), nicht nvarchar(4000)
    let vorlauf = "x".repeat(9000);
    let text = format!("{vorlauf}Nadel{}", "y".repeat(100));
    uow.sql()
        .execute(
            "UPDATE board.Tickets SET Description = @text WHERE Id = N'TST-0001'",
            &[("text", MsValue::text(text.clone()))],
        )
        .await
        .expect("Beschreibung setzen");

    let treffer = uow
        .find(&anfrage(&["Nadel"], &[SearchKind::Ticket], 10))
        .await
        .unwrap();
    let zeile = treffer
        .rows
        .iter()
        .find(|row| row.field == SearchField::Description)
        .expect("ein Treffer in der Beschreibung");
    // Das Fenster beginnt SEARCH_WINDOW Zeichen vor der Fundstelle
    assert_eq!(zeile.text_start, (9000 - SEARCH_WINDOW) as i32);
    assert_eq!(zeile.text_length, text.chars().count() as i32);
    assert!(zeile.text.contains("Nadel"));
    assert_eq!(zeile.text.len(), SEARCH_WINDOW * 2 + "Nadel".len());
}

#[tokio::test]
async fn kurzt_je_quelle_auf_die_hochstzahl_und_meldet_das() {
    let Some(config) = vorbereitet("kurzt_je_quelle_auf_die_hochstzahl_und_meldet_das").await
    else {
        return;
    };
    let mut client = test_db::client(&config).await;
    let mut runner = TiberiusRunner::new(&mut client);
    let mut uow = MssqlUnitOfWork::new(&mut runner, Vec::new(), config.database.clone());
    let stamm = stamm(uow.sql()).await;
    for number in 1..=4 {
        ticket(
            uow.sql(),
            &stamm,
            &format!("TST-000{number}"),
            number,
            "Nadel im Titel",
        )
        .await;
    }

    let treffer = uow
        .find(&anfrage(&["Nadel"], &[SearchKind::Ticket], 2))
        .await
        .unwrap();
    let titel: Vec<&str> = treffer
        .rows
        .iter()
        .filter(|row| row.field == SearchField::Title)
        .map(|row| row.id.as_str())
        .collect();
    assert_eq!(titel.len(), 2);
    assert!(treffer.truncated);

    // Reicht die Höchstzahl, meldet die Suche keine Kürzung
    let treffer = uow
        .find(&anfrage(&["Nadel"], &[SearchKind::Ticket], 10))
        .await
        .unwrap();
    assert_eq!(
        treffer
            .rows
            .iter()
            .filter(|row| row.field == SearchField::Title)
            .count(),
        4
    );
    assert!(!treffer.truncated);
}
