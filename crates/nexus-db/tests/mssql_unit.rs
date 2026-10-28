//! Prüfungen der SQL-Server-Schicht, die ohne Datenbank auskommen: Umschreiben der benannten
//! Parameter, Werte und Zeilen, Feature-Gate, Schema-Runner, Fehlerübersetzung, die Skripte
//! unter `packages/db/sql` und die Meldung einer nicht erreichbaren Datenbank.
//!
//! Portiert aus `packages/db/tests/MssqlSchemaRunner.test.ts`, `MssqlFeatureGate.test.ts`
//! (nur der Teil, der ohne Repositories auskommt), `SqlErrorMapper.test.ts` (SQL-Server-Teil)
//! und `SqlScripts.test.ts`.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use nexus_core::board::config::{BoardConfig, BoardProvider};
use nexus_core::Error;
use nexus_db::mssql::testing::{FakeBatchRunner, FakeRunner};
use nexus_db::mssql::{
    rewrite_parameters, script_text, to_iso, MsCell, MsRow, MsType, MsValue, MssqlBoardDatabase,
    MssqlErrorMap, MssqlFeatureGate, MssqlRunner, MssqlSchemaRunner, SchemaBatchError,
};
use nexus_db::{ConnectOptions, SchemaVersionGuard};

mod common;

// ---------------------------------------------------------------------------------------------
// Umschreiben der benannten Parameter
// ---------------------------------------------------------------------------------------------

/// Der umgeschriebene Text und die Namen in der Reihenfolge der Bindung
fn umschreiben(sql: &str, names: &[&str]) -> (String, Vec<String>) {
    let (text, order) = rewrite_parameters(sql, names);
    (
        text,
        order
            .iter()
            .map(|index| names[*index].to_string())
            .collect(),
    )
}

#[test]
fn schreibt_benannte_parameter_in_der_reihenfolge_ihres_auftretens_um() {
    let (text, bound) = umschreiben(
        "SELECT * FROM board.Tickets WHERE BoardId = @boardId AND Status = @status",
        &["status", "boardId"],
    );
    assert_eq!(
        text,
        "SELECT * FROM board.Tickets WHERE BoardId = @P1 AND Status = @P2"
    );
    assert_eq!(bound, ["boardId", "status"]);
}

#[test]
fn derselbe_name_bekommt_immer_dieselbe_nummer_und_wird_einmal_gebunden() {
    let (text, bound) = umschreiben(
        "UPDATE board.Tickets SET Rev = @rev + 1 WHERE Id = @id AND Rev = @rev",
        &["id", "rev"],
    );
    assert_eq!(
        text,
        "UPDATE board.Tickets SET Rev = @P1 + 1 WHERE Id = @P2 AND Rev = @P1"
    );
    assert_eq!(bound, ["rev", "id"]);
}

#[test]
fn bindet_namen_nicht_die_im_text_gar_nicht_vorkommen() {
    let (text, bound) = umschreiben(
        "SELECT 1 FROM board.Notes WHERE BoardId = @boardId",
        &["boardId", "source", "state"],
    );
    assert_eq!(text, "SELECT 1 FROM board.Notes WHERE BoardId = @P1");
    assert_eq!(bound, ["boardId"]);
}

#[test]
fn lasst_server_variablen_wie_trancount_und_rowcount_unberuhrt() {
    let (text, bound) = umschreiben(
        "IF @@TRANCOUNT > 0 ROLLBACK; UPDATE board.Tickets SET Title = @title WHERE Id = @id; SELECT @@ROWCOUNT AS Affected;",
        &["title", "id"],
    );
    assert_eq!(
        text,
        "IF @@TRANCOUNT > 0 ROLLBACK; UPDATE board.Tickets SET Title = @P1 WHERE Id = @P2; SELECT @@ROWCOUNT AS Affected;"
    );
    assert_eq!(bound, ["title", "id"]);
}

#[test]
fn ersetzt_nichts_in_texten_klammern_und_kommentaren() {
    let sql = "-- @id bleibt stehen\n/* @id auch /* verschachtelt @id */ hier noch */\nSELECT '@id' AS T, [@id] AS S, @id AS Echt";
    let (text, bound) = umschreiben(sql, &["id"]);
    assert_eq!(
        text,
        "-- @id bleibt stehen\n/* @id auch /* verschachtelt @id */ hier noch */\nSELECT '@id' AS T, [@id] AS S, @P1 AS Echt"
    );
    assert_eq!(bound, ["id"]);
}

#[test]
fn lasst_im_batch_selbst_erklarte_variablen_stehen() {
    let (text, bound) = umschreiben(
        "DECLARE @zahl int = 0; SELECT @zahl + @offset AS Summe",
        &["offset"],
    );
    assert_eq!(text, "DECLARE @zahl int = 0; SELECT @zahl + @P1 AS Summe");
    assert_eq!(bound, ["offset"]);
}

#[tokio::test]
async fn der_runner_merkt_sich_den_umgeschriebenen_text_und_die_bindung() {
    let mut runner = FakeRunner::with_rows(vec![MsRow::from_cells([(
        "Id".to_string(),
        MsCell::Int(7),
    )])]);
    let rows = runner
        .query_rows(
            "SELECT Id FROM board.Tickets WHERE BoardId = @boardId AND Status = @status",
            &[
                ("status", MsValue::text("backlog")),
                ("boardId", MsValue::Int(1)),
            ],
        )
        .await
        .unwrap();
    assert_eq!(rows[0].get_i32("Id").unwrap(), 7);
    assert_eq!(
        runner.last().unwrap(),
        "SELECT Id FROM board.Tickets WHERE BoardId = @P1 AND Status = @P2"
    );
    assert_eq!(runner.bound[0], ["boardId", "status"]);
}

// ---------------------------------------------------------------------------------------------
// Werte und Zeilen
// ---------------------------------------------------------------------------------------------

#[test]
fn leitet_die_typen_wie_sql_runner_type_of_ab() {
    // Texte immer als nvarchar(max), sonst gehen Umlaute über varchar verloren
    assert_eq!(
        MsValue::text("Prüfung"),
        MsValue::NVarChar("Prüfung".into())
    );
    assert_eq!(
        MsValue::text_opt(Some("x")),
        MsValue::NVarChar("x".to_string())
    );
    assert_eq!(MsValue::text_opt(None), MsValue::Null(MsType::NVarChar));
    // Ein NULL ohne Typ wüsste der Server nicht zu vergleichen – jede Variante nennt ihren Typ
    assert_eq!(
        MsValue::date_opt(None).unwrap(),
        MsValue::Null(MsType::DateTime2)
    );
    // Die ausdrücklichen Typen der Repositories
    assert_eq!(MsValue::BigInt(1), MsValue::BigInt(1));
    assert_eq!(MsValue::Decimal(1.5, 6), MsValue::Decimal(1.5, 6));
    assert_eq!(
        MsValue::VarChar("abc".into()),
        MsValue::VarChar("abc".into())
    );
}

#[test]
fn bindet_zeitpunkte_als_naive_zeit_in_utc() {
    let MsValue::DateTime2(mit_zone) = MsValue::date("2026-01-02T03:04:05.678Z").unwrap() else {
        panic!("kein datetime2");
    };
    assert_eq!(to_iso(mit_zone), "2026-01-02T03:04:05.678Z");
    // Ein Zeitpunkt mit Versatz wird nach UTC gerechnet, nicht als datetimeoffset gebunden
    let MsValue::DateTime2(versetzt) = MsValue::date("2026-01-02T05:04:05.678+02:00").unwrap()
    else {
        panic!("kein datetime2");
    };
    assert_eq!(to_iso(versetzt), "2026-01-02T03:04:05.678Z");
    assert!(MsValue::date("übermorgen").is_err());
}

#[test]
fn liest_zellen_uber_den_spaltennamen() {
    let row = MsRow::from_cells([
        ("Id".to_string(), MsCell::Int(42)),
        ("Title".to_string(), MsCell::Text("Prüfung".to_string())),
        ("Done".to_string(), MsCell::Bool(true)),
        ("Cost".to_string(), MsCell::Float(1.5)),
        (
            "UserId".to_string(),
            MsCell::Guid("AAAAAAAA-aaaa-AAAA-aaaa-AAAAAAAAAAAA".to_string()),
        ),
        ("Blocked".to_string(), MsCell::Null),
    ]);
    assert_eq!(row.get_i32("Id").unwrap(), 42);
    assert_eq!(row.get_i64("Id").unwrap(), 42);
    assert_eq!(row.get_str("Title").unwrap(), "Prüfung");
    assert!(row.get_bool("Done").unwrap());
    assert_eq!(row.get_f64("Cost").unwrap(), 1.5);
    // uniqueidentifier immer kleingeschrieben, wie SqlRunner.guid
    assert_eq!(
        row.get_guid("UserId").unwrap(),
        "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa"
    );
    assert_eq!(row.get_opt_str("Blocked").unwrap(), None);
    assert!(row.has("Id"));
    assert!(!row.has("Fehlt"));
    // NULL, wo ein Wert erwartet wird, und eine fehlende Spalte sind Fehler
    assert!(row.get_str("Blocked").is_err());
    assert!(row.get_i32("Fehlt").is_err());
}

#[test]
fn liest_zeitpunkte_als_iso_text_mit_millisekunden() {
    let row = MsRow::from_cells([
        (
            "CreatedAt".to_string(),
            MsCell::DateTime(
                chrono::NaiveDate::from_ymd_opt(2026, 1, 2)
                    .unwrap()
                    .and_hms_milli_opt(3, 4, 5, 678)
                    .unwrap(),
            ),
        ),
        ("PlannedAt".to_string(), MsCell::Null),
    ]);
    assert_eq!(
        row.get_iso("CreatedAt").unwrap(),
        "2026-01-02T03:04:05.678Z"
    );
    assert_eq!(row.get_opt_iso("PlannedAt").unwrap(), None);
}

// ---------------------------------------------------------------------------------------------
// Feature-Gate
// ---------------------------------------------------------------------------------------------

#[test]
fn verweigert_den_zugriff_mit_hinweis_auf_das_skript() {
    let gate = MssqlFeatureGate::new(vec!["note-structure".to_string()], "AiHarness");
    let error = gate.require(&["note-structure"]).unwrap_err();
    assert_eq!(error.code(), Some("board-schema-outdated"));
    let message = error.to_string();
    assert!(
        message.contains("packages/db/sql/005_note_structure.sql"),
        "{message}"
    );
    // Die Datenbank wird genannt und die übrigen Befehle laufen weiter
    assert!(message.contains("AiHarness"), "{message}");
    assert!(message.contains("laufen weiter"), "{message}");
}

#[test]
fn nennt_die_erste_fehlende_erweiterung_in_der_reihenfolge_der_angabe() {
    // Die Läufe des Dispatchers lesen auch die Spalten aus 006 – fehlt nur die, verweigert
    // sich der Zugriff trotzdem und nennt 006
    let gate = MssqlFeatureGate::new(vec!["dispatch-run-details".to_string()], "AiHarness");
    let error = gate
        .require(&["dispatch-runs", "dispatch-run-details"])
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("packages/db/sql/006_dispatch_run_details.sql"),
        "{error}"
    );
}

#[test]
fn lasst_zugriffe_durch_deren_erweiterungen_eingespielt_sind() {
    let gate = MssqlFeatureGate::new(vec!["note-structure".to_string()], "AiHarness");
    assert!(gate.require(&["dispatch-runs", "target-branch"]).is_ok());
    assert!(gate.has("dispatch-runs"));
    assert!(!gate.has("note-structure"));
    assert_eq!(gate.database(), "AiHarness");
    // Ohne fehlende Erweiterung geht alles durch
    assert!(MssqlFeatureGate::default()
        .require(&["note-structure"])
        .is_ok());
}

// ---------------------------------------------------------------------------------------------
// Schema-Runner (Port von MssqlSchemaRunner.test.ts)
// ---------------------------------------------------------------------------------------------

/// Gesammelte Warnungen und der Weg, auf dem der Runner sie meldet
fn warnungen() -> (Arc<Mutex<Vec<String>>>, nexus_db::mssql::Warn) {
    let gesammelt: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let ziel = Arc::clone(&gesammelt);
    (
        gesammelt,
        Box::new(move |message: &str| ziel.lock().unwrap().push(message.to_string())),
    )
}

/// Fehler, wie der SQL Server ihn meldet
fn sql_fehler(message: &str, number: Option<u32>) -> SchemaBatchError {
    match number {
        Some(number) => SchemaBatchError::numbered(number, message),
        None => SchemaBatchError::plain(message),
    }
}

#[tokio::test]
async fn spielt_die_fehlenden_erweiterungen_in_skript_reihenfolge_ein() {
    let (warnungen, warn) = warnungen();
    let mut runner = MssqlSchemaRunner::with_warn(FakeBatchRunner::new(), warn);
    let applied = runner
        .apply(&["note-structure".to_string(), "dispatch-runs".to_string()])
        .await;
    assert_eq!(applied, ["dispatch-runs", "note-structure"]);
    let batches = &runner.runner().batches;
    assert_eq!(batches.len(), 2);
    assert!(batches[0].contains("CREATE TABLE board.DispatchRuns ("));
    assert!(batches[1].contains("ALTER TABLE board.Notes ADD Title"));
    assert!(warnungen.lock().unwrap().is_empty());
}

#[tokio::test]
async fn lasst_eingespielte_erweiterungen_aus_und_fuhrt_ohne_fehlende_nichts_aus() {
    let (_, warn) = warnungen();
    let mut runner = MssqlSchemaRunner::with_warn(FakeBatchRunner::new(), warn);
    assert_eq!(
        runner.apply(&["note-structure".to_string()]).await,
        ["note-structure"]
    );
    assert_eq!(runner.runner().batches.len(), 1);
    assert!(runner.apply(&[]).await.is_empty());
    assert_eq!(runner.runner().batches.len(), 1);
}

#[tokio::test]
async fn schickt_den_skripttext_ohne_das_abschliessende_go() {
    let (_, warn) = warnungen();
    let mut runner = MssqlSchemaRunner::with_warn(FakeBatchRunner::new(), warn);
    runner.apply(&["note-structure".to_string()]).await;
    let batch = &runner.runner().batches[0];
    assert!(batch.contains("COMMIT TRANSACTION;"));
    assert_eq!(go_zeilen(batch), 0);
    // Die Quelle selbst hat das GO – entfernt wird es erst beim Absetzen
    let original = script("005_note_structure.sql");
    assert_eq!(go_zeilen(&original), 1);
    assert_eq!(
        script_text("note-structure").as_deref(),
        Some(batch.as_str())
    );
}

#[tokio::test]
async fn wertet_schon_eingespielt_50002_als_erfolg_ohne_zu_warnen() {
    let (gesammelt, warn) = warnungen();
    let mut runner = MssqlSchemaRunner::with_warn(
        FakeBatchRunner::failing(|_| {
            Some(sql_fehler(
                "Erweiterung note-structure ist bereits eingespielt.",
                Some(50002),
            ))
        }),
        warn,
    );
    assert_eq!(
        runner.apply(&["note-structure".to_string()]).await,
        ["note-structure"]
    );
    assert!(gesammelt.lock().unwrap().is_empty());
}

#[tokio::test]
async fn wirft_bei_fehlendem_recht_nicht_sondern_warnt_mit_dem_skriptnamen() {
    for fehler in [
        sql_fehler("Keine Berechtigung", Some(15247)),
        sql_fehler("ALTER permission was denied on object Notes", None),
    ] {
        let (gesammelt, warn) = warnungen();
        let mut runner = MssqlSchemaRunner::with_warn(
            FakeBatchRunner::failing(move |_| Some(fehler.clone())),
            warn,
        );
        assert!(runner
            .apply(&["note-structure".to_string()])
            .await
            .is_empty());
        let warnungen = gesammelt.lock().unwrap();
        assert_eq!(warnungen.len(), 1);
        assert!(
            warnungen[0].contains("005_note_structure.sql"),
            "{}",
            warnungen[0]
        );
        assert!(warnungen[0].contains("ALTER"), "{}", warnungen[0]);
    }
}

#[tokio::test]
async fn wirft_auch_bei_fehlender_vorbedingung_und_unbekannten_fehlern_nicht() {
    let (gesammelt, warn) = warnungen();
    let mut runner = MssqlSchemaRunner::with_warn(
        FakeBatchRunner::failing(|text| {
            Some(if text.contains("DispatchRuns") {
                sql_fehler("Verbindung weg", None)
            } else {
                sql_fehler("Erweiterung dispatch-runs fehlt", Some(50001))
            })
        }),
        warn,
    );
    assert!(runner
        .apply(&["dispatch-runs".to_string(), "note-structure".to_string()])
        .await
        .is_empty());
    let warnungen = gesammelt.lock().unwrap();
    assert_eq!(warnungen.len(), 2);
    assert!(warnungen[0].contains("Verbindung weg"), "{}", warnungen[0]);
    assert!(warnungen[1].contains("Vorbedingung"), "{}", warnungen[1]);
}

#[tokio::test]
async fn uberspringt_eine_erweiterung_ohne_skriptdatei() {
    let (_, warn) = warnungen();
    let mut runner = MssqlSchemaRunner::with_warn(FakeBatchRunner::new(), warn);
    assert!(runner.apply(&["unbekannt".to_string()]).await.is_empty());
    assert!(runner.runner().batches.is_empty());
    assert_eq!(script_text("unbekannt"), None);
}

#[test]
fn bettet_zu_jeder_verlangten_erweiterung_das_skript_ein() {
    for (feature, datei) in nexus_db::REQUIRED_SCHEMA_FEATURES {
        let text = script_text(feature).unwrap_or_else(|| panic!("Skript zu {feature} fehlt"));
        assert_eq!(SchemaVersionGuard::script(feature), datei);
        // Eingebettet ist genau die Datei aus packages/db/sql, nur ohne das GO
        assert_eq!(go_zeilen(&text), 0, "{feature}");
        assert_eq!(text, ohne_erstes_go(&script(datei)), "{feature}");
    }
}

// ---------------------------------------------------------------------------------------------
// Fehlerübersetzung (SQL-Server-Teil von SqlErrorMapper.test.ts)
// ---------------------------------------------------------------------------------------------

#[test]
fn macht_aus_einem_deadlock_opfer_1205_einen_conflict() {
    let harness = MssqlErrorMap::harness(Some(1205)).unwrap();
    assert_eq!(harness.code, "conflict");
    assert_eq!(
        harness.message,
        "Gleichzeitige Änderung – bitte erneut versuchen."
    );
}

#[test]
fn macht_aus_speichermangel_701_einen_board_db_overloaded() {
    let harness = MssqlErrorMap::harness(Some(701)).unwrap();
    assert_eq!(harness.code, "board-db-overloaded");
    assert!(
        harness.message.contains("Arbeitsspeicher"),
        "{}",
        harness.message
    );
}

#[test]
fn lasst_andere_fehlernummern_unverandert() {
    for code in [Some(2627), Some(2601), Some(50001), None] {
        assert!(MssqlErrorMap::harness(code).is_none(), "{code:?}");
    }
}

#[test]
fn erkennt_doppelte_schlussel_an_2627_und_2601() {
    assert!(MssqlErrorMap::is_unique_violation_number(Some(2627)));
    assert!(MssqlErrorMap::is_unique_violation_number(Some(2601)));
    assert!(!MssqlErrorMap::is_unique_violation_number(Some(1205)));
    assert!(!MssqlErrorMap::is_unique_violation_number(None));
    // Ein Fehler, der nicht vom Treiber kommt, ist nie ein doppelter Schlüssel
    assert!(!MssqlErrorMap::is_unique_violation(
        &Error::unexpected_message("x")
    ));
    assert_eq!(
        MssqlErrorMap::number_of(&Error::unexpected_message("x")),
        None
    );
    assert_eq!(
        MssqlErrorMap::number_of(&nexus_core::HarnessError::new("conflict", "x").into()),
        None
    );
}

// ---------------------------------------------------------------------------------------------
// Die Skripte unter packages/db/sql (Port von SqlScripts.test.ts)
// ---------------------------------------------------------------------------------------------

/// Ordner der SQL-Skripte, von diesem Crate aus
const SQL_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../packages/db/sql/");

/// Inhalt eines Skripts aus `packages/db/sql`
fn script(name: &str) -> String {
    std::fs::read_to_string(format!("{SQL_DIR}{name}"))
        .unwrap_or_else(|error| panic!("{name} konnte nicht gelesen werden: {error}"))
}

/// Der Text mit zu einem Leerzeichen zusammengefasstem Leerraum – so lassen sich die
/// `\s+`-Muster der TypeScript-Tests ohne regulären Ausdruck prüfen
fn eine_zeile(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Zahl der Zeilen, die nur aus `GO` bestehen
fn go_zeilen(text: &str) -> usize {
    text.lines().filter(|line| line.trim() == "GO").count()
}

/// Der Text ohne die erste Zeile, die nur aus `GO` besteht – wie der Schema-Runner ihn absetzt
fn ohne_erstes_go(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut entfernt = false;
    for line in text.split_inclusive('\n') {
        if !entfernt && line.trim() == "GO" {
            entfernt = true;
            continue;
        }
        out.push_str(line);
    }
    out
}

/// Kommt das Wort (unabhängig von Groß- und Kleinschreibung) als ganzes Wort vor?
fn hat_wort(text: &str, wort: &str) -> bool {
    let klein = text.to_lowercase();
    klein.match_indices(wort).any(|(pos, _)| {
        let grenze = |c: Option<char>| !c.is_some_and(|c| c.is_alphanumeric() || c == '_');
        grenze(klein[..pos].chars().next_back()) && grenze(klein[pos + wort.len()..].chars().next())
    })
}

/// Prüft, dass ein Skript nichts löscht; `auch_use` schließt zusätzlich `USE` aus
fn loscht_nichts(name: &str, sql: &str, auch_use: bool) {
    for wort in ["drop", "truncate"] {
        assert!(!hat_wort(sql, wort), "{name} enthält {wort}");
    }
    assert!(
        !sql.to_lowercase().contains("delete from"),
        "{name} löscht Zeilen"
    );
    if auch_use {
        assert!(!hat_wort(sql, "use"), "{name} wechselt die Datenbank");
    }
}

/// Prüft den gemeinsamen Rahmen der additiven Skripte: abbruchsicher, ein einziges `GO` am Ende
fn rahmen(name: &str, sql: &str) {
    assert!(sql.contains("SET XACT_ABORT ON;"), "{name}");
    assert!(
        eine_zeile(sql).ends_with("COMMIT TRANSACTION; GO"),
        "{name} endet nicht mit COMMIT TRANSACTION; GO"
    );
    assert_eq!(go_zeilen(sql), 1, "{name}");
}

#[test]
fn skript_000_gibt_dem_login_in_beiden_datenbanken_ddl_und_loscht_nichts() {
    let sql = script("000_create_databases.sql");
    for rolle in ["db_ddladmin", "db_datawriter"] {
        let zeile = format!("ALTER ROLE {rolle} ADD MEMBER [<LOGIN>];");
        assert_eq!(
            sql.lines().filter(|line| line.trim() == zeile).count(),
            2,
            "{rolle}"
        );
    }
    loscht_nichts("000_create_databases.sql", &sql, false);
}

#[test]
fn skript_002_legt_die_fragentabellen_an_und_setzt_version_2() {
    let sql = script("002_questions.sql");
    assert!(sql.contains("ALTER TABLE board.Projects ADD NextQuestionNumber int NOT NULL CONSTRAINT DF_Projects_NextQuestion DEFAULT 1;"));
    assert!(sql.contains("INSERT INTO board.SchemaVersion (Version) VALUES (2);"));
    for tabelle in [
        "Questions",
        "QuestionTags",
        "QuestionTickets",
        "QuestionDependencies",
        "QuestionResearch",
        "QuestionOptions",
        "QuestionComments",
        "QuestionHistory",
    ] {
        assert!(
            sql.contains(&format!("CREATE TABLE board.{tabelle} (")),
            "{tabelle}"
        );
    }
    assert!(sql.contains("CONSTRAINT UQ_Questions_Number UNIQUE (ProjectId, Number)"));
    let flach = eine_zeile(&sql);
    assert!(flach.contains("WHERE Version = 1) THROW 50001"));
    assert!(flach.contains("WHERE Version >= 2) THROW 50002"));
    rahmen("002_questions.sql", &sql);
    loscht_nichts("002_questions.sql", &sql, false);
}

#[test]
fn skript_003_erganzt_tickets_schritte_und_sessions_und_setzt_version_3() {
    let sql = script("003_harness.sql");
    for spalte in [
        "Spec     nvarchar(200) NULL",
        "SpecTask nvarchar(20)  NULL",
        "Branch   nvarchar(400) NULL",
        "Worktree nvarchar(500) NULL",
        "Touches  nvarchar(max) NOT NULL CONSTRAINT DF_Tickets_Touches DEFAULT N'[]'",
        "Consumes nvarchar(max) NOT NULL CONSTRAINT DF_Tickets_Consumes DEFAULT N'[]'",
        "Produces nvarchar(max) NOT NULL CONSTRAINT DF_Tickets_Produces DEFAULT N'[]'",
    ] {
        assert!(sql.contains(spalte), "{spalte}");
    }
    assert!(sql.contains("ALTER TABLE board.Comments ADD ReadByAgentAt datetime2(3) NULL;"));
    assert!(sql.contains("INSERT INTO board.SchemaVersion (Version) VALUES (3);"));
    assert!(sql.contains("CREATE TABLE board.TicketSteps ("));
    assert!(sql.contains("CONSTRAINT PK_TicketSteps PRIMARY KEY (TicketId, Position)"));
    assert!(sql.contains("CREATE TABLE board.TicketSessions ("));
    assert!(sql.contains("CONSTRAINT PK_TicketSessions PRIMARY KEY (TicketId, SessionId)"));
    assert_eq!(
        sql.matches("REFERENCES board.Tickets(Id) ON DELETE CASCADE")
            .count(),
        2
    );
    let flach = eine_zeile(&sql);
    assert!(flach.contains("WHERE Version = 2) THROW 50001"));
    assert!(flach.contains("WHERE Version >= 3) THROW 50002"));
    rahmen("003_harness.sql", &sql);
    loscht_nichts("003_harness.sql", &sql, true);
}

#[test]
fn skript_003_verweist_im_batch_nicht_auf_die_neuen_ticket_spalten() {
    // Sonst scheitert schon die Übersetzung des Batches: im selben Batch kennt der Übersetzer
    // die eben angelegten Spalten noch nicht
    let sql = script("003_harness.sql");
    let nach_alter = &sql[sql.find("ALTER TABLE board.Tickets ADD").unwrap() + 1..];
    let rest = &nach_alter[nach_alter.find(';').unwrap() + 1..];
    for spalte in ["SpecTask", "Worktree", "Touches", "Consumes", "Produces"] {
        assert!(!rest.contains(spalte), "{spalte}");
    }
}

#[test]
fn skript_004_legt_lauf_tabelle_indizes_und_die_erweiterungs_tabelle_an() {
    let sql = script("004_dispatch_runs.sql");
    assert!(sql.contains("CREATE TABLE board.DispatchRuns ("));
    assert!(sql.contains("CREATE TABLE board.SchemaFeatures ("));
    assert!(sql
        .contains("CONSTRAINT CK_DispatchRuns_Kind CHECK (Kind IN ('work', 'merge', 'research'))"));
    assert!(sql.contains(
        "CONSTRAINT CK_DispatchRuns_Source CHECK (Source IN ('dispatcher', 'comments'))"
    ));
    for index in [
        "IX_DispatchRuns_Project_Started",
        "IX_DispatchRuns_Subject",
        "IX_History_Field_At",
    ] {
        assert!(sql.contains(&format!("CREATE INDEX {index} ")), "{index}");
    }
    assert!(sql.contains("INSERT INTO board.SchemaFeatures (Name, Script) VALUES ('dispatch-runs', N'004_dispatch_runs.sql');"));
    // Additiv: die Schema-Version bleibt, damit ältere Stände weiterlaufen; kein Fremdschlüssel
    assert!(!sql.contains("INSERT INTO board.SchemaVersion"));
    assert!(!sql.contains("REFERENCES"));
    // Gefilterte Indizes verlangen besondere SET-Optionen von jedem Schreiber – auch von älteren
    for index in sql.match_indices("CREATE INDEX") {
        let rest = &sql[index.0..];
        let anweisung = &rest[..rest.find(';').unwrap_or(rest.len())];
        assert!(!hat_wort(anweisung, "where"), "{anweisung}");
    }
    let flach = eine_zeile(&sql);
    assert!(flach.contains("WHERE Version = 3) THROW 50001"));
    assert!(flach.contains("OBJECT_ID(N'board.DispatchRuns') IS NOT NULL THROW 50002"));
    rahmen("004_dispatch_runs.sql", &sql);
    loscht_nichts("004_dispatch_runs.sql", &sql, true);
}

#[test]
fn skript_005_erganzt_notizen_um_titel_und_zahler() {
    let sql = script("005_note_structure.sql");
    assert!(sql.contains("ALTER TABLE board.Notes ADD Title nvarchar(200) NOT NULL CONSTRAINT DF_Notes_Title DEFAULT N'';"));
    assert!(sql.contains("ALTER TABLE board.Notes ADD SeenCount int NOT NULL CONSTRAINT DF_Notes_SeenCount DEFAULT 1;"));
    assert!(sql.contains("ALTER TABLE board.Notes ADD SeenIn nvarchar(1000) NOT NULL CONSTRAINT DF_Notes_SeenIn DEFAULT N'';"));
    assert!(sql.contains("INSERT INTO board.SchemaFeatures (Name, Script) VALUES ('note-structure', N'005_note_structure.sql');"));
    assert!(!sql.contains("INSERT INTO board.SchemaVersion"));
    let flach = eine_zeile(&sql);
    assert!(flach.contains("Name = 'dispatch-runs') THROW 50001"));
    assert!(flach.contains("COL_LENGTH(N'board.Notes', N'Title') IS NOT NULL THROW 50002"));
    rahmen("005_note_structure.sql", &sql);
    loscht_nichts("005_note_structure.sql", &sql, true);
}

#[test]
fn skript_006_erganzt_die_lauf_spalten_und_die_konfig_versionen() {
    let sql = script("006_dispatch_run_details.sql");
    for spalte in [
        "ALTER TABLE board.DispatchRuns ADD MaxTurns int NULL;",
        "ALTER TABLE board.DispatchRuns ADD TimeoutMinutes int NULL;",
        "ALTER TABLE board.DispatchRuns ADD RuleIndex int NULL;",
        "ALTER TABLE board.DispatchRuns ADD ConfigHash varchar(64) NULL;",
        "ALTER TABLE board.DispatchRuns ADD DeniedCount int NULL;",
        "ALTER TABLE board.DispatchRuns ADD DeniedTools nvarchar(500) NULL;",
    ] {
        assert!(sql.contains(spalte), "{spalte}");
    }
    assert!(sql.contains("CONSTRAINT CK_DispatchRuns_ProfileReason CHECK (ProfileReason IN ('default', 'label', 'plan', 'rule', 'setting'))"));
    assert!(sql.contains("INSERT INTO board.SchemaFeatures (Name, Script) VALUES ('dispatch-run-details', N'006_dispatch_run_details.sql');"));
    assert!(!sql.contains("INSERT INTO board.SchemaVersion"));
    assert!(sql.contains("CREATE TABLE board.DispatchConfigVersions ("));
    assert!(sql.contains("CONSTRAINT PK_DispatchConfigVersions PRIMARY KEY"));
    assert!(sql.contains("CREATE INDEX IX_DispatchConfigVersions_FirstSeen "));
    // Index auf der neuen Spalte nur als Zeichenkette: im selben Batch kennt der Übersetzer
    // ConfigHash noch nicht
    assert!(sql.contains(
        "EXEC(N'CREATE INDEX IX_DispatchRuns_Config ON board.DispatchRuns(ConfigHash)');"
    ));
    let flach = eine_zeile(&sql);
    assert!(flach.contains("Name = 'note-structure') THROW 50001"));
    assert!(
        flach.contains("COL_LENGTH(N'board.DispatchRuns', N'MaxTurns') IS NOT NULL THROW 50002")
    );
    rahmen("006_dispatch_run_details.sql", &sql);
    loscht_nichts("006_dispatch_run_details.sql", &sql, true);
}

#[test]
fn skript_007_erganzt_tickets_um_den_zielbranch() {
    let sql = script("007_target_branch.sql");
    assert!(sql.contains("ALTER TABLE board.Tickets ADD TargetBranch nvarchar(400) NULL;"));
    assert!(sql.contains("INSERT INTO board.SchemaFeatures (Name, Script) VALUES ('target-branch', N'007_target_branch.sql');"));
    assert!(!sql.contains("INSERT INTO board.SchemaVersion"));
    let flach = eine_zeile(&sql);
    assert!(flach.contains("OBJECT_ID(N'board.SchemaFeatures') IS NULL THROW 50001"));
    assert!(flach.contains("Name = 'target-branch') THROW 50002"));
    rahmen("007_target_branch.sql", &sql);
    loscht_nichts("007_target_branch.sql", &sql, true);
}

// ---------------------------------------------------------------------------------------------
// Nicht erreichbare Datenbank
// ---------------------------------------------------------------------------------------------

/// Konfig auf einen Port, auf dem nichts lauscht
fn unerreichbar(port: u16) -> BoardConfig {
    BoardConfig {
        provider: BoardProvider::Mssql,
        file: String::new(),
        server: "127.0.0.1".to_string(),
        port,
        database: "AiHarness".to_string(),
        user: "harness".to_string(),
        password: "egal".to_string(),
        encrypt: false,
        trust_server_certificate: true,
        user_id: common::test_db::USER_ID.to_string(),
        user_name: "Test".to_string(),
        active_board: None,
    }
}

#[tokio::test]
async fn eine_nicht_erreichbare_datenbank_meldet_board_db_unreachable() {
    // Einen Port binden und sofort wieder freigeben: dort lauscht danach verlässlich nichts
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);

    let begonnen = Instant::now();
    let error = MssqlBoardDatabase::connect(
        &unerreichbar(port),
        ConnectOptions {
            connection_timeout_ms: Some(2000),
        },
    )
    .await
    .err()
    .expect("ein geschlossener Port darf keine Verbindung liefern");
    assert_eq!(error.code(), Some("board-db-unreachable"));
    let message = error.to_string();
    assert!(
        message.starts_with(&format!(
            "Board-Datenbank AiHarness auf 127.0.0.1:{port} ist nicht erreichbar: "
        )),
        "{message}"
    );
    // Der Grund steht hinter dem Doppelpunkt und wird nicht verschluckt
    assert!(
        message.len()
            > "Board-Datenbank AiHarness auf 127.0.0.1:65535 ist nicht erreichbar: ".len()
    );
    assert!(
        begonnen.elapsed() < Duration::from_secs(10),
        "Der Verbindungsaufbau hat das Zeitlimit überschritten: {:?}",
        begonnen.elapsed()
    );
}
