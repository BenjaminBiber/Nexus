//! Schema-Runner und die Struktur der Stufen 001–008; portiert aus
//! `packages/db/tests/sqlite/SqliteSchemaRunner.test.ts`,
//! `SqliteSchema001/002/003/004/006/007.test.ts` und `SqliteRowVer.test.ts`.

mod common;

use common::schema_inspect::{ColumnInfo, ForeignKey, SchemaInspect, NOW, USER};

use nexus_core::board::BoardProvider;
use nexus_db::sqlite::{SqliteSchemaRunner, SqliteSchemaStep, SQLITE_SCHEMA_STEPS};
use nexus_db::{SchemaVersionGuard, EXPECTED_SCHEMA_VERSION};
use rusqlite::Connection;
use tempfile::TempDir;

// ---------------------------------------------------------------------------------------------
// SqliteSchemaRunner
// ---------------------------------------------------------------------------------------------

const JETZT: &str = "(strftime('%Y-%m-%dT%H:%M:%fZ','now'))";

/// Erste Stufe: legt SchemaVersion und Tabelle A an
const V1: SqliteSchemaStep = SqliteSchemaStep {
    id: "t001",
    version: Some(1),
    feature: None,
    sql: "CREATE TABLE SchemaVersion (Version INTEGER NOT NULL PRIMARY KEY, AppliedAt TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))); CREATE TABLE A (X INTEGER);",
};

/// Zweite Stufe: legt Tabelle B an
const V2: SqliteSchemaStep = SqliteSchemaStep {
    id: "t002",
    version: Some(2),
    feature: None,
    sql: "CREATE TABLE B (X INTEGER);",
};

/// Zweite Stufe, deren zweite Anweisung scheitert, weil es A schon gibt
const KAPUTT: SqliteSchemaStep = SqliteSchemaStep {
    id: "t002",
    version: Some(2),
    feature: None,
    sql: "CREATE TABLE B (X INTEGER); CREATE TABLE A (Y INTEGER);",
};

/// Stufe mit Erweiterung statt Version
const FEATURE: SqliteSchemaStep = SqliteSchemaStep {
    id: "t003",
    version: None,
    feature: Some("f-test"),
    sql: "CREATE TABLE SchemaFeatures (Name TEXT NOT NULL PRIMARY KEY, Script TEXT NOT NULL, AppliedAt TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')));",
};

/// Temp-Datei, die zwischen zwei Verbindungen bestehen bleibt
struct TempDatei {
    dir: TempDir,
    conn: Connection,
}

impl TempDatei {
    fn neu() -> Self {
        let dir = TempDir::with_prefix("harness-sqlite-").expect("Temp-Ordner");
        let conn = Connection::open(dir.path().join("board.db")).expect("Datei");
        Self { dir, conn }
    }

    /// Schließt die Verbindung und öffnet dieselbe Datei erneut
    fn erneut_offnen(&mut self) {
        let conn = std::mem::replace(
            &mut self.conn,
            Connection::open_in_memory().expect("Zwischenstand"),
        );
        drop(conn.close());
        self.conn = Connection::open(self.dir.path().join("board.db")).expect("Datei");
    }

    fn hat_tabelle(&self, name: &str) -> bool {
        self.conn
            .prepare("SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1")
            .expect("prepare")
            .exists([name])
            .expect("exists")
    }

    fn zahl(&self, sql: &str) -> i64 {
        self.conn.query_row(sql, [], |row| row.get(0)).expect(sql)
    }

    fn stufen(&self) -> Vec<String> {
        let mut statement = self
            .conn
            .prepare("SELECT Id FROM SchemaSteps ORDER BY Id")
            .expect("prepare");
        statement
            .query_map([], |row| row.get::<_, String>(0))
            .expect("query")
            .collect::<rusqlite::Result<Vec<String>>>()
            .expect("Zeilen")
    }
}

#[test]
fn eine_leere_datei_hat_keine_version_und_keine_erweiterungen() {
    let datei = TempDatei::neu();
    let runner = SqliteSchemaRunner::with_steps(&datei.conn, &[]);
    assert_eq!(runner.schema_version().unwrap(), None);
    assert!(runner.schema_features().unwrap().is_empty());
}

#[test]
fn eine_frische_datei_erreicht_die_letzte_version() {
    let datei = TempDatei::neu();
    let stufen = [V1, V2];
    let runner = SqliteSchemaRunner::with_steps(&datei.conn, &stufen);
    assert_eq!(runner.run().unwrap(), ["t001", "t002"]);
    assert_eq!(runner.schema_version().unwrap(), Some(2));
    assert_eq!(datei.stufen(), ["t001", "t002"]);
    assert!(datei.hat_tabelle("A"));
    assert!(datei.hat_tabelle("B"));
}

#[test]
fn das_zweite_offnen_fuhrt_nichts_erneut_aus() {
    let mut datei = TempDatei::neu();
    let stufen = [V1, V2];
    SqliteSchemaRunner::with_steps(&datei.conn, &stufen)
        .run()
        .unwrap();
    datei.erneut_offnen();

    let runner = SqliteSchemaRunner::with_steps(&datei.conn, &stufen);
    assert!(runner.run().unwrap().is_empty());
    assert_eq!(datei.zahl("SELECT COUNT(*) FROM SchemaVersion"), 2);
    assert_eq!(runner.schema_version().unwrap(), Some(2));
}

#[test]
fn eine_neue_stufe_hinten_wird_nachgeholt() {
    let datei = TempDatei::neu();
    let nur_eine = [V1];
    assert_eq!(
        SqliteSchemaRunner::with_steps(&datei.conn, &nur_eine)
            .run()
            .unwrap(),
        ["t001"]
    );
    let beide = [V1, V2];
    let runner = SqliteSchemaRunner::with_steps(&datei.conn, &beide);
    assert_eq!(runner.run().unwrap(), ["t002"]);
    assert_eq!(runner.schema_version().unwrap(), Some(2));
}

#[test]
fn ein_fehler_rollt_nur_diese_stufe_zuruck() {
    let datei = TempDatei::neu();
    let mit_fehler = [V1, KAPUTT];
    let fehler = SqliteSchemaRunner::with_steps(&datei.conn, &mit_fehler)
        .run()
        .expect_err("Stufe t002 scheitert");
    assert_eq!(fehler.code(), Some("board-schema-failed"));
    assert!(fehler.to_string().contains("t002"), "{fehler}");

    assert!(!datei.hat_tabelle("B"));
    assert!(datei.hat_tabelle("A"));
    assert_eq!(datei.stufen(), ["t001"]);

    // Die heile Fassung derselben Stufe lässt sich danach einspielen
    let heil = [V1, V2];
    let runner = SqliteSchemaRunner::with_steps(&datei.conn, &heil);
    assert_eq!(runner.schema_version().unwrap(), Some(1));
    assert_eq!(runner.run().unwrap(), ["t002"]);
    assert_eq!(runner.schema_version().unwrap(), Some(2));
}

#[test]
fn eine_erweiterung_erhoht_die_version_nicht() {
    let datei = TempDatei::neu();
    let stufen = [V1, FEATURE];
    let runner = SqliteSchemaRunner::with_steps(&datei.conn, &stufen);
    runner.run().unwrap();
    assert_eq!(runner.schema_features().unwrap(), ["f-test"]);
    assert_eq!(
        datei
            .conn
            .query_row("SELECT Script FROM SchemaFeatures", [], |row| row
                .get::<_, String>(0))
            .unwrap(),
        "t003"
    );
    assert_eq!(runner.schema_version().unwrap(), Some(1));
    // Der Ausdruck für „jetzt“ ist in allen Stufen derselbe
    assert!(FEATURE.sql.contains(JETZT));
}

#[test]
fn die_standardliste_lauft_ohne_fehler() {
    let datei = TempDatei::neu();
    let runner = SqliteSchemaRunner::new(&datei.conn);
    assert_eq!(runner.run().unwrap().len(), SQLITE_SCHEMA_STEPS.len());
    assert_eq!(runner.schema_version().unwrap(), Some(3));
}

// ---------------------------------------------------------------------------------------------
// Stufe 001_initial
// ---------------------------------------------------------------------------------------------

/// Die ersten `anzahl` Stufen der Standardliste
fn bis(anzahl: usize) -> &'static [SqliteSchemaStep] {
    &SQLITE_SCHEMA_STEPS[..anzahl]
}

/// Prüft, dass der Zeitpunkt wie `strftime('%Y-%m-%dT%H:%M:%fZ','now')` aussieht
fn ist_iso(value: &str) -> bool {
    value.len() == 24
        && value.ends_with('Z')
        && value.as_bytes()[10] == b'T'
        && value.as_bytes()[19] == b'.'
}

#[test]
fn stufe_001_legt_alle_tabellen_an_und_meldet_version_1() {
    let s = SchemaInspect::fresh(bis(1));
    assert_eq!(
        s.tables(),
        [
            "Boards",
            "Comments",
            "Deletions",
            "History",
            "Notes",
            "Projects",
            "SchemaVersion",
            "TicketDependencies",
            "TicketLabels",
            "Tickets",
            "Users",
        ]
    );
    assert_eq!(s.runner().schema_version().unwrap(), Some(1));
    s.dispose();
}

#[test]
fn stufe_001_ubernimmt_die_spaltennamen_von_sql_server() {
    let s = SchemaInspect::fresh(bis(1));
    assert_eq!(s.columns("SchemaVersion"), ["Version", "AppliedAt"]);
    assert_eq!(s.columns("Users"), ["Id", "Name", "CreatedAt"]);
    assert_eq!(
        s.columns("Projects"),
        ["Id", "Key", "Name", "NextTicketNumber", "CreatedAt", "CreatedBy"]
    );
    assert_eq!(
        s.columns("Boards"),
        ["Id", "ProjectId", "OwnerId", "Name", "RepoPath", "CreatedAt", "RowVer"]
    );
    assert_eq!(
        s.columns("Tickets"),
        [
            "Id", "ProjectId", "BoardId", "Number", "Title", "Description", "Status", "Priority",
            "SortOrder", "Assignee", "NeedsPlanning", "Acceptance", "Plan", "Handbook", "Diagram",
            "BlockedReason", "PlannedAt", "PlannedBy", "PlanningClaimBy", "PlanningClaimAt",
            "CreatedAt", "CreatedById", "CreatedByName", "UpdatedAt", "Rev", "RowVer",
        ]
    );
    assert_eq!(s.columns("TicketLabels"), ["TicketId", "Position", "Label"]);
    assert_eq!(
        s.columns("TicketDependencies"),
        ["TicketId", "Position", "DependsOnId"]
    );
    assert_eq!(
        s.columns("Comments"),
        ["Id", "TicketId", "AuthorId", "AuthorName", "Body", "CreatedAt", "RowVer"]
    );
    assert_eq!(
        s.columns("History"),
        ["Id", "TicketId", "At", "ById", "ByName", "Field", "FromValue", "ToValue", "Note"]
    );
    assert_eq!(
        s.columns("Notes"),
        [
            "Id", "BoardId", "AuthorId", "AuthorName", "Body", "Source", "Labels", "State",
            "TicketId", "Reason", "CreatedAt", "RowVer",
        ]
    );
    assert_eq!(
        s.columns("Deletions"),
        ["Id", "At", "ById", "ByName", "TicketId", "TicketTitle", "FromStatus"]
    );
    assert_eq!(
        s.column("Tickets", "RowVer"),
        Some(ColumnInfo {
            typ: "INTEGER".into(),
            notnull: 1,
            dflt_value: Some("0".into()),
        })
    );
    s.dispose();
}

#[test]
fn stufe_001_legt_indizes_und_schlussel_an() {
    let s = SchemaInspect::fresh(bis(1));
    assert_eq!(
        s.indexes("Boards").get("IX_Boards_Project"),
        Some(&vec!["ProjectId".to_string()])
    );
    assert_eq!(s.indexes("Boards").len(), 1);
    assert_eq!(
        s.indexes("Tickets").get("IX_Tickets_Board"),
        Some(&vec!["BoardId".to_string(), "Status".to_string()])
    );
    assert_eq!(
        s.indexes("TicketDependencies").get("IX_TicketDeps_DependsOn"),
        Some(&vec!["DependsOnId".to_string()])
    );
    assert_eq!(
        s.indexes("Comments").get("IX_Comments_Ticket"),
        Some(&vec!["TicketId".to_string()])
    );
    assert_eq!(
        s.indexes("History").get("IX_History_Ticket"),
        Some(&vec!["TicketId".to_string()])
    );
    assert_eq!(
        s.indexes("Notes").get("IX_Notes_Board"),
        Some(&vec!["BoardId".to_string(), "State".to_string()])
    );
    assert_eq!(s.uniques("Projects"), [["Key"]]);
    assert_eq!(s.uniques("Tickets"), [["ProjectId", "Number"]]);
    assert_eq!(s.primary_key("Tickets"), ["Id"]);
    assert_eq!(s.primary_key("TicketLabels"), ["TicketId", "Label"]);
    assert_eq!(
        s.primary_key("TicketDependencies"),
        ["TicketId", "DependsOnId"]
    );
    s.dispose();
}

#[test]
fn stufe_001_ubernimmt_fremdschlussel_und_kaskaden() {
    let s = SchemaInspect::fresh(bis(1));
    assert_eq!(
        s.foreign_keys("Tickets"),
        [
            ForeignKey { from: "BoardId".into(), table: "Boards".into(), to: "Id".into(), on_delete: "NO ACTION".into() },
            ForeignKey { from: "CreatedById".into(), table: "Users".into(), to: "Id".into(), on_delete: "NO ACTION".into() },
            ForeignKey { from: "ProjectId".into(), table: "Projects".into(), to: "Id".into(), on_delete: "NO ACTION".into() },
        ]
    );
    assert_eq!(
        s.foreign_keys("TicketDependencies"),
        [
            ForeignKey { from: "DependsOnId".into(), table: "Tickets".into(), to: "Id".into(), on_delete: "NO ACTION".into() },
            ForeignKey { from: "TicketId".into(), table: "Tickets".into(), to: "Id".into(), on_delete: "CASCADE".into() },
        ]
    );
    assert_eq!(
        s.foreign_keys("TicketLabels"),
        [ForeignKey { from: "TicketId".into(), table: "Tickets".into(), to: "Id".into(), on_delete: "CASCADE".into() }]
    );
    assert_eq!(
        s.foreign_keys_short("Comments"),
        ["AuthorId:NO ACTION", "TicketId:CASCADE"]
    );
    assert_eq!(
        s.foreign_keys_short("History"),
        ["ById:NO ACTION", "TicketId:CASCADE"]
    );
    assert_eq!(
        s.foreign_keys_short("Notes"),
        ["AuthorId:NO ACTION", "BoardId:CASCADE"]
    );
    assert_eq!(
        s.foreign_keys("Deletions"),
        [ForeignKey { from: "ById".into(), table: "Users".into(), to: "Id".into(), on_delete: "NO ACTION".into() }]
    );
    assert_eq!(s.foreign_key_violations(), 0);
    s.dispose();
}

#[test]
fn stufe_001_setzt_die_defaults_wie_sql_server() {
    let s = SchemaInspect::fresh(bis(1));
    s.seed_board();
    s.insert_ticket("AIH-0001", 1).expect("Ticket");
    assert!(ist_iso(&s.text("SELECT CreatedAt FROM Users").unwrap()));
    assert!(ist_iso(&s.text("SELECT AppliedAt FROM SchemaVersion").unwrap()));
    assert_eq!(s.number("SELECT NextTicketNumber FROM Projects"), Some(1));
    let texte: Vec<(String, String, String, String, String, i64)> = s.rows(
        "SELECT Description, Acceptance, \"Plan\", Handbook, Diagram, RowVer FROM Tickets",
        &[],
        |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
            ))
        },
    );
    assert_eq!(
        texte,
        [(
            String::new(),
            String::new(),
            String::new(),
            String::new(),
            String::new(),
            0
        )]
    );
    s.dispose();
}

#[test]
fn stufe_001_lehnt_check_fremdschlussel_und_doppelte_ab() {
    let s = SchemaInspect::fresh(bis(1));
    s.seed_board();
    let fehler = s
        .insert_ticket_with("AIH-0001", 1, "foo", "normal", 1)
        .expect_err("Status");
    assert!(
        fehler.to_string().contains("CK_Tickets_Status"),
        "{fehler}"
    );
    let fehler = s
        .insert_ticket_with("AIH-0001", 1, "backlog", "mega", 1)
        .expect_err("Priorität");
    assert!(
        fehler.to_string().contains("CK_Tickets_Priority"),
        "{fehler}"
    );
    let fehler = s
        .run_with(
            "INSERT INTO Notes (BoardId, AuthorId, AuthorName, Body, State, CreatedAt) VALUES (1, ?1, 'T', 'b', 'weg', ?2)",
            &[&USER, &NOW],
        )
        .expect_err("Notiz-Status");
    assert!(fehler.to_string().contains("CK_Notes_State"), "{fehler}");

    let fehler = s
        .insert_ticket_with("AIH-0001", 1, "backlog", "normal", 99)
        .expect_err("fremdes Board");
    assert!(
        fehler.to_string().contains("FOREIGN KEY constraint failed"),
        "{fehler}"
    );

    s.insert_ticket("AIH-0001", 1).expect("Ticket");
    let fehler = s.insert_ticket("AIH-0002", 1).expect_err("Nummer doppelt");
    assert!(
        fehler
            .to_string()
            .contains("UNIQUE constraint failed: Tickets.ProjectId, Tickets.Number"),
        "{fehler}"
    );
    s.dispose();
}

#[test]
fn stufe_001_loscht_labels_kommentare_und_verlauf_mit_dem_ticket() {
    let s = SchemaInspect::fresh(bis(1));
    s.seed_board();
    s.insert_ticket("AIH-0001", 1).expect("Ticket");
    s.insert_ticket("AIH-0002", 2).expect("Ticket");
    s.run("INSERT INTO TicketLabels (TicketId, Position, Label) VALUES ('AIH-0001', 0, 'db')")
        .expect("Label");
    s.run("INSERT INTO TicketDependencies (TicketId, Position, DependsOnId) VALUES ('AIH-0002', 0, 'AIH-0001')")
        .expect("Voraussetzung");
    s.run_with(
        "INSERT INTO Comments (TicketId, AuthorId, AuthorName, Body, CreatedAt) VALUES ('AIH-0001', ?1, 'T', 'Text', ?2)",
        &[&USER, &NOW],
    )
    .expect("Kommentar");
    s.run_with(
        "INSERT INTO History (TicketId, At, ById, ByName, Field) VALUES ('AIH-0001', ?2, ?1, 'T', 'status')",
        &[&USER, &NOW],
    )
    .expect("Verlauf");

    // Solange ein anderes Ticket es voraussetzt, bleibt es stehen
    let fehler = s
        .run("DELETE FROM Tickets WHERE Id = 'AIH-0001'")
        .expect_err("Voraussetzung blockiert");
    assert!(
        fehler.to_string().contains("FOREIGN KEY constraint failed"),
        "{fehler}"
    );
    s.run("DELETE FROM TicketDependencies WHERE DependsOnId = 'AIH-0001'")
        .expect("Verweis weg");
    s.run("DELETE FROM Tickets WHERE Id = 'AIH-0001'")
        .expect("Ticket weg");
    for table in ["TicketLabels", "Comments", "History"] {
        assert_eq!(s.count(table), 0, "{table}");
    }
    s.dispose();
}

#[test]
fn stufe_001_vergibt_board_ids_nach_dem_loschen_nicht_neu() {
    let s = SchemaInspect::fresh(bis(1));
    s.seed_board();
    s.run("DELETE FROM Boards WHERE Id = 1").expect("Board weg");
    s.run_with(
        "INSERT INTO Boards (ProjectId, OwnerId, Name, CreatedAt) VALUES (1, ?1, 'Neu', ?2)",
        &[&USER, &NOW],
    )
    .expect("Board");
    assert_eq!(s.number("SELECT Id FROM Boards"), Some(2));
    s.dispose();
}

// ---------------------------------------------------------------------------------------------
// Stufe 002_questions
// ---------------------------------------------------------------------------------------------

/// Frage in Projekt 1 mit allen Pflichtfeldern
fn frage(
    s: &SchemaInspect,
    id: &str,
    number: i32,
    state: &str,
    origin_kind: &str,
    outcome: Option<&str>,
) -> rusqlite::Result<usize> {
    s.run_with(
        "INSERT INTO Questions (Id, ProjectId, Number, Title, State, OriginKind, OriginName, DecisionOutcome, CreatedAt, CreatedById, UpdatedAt, Rev)
         VALUES (?1, 1, ?2, 'Frage', ?3, ?4, 'Test', ?5, ?6, ?7, ?6, 1)",
        &[&id, &number, &state, &origin_kind, &outcome, &NOW, &USER],
    )
}

#[test]
fn stufe_002_erganzt_den_fragenzahler_und_meldet_version_2() {
    let s = SchemaInspect::fresh(bis(2));
    s.seed_board();
    assert_eq!(
        s.columns("Projects").last().map(String::as_str),
        Some("NextQuestionNumber")
    );
    assert_eq!(s.number("SELECT NextQuestionNumber FROM Projects"), Some(1));
    assert_eq!(s.runner().schema_version().unwrap(), Some(2));
    s.dispose();
}

#[test]
fn stufe_002_legt_alle_fragentabellen_an() {
    let s = SchemaInspect::fresh(bis(2));
    assert_eq!(
        s.columns("Questions"),
        [
            "Id", "ProjectId", "Number", "Title", "Description", "State", "Occasion", "OriginKind",
            "OriginName", "OriginSession", "DecisionOutcome", "DecisionOption", "DecisionText",
            "DecisionReason", "DecidedById", "DecidedByName", "DecidedAt", "CreatedAt",
            "CreatedById", "UpdatedAt", "Rev", "RowVer",
        ]
    );
    assert_eq!(s.columns("QuestionTags"), ["QuestionId", "Position", "Tag"]);
    assert_eq!(s.columns("QuestionTickets"), ["QuestionId", "TicketId"]);
    assert_eq!(
        s.columns("QuestionDependencies"),
        ["QuestionId", "Position", "DependsOnId"]
    );
    assert_eq!(
        s.columns("QuestionResearch"),
        ["Id", "QuestionId", "At", "ById", "ByName", "Kind", "Context"]
    );
    assert_eq!(
        s.columns("QuestionOptions"),
        ["QuestionId", "ResearchId", "OptionId", "Title", "Text", "Pros", "Cons", "Recommended"]
    );
    assert_eq!(
        s.columns("QuestionComments"),
        ["Id", "QuestionId", "AuthorId", "AuthorName", "AuthorKind", "Body", "CreatedAt"]
    );
    assert_eq!(
        s.columns("QuestionHistory"),
        ["Id", "QuestionId", "At", "ById", "ByName", "ByKind", "Type", "Text"]
    );
    s.dispose();
}

#[test]
fn stufe_002_legt_indizes_und_schlussel_an() {
    let s = SchemaInspect::fresh(bis(2));
    let erwartet = [
        ("Questions", "IX_Questions_Project", vec!["ProjectId", "State"]),
        ("QuestionTickets", "IX_QuestionTickets_Ticket", vec!["TicketId"]),
        ("QuestionDependencies", "IX_QuestionDeps_DependsOn", vec!["DependsOnId"]),
        ("QuestionResearch", "IX_QuestionResearch_Question", vec!["QuestionId"]),
        ("QuestionOptions", "IX_QuestionOptions_Research", vec!["ResearchId"]),
        ("QuestionComments", "IX_QuestionComments_Question", vec!["QuestionId"]),
        ("QuestionHistory", "IX_QuestionHistory_Question", vec!["QuestionId"]),
    ];
    for (table, index, columns) in erwartet {
        let indizes = s.indexes(table);
        assert_eq!(indizes.len(), 1, "{table}");
        assert_eq!(
            indizes.get(index),
            Some(&columns.iter().map(|c| c.to_string()).collect::<Vec<_>>()),
            "{table}"
        );
    }
    assert_eq!(s.uniques("Questions"), [["ProjectId", "Number"]]);
    assert_eq!(s.primary_key("QuestionTags"), ["QuestionId", "Tag"]);
    assert_eq!(s.primary_key("QuestionTickets"), ["QuestionId", "TicketId"]);
    assert_eq!(
        s.primary_key("QuestionDependencies"),
        ["QuestionId", "DependsOnId"]
    );
    assert_eq!(s.primary_key("QuestionOptions"), ["QuestionId", "OptionId"]);
    s.dispose();
}

#[test]
fn stufe_002_ubernimmt_fremdschlussel_und_kaskaden() {
    let s = SchemaInspect::fresh(bis(2));
    assert_eq!(
        s.foreign_keys("Questions"),
        [
            ForeignKey { from: "CreatedById".into(), table: "Users".into(), to: "Id".into(), on_delete: "NO ACTION".into() },
            ForeignKey { from: "DecidedById".into(), table: "Users".into(), to: "Id".into(), on_delete: "NO ACTION".into() },
            ForeignKey { from: "ProjectId".into(), table: "Projects".into(), to: "Id".into(), on_delete: "NO ACTION".into() },
        ]
    );
    assert_eq!(
        s.foreign_keys("QuestionTickets"),
        [
            ForeignKey { from: "QuestionId".into(), table: "Questions".into(), to: "Id".into(), on_delete: "CASCADE".into() },
            ForeignKey { from: "TicketId".into(), table: "Tickets".into(), to: "Id".into(), on_delete: "NO ACTION".into() },
        ]
    );
    assert_eq!(
        s.foreign_keys("QuestionDependencies"),
        [ForeignKey { from: "QuestionId".into(), table: "Questions".into(), to: "Id".into(), on_delete: "CASCADE".into() }]
    );
    assert_eq!(
        s.foreign_keys("QuestionOptions"),
        [ForeignKey { from: "ResearchId".into(), table: "QuestionResearch".into(), to: "Id".into(), on_delete: "CASCADE".into() }]
    );
    for table in [
        "QuestionTags",
        "QuestionResearch",
        "QuestionComments",
        "QuestionHistory",
    ] {
        let auf_frage = s
            .foreign_keys(table)
            .into_iter()
            .find(|f| f.from == "QuestionId");
        assert_eq!(
            auf_frage,
            Some(ForeignKey { from: "QuestionId".into(), table: "Questions".into(), to: "Id".into(), on_delete: "CASCADE".into() }),
            "{table}"
        );
    }
    assert_eq!(s.foreign_key_violations(), 0);
    s.dispose();
}

#[test]
fn stufe_002_lasst_nur_gultige_werte_in_den_check_spalten_zu() {
    let s = SchemaInspect::fresh(bis(2));
    s.seed_board();
    for (zustand, art, ergebnis, regel) in [
        ("vielleicht", "human", None, "CK_Questions_State"),
        ("open", "robot", None, "CK_Questions_OriginKind"),
        ("open", "human", Some("maybe"), "CK_Questions_Outcome"),
    ] {
        let fehler = frage(&s, "AIH-F001", 1, zustand, art, ergebnis).expect_err(regel);
        assert!(fehler.to_string().contains(regel), "{fehler}");
    }
    frage(&s, "AIH-F001", 1, "answered", "human", Some("answered")).expect("Frage");
    frage(&s, "AIH-F002", 2, "open", "human", None).expect("Frage");

    let fehler = s
        .run_with(
            "INSERT INTO QuestionResearch (QuestionId, At, ById, ByName, Kind) VALUES ('AIH-F001', ?2, ?1, 'T', 'x')",
            &[&USER, &NOW],
        )
        .expect_err("Recherche-Art");
    assert!(
        fehler.to_string().contains("CK_QuestionResearch_Kind"),
        "{fehler}"
    );
    let fehler = s
        .run_with(
            "INSERT INTO QuestionComments (QuestionId, AuthorId, AuthorName, AuthorKind, Body, CreatedAt) VALUES ('AIH-F001', ?1, 'T', 'x', 'b', ?2)",
            &[&USER, &NOW],
        )
        .expect_err("Kommentar-Art");
    assert!(
        fehler.to_string().contains("CK_QuestionComments_Kind"),
        "{fehler}"
    );
    let fehler = s
        .run_with(
            "INSERT INTO QuestionHistory (QuestionId, At, ById, ByName, ByKind, Type, Text) VALUES ('AIH-F001', ?2, ?1, 'T', 'x', 'created', 't')",
            &[&USER, &NOW],
        )
        .expect_err("Verlauf-Art");
    assert!(
        fehler.to_string().contains("CK_QuestionHistory_Kind"),
        "{fehler}"
    );
    // NULL ist bei ByKind erlaubt
    s.run_with(
        "INSERT INTO QuestionHistory (QuestionId, At, ById, ByName, ByKind, Type, Text) VALUES ('AIH-F001', ?2, ?1, 'T', NULL, 'created', 't')",
        &[&USER, &NOW],
    )
    .expect("Verlauf ohne Art");

    let fehler = frage(&s, "AIH-F003", 2, "open", "human", None).expect_err("Nummer doppelt");
    assert!(
        fehler
            .to_string()
            .contains("UNIQUE constraint failed: Questions.ProjectId, Questions.Number"),
        "{fehler}"
    );
    s.dispose();
}

#[test]
fn stufe_002_loscht_tags_recherchen_kommentare_und_verlauf_mit_der_frage() {
    let s = SchemaInspect::fresh(bis(2));
    s.seed_board();
    frage(&s, "AIH-F001", 1, "open", "human", None).expect("Frage");
    s.insert_ticket("AIH-0001", 1).expect("Ticket");
    s.run("INSERT INTO QuestionTags (QuestionId, Position, Tag) VALUES ('AIH-F001', 0, 'db')")
        .expect("Tag");
    s.run("INSERT INTO QuestionTickets (QuestionId, TicketId) VALUES ('AIH-F001', 'AIH-0001')")
        .expect("Ticket-Bezug");
    s.run("INSERT INTO QuestionDependencies (QuestionId, Position, DependsOnId) VALUES ('AIH-F001', 0, 'AIH-0001')")
        .expect("Abhängigkeit");
    s.run_with(
        "INSERT INTO QuestionResearch (QuestionId, At, ById, ByName, Kind) VALUES ('AIH-F001', ?2, ?1, 'T', 'session')",
        &[&USER, &NOW],
    )
    .expect("Recherche");
    s.run("INSERT INTO QuestionOptions (QuestionId, ResearchId, OptionId, Title, Text, Recommended) VALUES ('AIH-F001', 1, 'O1', 'Option', 'Text', 1)")
        .expect("Option");
    s.run_with(
        "INSERT INTO QuestionComments (QuestionId, AuthorId, AuthorName, AuthorKind, Body, CreatedAt) VALUES ('AIH-F001', ?1, 'T', 'human', 'b', ?2)",
        &[&USER, &NOW],
    )
    .expect("Kommentar");
    s.run_with(
        "INSERT INTO QuestionHistory (QuestionId, At, ById, ByName, Type, Text) VALUES ('AIH-F001', ?2, ?1, 'T', 'created', 't')",
        &[&USER, &NOW],
    )
    .expect("Verlauf");

    let vor_nach: Vec<(String, String)> = s.rows(
        "SELECT Pros, Cons FROM QuestionOptions",
        &[],
        |row| Ok((row.get(0)?, row.get(1)?)),
    );
    assert_eq!(vor_nach, [("[]".to_string(), "[]".to_string())]);

    // Der Bezug aufs Ticket blockiert dessen Löschen
    let fehler = s
        .run("DELETE FROM Tickets WHERE Id = 'AIH-0001'")
        .expect_err("Bezug blockiert");
    assert!(
        fehler.to_string().contains("FOREIGN KEY constraint failed"),
        "{fehler}"
    );

    s.run("DELETE FROM Questions WHERE Id = 'AIH-F001'")
        .expect("Frage weg");
    for table in [
        "QuestionTags",
        "QuestionTickets",
        "QuestionDependencies",
        "QuestionResearch",
        "QuestionOptions",
        "QuestionComments",
        "QuestionHistory",
    ] {
        assert_eq!(s.count(table), 0, "{table}");
    }
    s.dispose();
}

// ---------------------------------------------------------------------------------------------
// Stufe 003_harness
// ---------------------------------------------------------------------------------------------

#[test]
fn stufe_003_meldet_die_erwartete_schema_version() {
    let s = SchemaInspect::fresh(bis(3));
    assert_eq!(s.runner().schema_version().unwrap(), Some(3));
    assert_eq!(
        s.runner().schema_version().unwrap(),
        Some(EXPECTED_SCHEMA_VERSION)
    );
    let versionen: Vec<i64> = s.rows("SELECT Version FROM SchemaVersion ORDER BY Version", &[], |row| row.get(0));
    assert_eq!(versionen, [1, 2, 3]);
    s.dispose();
}

#[test]
fn stufe_003_hangt_die_harness_spalten_an() {
    let s = SchemaInspect::fresh(bis(3));
    s.seed_board();
    let spalten = s.columns("Tickets");
    assert_eq!(
        spalten[spalten.len() - 7..],
        ["Spec", "SpecTask", "Branch", "Worktree", "Touches", "Consumes", "Produces"]
    );
    assert_eq!(spalten.len(), 33);
    for name in ["Touches", "Consumes", "Produces"] {
        assert_eq!(
            s.column("Tickets", name),
            Some(ColumnInfo {
                typ: "TEXT".into(),
                notnull: 1,
                dflt_value: Some("'[]'".into()),
            }),
            "{name}"
        );
    }
    assert_eq!(
        s.column("Tickets", "Spec"),
        Some(ColumnInfo {
            typ: "TEXT".into(),
            notnull: 0,
            dflt_value: None,
        })
    );
    assert_eq!(
        s.columns("Comments").last().map(String::as_str),
        Some("ReadByAgentAt")
    );

    s.insert_ticket("AIH-0001", 1).expect("Ticket");
    let werte: Vec<(Option<String>, Option<String>, String)> = s.rows(
        "SELECT Spec, Branch, Touches FROM Tickets",
        &[],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    );
    assert_eq!(werte, [(None, None, "[]".to_string())]);
    s.dispose();
}

#[test]
fn stufe_003_legt_schritte_und_sessions_mit_kaskade_an() {
    let s = SchemaInspect::fresh(bis(3));
    s.seed_board();
    assert_eq!(
        s.columns("TicketSteps"),
        ["TicketId", "Position", "Text", "Done", "Detail"]
    );
    assert_eq!(s.primary_key("TicketSteps"), ["TicketId", "Position"]);
    assert_eq!(
        s.columns("TicketSessions"),
        ["TicketId", "SessionId", "AddedAt"]
    );
    assert_eq!(s.primary_key("TicketSessions"), ["TicketId", "SessionId"]);
    assert_eq!(
        s.indexes("TicketSessions").get("IX_TicketSessions_Session"),
        Some(&vec!["SessionId".to_string()])
    );
    assert_eq!(
        s.foreign_keys("TicketSteps"),
        [ForeignKey { from: "TicketId".into(), table: "Tickets".into(), to: "Id".into(), on_delete: "CASCADE".into() }]
    );
    assert_eq!(
        s.foreign_keys("TicketSessions"),
        [ForeignKey { from: "TicketId".into(), table: "Tickets".into(), to: "Id".into(), on_delete: "CASCADE".into() }]
    );

    s.insert_ticket("AIH-0001", 1).expect("Ticket");
    s.run("INSERT INTO TicketSteps (TicketId, Position, Text) VALUES ('AIH-0001', 0, 'Schritt')")
        .expect("Schritt");
    s.run_with(
        "INSERT INTO TicketSessions (TicketId, SessionId, AddedAt) VALUES ('AIH-0001', 'sess-1', ?1)",
        &[&NOW],
    )
    .expect("Session");
    let schritt: Vec<(i64, Option<String>)> =
        s.rows("SELECT Done, Detail FROM TicketSteps", &[], |row| {
            Ok((row.get(0)?, row.get(1)?))
        });
    assert_eq!(schritt, [(0, None)]);

    s.run("DELETE FROM Tickets WHERE Id = 'AIH-0001'")
        .expect("Ticket weg");
    assert_eq!(s.count("TicketSteps"), 0);
    assert_eq!(s.count("TicketSessions"), 0);
    assert_eq!(s.foreign_key_violations(), 0);
    s.dispose();
}

// ---------------------------------------------------------------------------------------------
// Stufe 004_dispatch_runs
// ---------------------------------------------------------------------------------------------

/// Lauf-Zeile mit Pflichtfeldern
fn lauf(s: &SchemaInspect, id: &str, kind: &str, source: &str) -> rusqlite::Result<usize> {
    s.run_with(
        "INSERT INTO DispatchRuns (Id, Kind, SubjectId, ProjectId, Agent, Profile, StartedAt, Outcome, Source, CostUsd, InputTokens)
         VALUES (?1, ?2, 'AIH-0001', 1, 'claude', 'standard', ?4, 'done', ?3, 0.123456, 1234567890123)",
        &[&id, &kind, &source, &NOW],
    )
}

#[test]
fn stufe_004_legt_dispatchruns_und_schemafeatures_an() {
    let s = SchemaInspect::fresh(bis(4));
    assert_eq!(s.columns("SchemaFeatures"), ["Name", "Script", "AppliedAt"]);
    assert_eq!(
        s.columns("DispatchRuns"),
        [
            "Id", "Kind", "SubjectId", "ProjectId", "BoardId", "Agent", "Attempt", "Profile",
            "EscalatedFrom", "RequestedModel", "Model", "Effort", "StartedAt", "EndedAt",
            "DurationMs", "NumTurns", "ExitCode", "TimedOut", "Outcome", "AbortCategory",
            "AbortReason", "CostUsd", "InputTokens", "OutputTokens", "CacheReadTokens",
            "CacheWriteTokens", "ModelUsage", "Source", "RecordedAt",
        ]
    );
    assert_eq!(
        s.column("DispatchRuns", "CostUsd").map(|c| c.typ),
        Some("REAL".to_string())
    );
    let indizes = s.indexes("DispatchRuns");
    assert_eq!(indizes.len(), 2);
    assert_eq!(
        indizes.get("IX_DispatchRuns_Project_Started"),
        Some(&vec!["ProjectId".to_string(), "StartedAt".to_string()])
    );
    assert_eq!(
        indizes.get("IX_DispatchRuns_Subject"),
        Some(&vec!["SubjectId".to_string()])
    );
    let verlauf = s.indexes("History");
    assert_eq!(verlauf.len(), 2);
    assert_eq!(
        verlauf.get("IX_History_Field_At"),
        Some(&vec!["Field".to_string(), "At".to_string()])
    );
    assert!(s.foreign_keys("DispatchRuns").is_empty());
    assert_eq!(s.runner().schema_features().unwrap(), ["dispatch-runs"]);
    let eintrag: Vec<(String, String)> = s.rows(
        "SELECT Name, Script FROM SchemaFeatures",
        &[],
        |row| Ok((row.get(0)?, row.get(1)?)),
    );
    assert_eq!(
        eintrag,
        [("dispatch-runs".to_string(), "004_dispatch_runs".to_string())]
    );
    assert_eq!(s.runner().schema_version().unwrap(), Some(3));
    s.dispose();
}

#[test]
fn stufe_004_speichert_kosten_und_tokens_und_pruft_kind_und_source() {
    let s = SchemaInspect::fresh(bis(4));
    lauf(&s, "run-1", "work", "dispatcher").expect("Lauf");
    let werte: Vec<(f64, i64, i64)> = s.rows(
        "SELECT CostUsd, InputTokens, TimedOut FROM DispatchRuns",
        &[],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    );
    assert_eq!(werte, [(0.123456, 1_234_567_890_123, 0)]);
    assert!(ist_iso(&s.text("SELECT RecordedAt FROM DispatchRuns").unwrap()));

    let fehler = lauf(&s, "run-2", "spielen", "dispatcher").expect_err("Kind");
    assert!(
        fehler.to_string().contains("CK_DispatchRuns_Kind"),
        "{fehler}"
    );
    let fehler = lauf(&s, "run-3", "merge", "hand").expect_err("Source");
    assert!(
        fehler.to_string().contains("CK_DispatchRuns_Source"),
        "{fehler}"
    );
    s.dispose();
}

#[test]
fn der_versionsschutz_nimmt_eine_frische_datei_mit_allen_stufen_an() {
    let s = SchemaInspect::fresh(SQLITE_SCHEMA_STEPS);
    let fehlend = SchemaVersionGuard
        .check(
            s.runner().schema_version().unwrap(),
            s.file(),
            &s.runner().schema_features().unwrap(),
            BoardProvider::Sqlite,
        )
        .expect("kein Fehler");
    assert!(fehlend.is_empty(), "{fehlend:?}");
    s.dispose();
}

#[test]
fn der_versionsschutz_meldet_dispatch_runs_solange_stufe_004_fehlt() {
    let s = SchemaInspect::fresh(bis(3));
    let fehlend = SchemaVersionGuard
        .check(
            s.runner().schema_version().unwrap(),
            s.file(),
            &s.runner().schema_features().unwrap(),
            BoardProvider::Sqlite,
        )
        .expect("kein Fehler");
    assert!(
        fehlend.contains(&"dispatch-runs".to_string()),
        "{fehlend:?}"
    );
    s.dispose();
}

// ---------------------------------------------------------------------------------------------
// Stufe 005_rowver (RowVer-Zähler per Trigger)
// ---------------------------------------------------------------------------------------------

/// RowVer einer Zeile
fn row_ver(s: &SchemaInspect, table: &str, id: &dyn rusqlite::ToSql) -> i64 {
    s.rows(
        &format!("SELECT RowVer AS V FROM {table} WHERE Id = ?1"),
        &[id],
        |row| row.get(0),
    )
    .into_iter()
    .next()
    .expect("Zeile")
}

/// Der datenbankweite Zähler
fn zahler(s: &SchemaInspect) -> i64 {
    s.number("SELECT Value AS V FROM ChangeCounter").unwrap()
}

#[test]
fn stufe_005_legt_zahler_und_je_tabelle_zwei_trigger_an() {
    let s = SchemaInspect::fresh(SQLITE_SCHEMA_STEPS);
    s.seed_board();
    assert_eq!(s.columns("ChangeCounter"), ["Id", "Value"]);
    let fehler = s
        .run("INSERT INTO ChangeCounter (Id, Value) VALUES (2, 0)")
        .expect_err("nur eine Zeile");
    assert!(
        fehler.to_string().contains("CK_ChangeCounter_Single"),
        "{fehler}"
    );
    assert_eq!(
        s.triggers(),
        [
            "TR_Boards_RowVer_Insert",
            "TR_Boards_RowVer_Update",
            "TR_Comments_RowVer_Insert",
            "TR_Comments_RowVer_Update",
            "TR_Notes_RowVer_Insert",
            "TR_Notes_RowVer_Update",
            "TR_Questions_RowVer_Insert",
            "TR_Questions_RowVer_Update",
            "TR_Tickets_RowVer_Insert",
            "TR_Tickets_RowVer_Update",
        ]
    );
    s.dispose();
}

#[test]
fn stufe_005_erhoht_rowver_streng_monoton_uber_alle_funf_tabellen() {
    let s = SchemaInspect::fresh(SQLITE_SCHEMA_STEPS);
    // Board 1 bekommt RowVer 1 (Users/Projects haben keinen Trigger)
    s.seed_board();
    let mut gesehen = vec![row_ver(&s, "Boards", &1)];
    s.insert_ticket("AIH-0001", 1).expect("Ticket");
    gesehen.push(row_ver(&s, "Tickets", &"AIH-0001"));
    s.run_with(
        "INSERT INTO Comments (TicketId, AuthorId, AuthorName, Body, CreatedAt) VALUES ('AIH-0001', ?1, 'T', 'b', ?2)",
        &[&USER, &NOW],
    )
    .expect("Kommentar");
    gesehen.push(row_ver(&s, "Comments", &1));
    s.run_with(
        "INSERT INTO Notes (BoardId, AuthorId, AuthorName, Body, State, CreatedAt) VALUES (1, ?1, 'T', 'b', 'open', ?2)",
        &[&USER, &NOW],
    )
    .expect("Notiz");
    gesehen.push(row_ver(&s, "Notes", &1));
    frage(&s, "AIH-F001", 1, "open", "human", None).expect("Frage");
    gesehen.push(row_ver(&s, "Questions", &"AIH-F001"));

    s.run("UPDATE Boards SET Name = 'B2' WHERE Id = 1")
        .expect("Board");
    gesehen.push(row_ver(&s, "Boards", &1));
    s.run("UPDATE Tickets SET Title = 'T2' WHERE Id = 'AIH-0001'")
        .expect("Ticket");
    gesehen.push(row_ver(&s, "Tickets", &"AIH-0001"));
    s.run_with(
        "UPDATE Comments SET ReadByAgentAt = ?1 WHERE Id = 1",
        &[&NOW],
    )
    .expect("Kommentar");
    gesehen.push(row_ver(&s, "Comments", &1));
    s.run("UPDATE Notes SET State = 'taken' WHERE Id = 1")
        .expect("Notiz");
    gesehen.push(row_ver(&s, "Notes", &1));
    s.run("UPDATE Questions SET Title = 'F2' WHERE Id = 'AIH-F001'")
        .expect("Frage");
    gesehen.push(row_ver(&s, "Questions", &"AIH-F001"));

    assert_eq!(gesehen, [1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
    assert_eq!(zahler(&s), 10);
    s.dispose();
}

#[test]
fn stufe_005_gibt_jeder_zeile_eines_updates_einen_eigenen_wert() {
    let s = SchemaInspect::fresh(SQLITE_SCHEMA_STEPS);
    s.seed_board();
    s.insert_ticket("AIH-0001", 1).expect("Ticket");
    s.insert_ticket("AIH-0002", 2).expect("Ticket");
    s.run("UPDATE Tickets SET Title = Title || '!'")
        .expect("Update");
    let werte: Vec<(String, i64)> = s.rows(
        "SELECT Id, RowVer FROM Tickets ORDER BY Id",
        &[],
        |row| Ok((row.get(0)?, row.get(1)?)),
    );
    assert_eq!(
        werte,
        [("AIH-0001".to_string(), 4), ("AIH-0002".to_string(), 5)]
    );
    assert_eq!(zahler(&s), 5);
    s.dispose();
}

#[test]
fn stufe_005_lost_beim_setzen_von_rowver_keine_endlosschleife_aus() {
    let s = SchemaInspect::fresh(SQLITE_SCHEMA_STEPS);
    s.seed_board();
    s.insert_ticket("AIH-0001", 1).expect("Ticket");
    s.run("UPDATE Tickets SET RowVer = 500 WHERE Id = 'AIH-0001'")
        .expect("RowVer von Hand");
    assert_eq!(row_ver(&s, "Tickets", &"AIH-0001"), 500);
    assert_eq!(zahler(&s), 2);

    s.run("PRAGMA recursive_triggers = ON").expect("Pragma");
    s.run("UPDATE Boards SET Name = 'B3' WHERE Id = 1")
        .expect("Board");
    assert_eq!(row_ver(&s, "Boards", &1), 3);
    assert_eq!(zahler(&s), 3);
    s.dispose();
}

#[test]
fn stufe_005_zahlt_fur_den_runner_nur_die_direkt_geanderten_zeilen() {
    let mut s = SchemaInspect::fresh(SQLITE_SCHEMA_STEPS);
    s.seed_board();
    s.insert_ticket("AIH-0001", 1).expect("Ticket");

    let mut sql = nexus_db::sqlite::SqliteRunner::new(s.connection());
    let geandert = sql
        .execute(
            "UPDATE Tickets SET Title = @title, Rev = Rev + 1 WHERE Id = @id AND Rev = @rev",
            &nexus_db::sqlite_params! { "title": "Neu", "id": "AIH-0001", "rev": 1i32 },
        )
        .expect("Update");
    assert_eq!(geandert, 1);
    let unverandert = sql
        .execute(
            "UPDATE Tickets SET Title = @title WHERE Id = @id AND Rev = @rev",
            &nexus_db::sqlite_params! { "title": "Alt", "id": "AIH-0001", "rev": 1i32 },
        )
        .expect("Update");
    assert_eq!(unverandert, 0);
    s.dispose();
}

// ---------------------------------------------------------------------------------------------
// Stufen 006_note_structure und 007_dispatch_run_details
// ---------------------------------------------------------------------------------------------

/// Die Standardliste ohne eine Stufe
fn ohne(id: &str) -> Vec<SqliteSchemaStep> {
    SQLITE_SCHEMA_STEPS
        .iter()
        .copied()
        .filter(|s| s.id != id)
        .collect()
}

#[test]
fn stufe_006_erganzt_notes_um_titel_und_zahler() {
    let s = SchemaInspect::fresh(SQLITE_SCHEMA_STEPS);
    for name in ["Title", "SeenCount", "SeenIn"] {
        assert!(s.columns("Notes").contains(&name.to_string()), "{name}");
    }
    let features = s.runner().schema_features().unwrap();
    assert!(features.contains(&"dispatch-runs".to_string()));
    assert!(features.contains(&"note-structure".to_string()));
    assert_eq!(s.runner().schema_version().unwrap(), Some(3));
    s.dispose();
}

#[test]
fn der_versionsschutz_meldet_note_structure_solange_stufe_006_fehlt() {
    let s = SchemaInspect::fresh(&ohne("006_note_structure"));
    let fehlend = SchemaVersionGuard
        .check(
            s.runner().schema_version().unwrap(),
            s.file(),
            &s.runner().schema_features().unwrap(),
            BoardProvider::Sqlite,
        )
        .expect("kein Fehler");
    assert_eq!(fehlend, ["note-structure"]);
    s.dispose();
}

#[test]
fn stufe_007_erganzt_die_lauf_spalten_und_die_konfig_versionen() {
    let s = SchemaInspect::fresh(SQLITE_SCHEMA_STEPS);
    for name in [
        "MaxTurns",
        "TimeoutMinutes",
        "ProfileReason",
        "RuleIndex",
        "ConfigHash",
        "DeniedCount",
        "DeniedTools",
    ] {
        assert!(
            s.columns("DispatchRuns").contains(&name.to_string()),
            "{name}"
        );
    }
    assert!(s.tables().contains(&"DispatchConfigVersions".to_string()));
    assert_eq!(
        s.columns("DispatchConfigVersions"),
        ["Hash", "Dispatcher", "FirstSeenAt", "Content"]
    );
    assert_eq!(s.primary_key("DispatchConfigVersions"), ["Hash"]);
    assert!(s.indexes("DispatchRuns").contains_key("IX_DispatchRuns_Config"));
    assert!(s
        .indexes("DispatchConfigVersions")
        .contains_key("IX_DispatchConfigVersions_FirstSeen"));
    let features = s.runner().schema_features().unwrap();
    assert!(features.contains(&"note-structure".to_string()));
    assert!(features.contains(&"dispatch-run-details".to_string()));
    assert_eq!(s.runner().schema_version().unwrap(), Some(3));
    s.dispose();
}

#[test]
fn stufe_007_nimmt_erlaubte_grunde_der_profilwahl_an() {
    let s = SchemaInspect::fresh(SQLITE_SCHEMA_STEPS);
    s.seed_board();
    s.run_with(
        "INSERT INTO DispatchRuns (Id, Kind, SubjectId, ProjectId, Agent, Profile, StartedAt, Outcome, Source, ProfileReason, RuleIndex)
         VALUES ('r1', 'work', 'AIH-0001', 1, 'junior', 'standard', ?1, 'done', 'dispatcher', 'rule', 2)",
        &[&NOW],
    )
    .expect("Lauf mit Regel");
    assert_eq!(
        s.text("SELECT ProfileReason FROM DispatchRuns WHERE Id = 'r1'"),
        Some("rule".to_string())
    );

    s.run_with(
        "INSERT INTO DispatchRuns (Id, Kind, SubjectId, ProjectId, Agent, Profile, StartedAt, Outcome, Source)
         VALUES ('r2', 'work', 'AIH-0001', 1, 'junior', 'standard', ?1, 'done', 'dispatcher')",
        &[&NOW],
    )
    .expect("Lauf ohne Grund");
    assert_eq!(
        s.text("SELECT ProfileReason FROM DispatchRuns WHERE Id = 'r2'"),
        None
    );

    let fehler = s
        .run_with(
            "INSERT INTO DispatchRuns (Id, Kind, SubjectId, ProjectId, Agent, Profile, StartedAt, Outcome, Source, ProfileReason)
             VALUES ('r3', 'work', 'AIH-0001', 1, 'junior', 'standard', ?1, 'done', 'dispatcher', 'irgendwas')",
            &[&NOW],
        )
        .expect_err("fremder Grund");
    assert!(
        fehler.to_string().contains("CK_DispatchRuns_ProfileReason"),
        "{fehler}"
    );
    s.dispose();
}

#[test]
fn der_versionsschutz_meldet_dispatch_run_details_solange_stufe_007_fehlt() {
    let s = SchemaInspect::fresh(&ohne("007_dispatch_run_details"));
    // Fehlende Erweiterungen werfen nicht, sie werden gemeldet; nur der Zugriff auf die neuen
    // Spalten verweigert sich
    let fehlend = SchemaVersionGuard
        .check(
            s.runner().schema_version().unwrap(),
            s.file(),
            &s.runner().schema_features().unwrap(),
            BoardProvider::Sqlite,
        )
        .expect("kein Fehler");
    assert_eq!(fehlend, ["dispatch-run-details"]);
    // Der Hinweis auf das Skript gilt für SQL Server; bei SQLite spielt der Harness selbst ein
    assert!(SchemaVersionGuard::feature_message(
        "dispatch-run-details",
        "AiHarness",
        BoardProvider::Mssql
    )
    .contains("006_dispatch_run_details.sql"));
    assert!(SchemaVersionGuard::feature_message(
        "dispatch-run-details",
        s.file(),
        BoardProvider::Sqlite
    )
    .contains("beim Öffnen selbst ein"));
    s.dispose();
}

#[test]
fn das_zweite_einspielen_andert_an_den_stufen_nichts() {
    let s = SchemaInspect::fresh(SQLITE_SCHEMA_STEPS);
    let vorher = s.columns("DispatchRuns");
    assert!(s.runner().run().unwrap().is_empty());
    assert_eq!(s.columns("DispatchRuns"), vorher);
    assert_eq!(
        s.runner()
            .schema_features()
            .unwrap()
            .iter()
            .filter(|f| *f == "dispatch-run-details")
            .count(),
        1
    );
    s.dispose();
}
