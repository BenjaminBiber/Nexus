//! Der Runner für benannte Parameter; portiert aus
//! `packages/db/tests/sqlite/SqliteRunner.test.ts`.

use nexus_db::sqlite::{bool_of, date, guid, iso, iso_millis, Params, SqliteRunner};
use nexus_db::sqlite_params;
use rusqlite::Connection;

/// Verbindung im Speicher mit der Testtabelle `T`
fn verbindung() -> Connection {
    let conn = Connection::open_in_memory().expect("Verbindung");
    conn.execute_batch(
        "CREATE TABLE T (Id INTEGER PRIMARY KEY, Name TEXT NULL, Flag INTEGER NULL, At TEXT NULL, UserId TEXT NULL)",
    )
    .expect("Tabelle");
    conn
}

/// Die erste Spalte aller Zeilen als Text
fn texts(runner: &mut SqliteRunner<'_>, sql: &str) -> Vec<Option<String>> {
    runner
        .query_rows(sql, &Vec::new(), |row| row.get::<_, Option<String>>(0))
        .expect("Abfrage")
}

/// Die erste Spalte aller Zeilen als ganze Zahl
fn numbers(runner: &mut SqliteRunner<'_>, sql: &str) -> Vec<Option<i64>> {
    runner
        .query_rows(sql, &Vec::new(), |row| row.get::<_, Option<i64>>(0))
        .expect("Abfrage")
}

#[test]
fn datum_hin_und_zuruck() {
    let mut conn = verbindung();
    let mut sql = SqliteRunner::new(&mut conn);
    sql.execute(
        "INSERT INTO T (At) VALUES (@at)",
        &vec![("at", date("2026-10-21T07:45:42.579Z"))],
    )
    .expect("Insert");

    let gelesen = texts(&mut sql, "SELECT At FROM T");
    assert_eq!(gelesen[0].as_deref(), Some("2026-10-21T07:45:42.579Z"));
    assert_eq!(
        iso(gelesen[0].clone()),
        Some("2026-10-21T07:45:42.579Z".to_string())
    );
    // Eine Angabe ohne Millisekunden bekommt sie beim Binden
    assert_eq!(iso_millis("2026-10-21T07:45:42Z"), "2026-10-21T07:45:42.000Z");
}

#[test]
fn datum_wird_auch_beim_binden_auf_millisekunden_gebracht() {
    let mut conn = verbindung();
    let mut sql = SqliteRunner::new(&mut conn);
    sql.execute(
        "INSERT INTO T (At) VALUES (@at)",
        &vec![("at", date("2026-10-21T09:45:42+02:00"))],
    )
    .expect("Insert");
    assert_eq!(
        texts(&mut sql, "SELECT At FROM T")[0].as_deref(),
        Some("2026-10-21T07:45:42.000Z")
    );
}

#[test]
fn bool_wird_null_oder_eins() {
    let mut conn = verbindung();
    let mut sql = SqliteRunner::new(&mut conn);
    sql.execute(
        "INSERT INTO T (Flag) VALUES (@flag)",
        &sqlite_params! { "flag": true },
    )
    .expect("Insert");
    sql.execute(
        "INSERT INTO T (Flag) VALUES (@flag)",
        &sqlite_params! { "flag": false },
    )
    .expect("Insert");

    let flags = numbers(&mut sql, "SELECT Flag FROM T ORDER BY Id");
    assert_eq!(flags, vec![Some(1), Some(0)]);
    assert!(bool_of(1));
    assert!(!bool_of(0));
}

#[test]
fn guids_gelten_kleingeschrieben() {
    let mut conn = verbindung();
    let mut sql = SqliteRunner::new(&mut conn);
    sql.execute(
        "INSERT INTO T (UserId) VALUES (@userId)",
        &sqlite_params! { "userId": "ABCDEF00-0000-0000-0000-00000000000A" },
    )
    .expect("Insert");

    let gelesen = texts(&mut sql, "SELECT UserId FROM T");
    assert_eq!(
        guid(gelesen[0].as_deref().unwrap()),
        "abcdef00-0000-0000-0000-00000000000a"
    );
}

#[test]
fn null_bleibt_null() {
    let mut conn = verbindung();
    let mut sql = SqliteRunner::new(&mut conn);
    // „name“ ist ausdrücklich NULL, „at“ wird gar nicht mitgegeben – beides wird NULL
    sql.execute(
        "INSERT INTO T (Name, At) VALUES (@name, @at)",
        &sqlite_params! { "name": None::<String> },
    )
    .expect("Insert");

    let zeilen: Vec<(Option<String>, Option<String>)> = sql
        .query_rows("SELECT Name, At FROM T", &Vec::new(), |row| {
            Ok((row.get(0)?, row.get(1)?))
        })
        .expect("Abfrage");
    assert_eq!(zeilen, vec![(None, None)]);
    assert_eq!(iso(None), None);
}

#[test]
fn umlaute_bleiben_erhalten() {
    let mut conn = verbindung();
    let mut sql = SqliteRunner::new(&mut conn);
    sql.execute(
        "INSERT INTO T (Name) VALUES (@name)",
        &sqlite_params! { "name": "Ä ö ß" },
    )
    .expect("Insert");
    assert_eq!(
        texts(&mut sql, "SELECT Name FROM T")[0].as_deref(),
        Some("Ä ö ß")
    );
}

#[test]
fn benannte_parameter_mit_at() {
    let mut conn = verbindung();
    let mut sql = SqliteRunner::new(&mut conn);
    // „a“ kommt zweimal vor, „fehlt“ wird nicht mitgegeben, „unbenutzt“ steht nicht im SQL
    let params: Params = sqlite_params! { "a": 5i32, "unbenutzt": 1i32 };
    let zeilen: Vec<(Option<i64>, Option<i64>, Option<i64>)> = sql
        .query_rows(
            "SELECT @a AS x, @a AS y, @fehlt AS z",
            &params,
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("Abfrage");
    assert_eq!(zeilen, vec![(Some(5), Some(5), None)]);

    // In Text und Kommentar ist @ kein Parameter
    let zeilen = texts(&mut sql, "SELECT '@a' AS v -- @b");
    assert_eq!(zeilen[0].as_deref(), Some("@a"));
}

#[test]
fn geanderte_zeilen_werden_gezahlt() {
    let mut conn = verbindung();
    let mut sql = SqliteRunner::new(&mut conn);
    assert_eq!(
        sql.execute(
            "INSERT INTO T (Name) VALUES (@n)",
            &sqlite_params! { "n": "a" }
        )
        .expect("Insert"),
        1
    );
    sql.execute(
        "INSERT INTO T (Name) VALUES (@n)",
        &sqlite_params! { "n": "b" },
    )
    .expect("Insert");

    assert_eq!(
        sql.execute("UPDATE T SET Name = @n", &sqlite_params! { "n": "c" })
            .expect("Update"),
        2
    );
    assert_eq!(
        sql.execute("DELETE FROM T WHERE Id = 99", &Vec::new())
            .expect("Delete"),
        0
    );

    // Eine Abfrage liefert ihre Zeilen, nicht eine Zahl
    assert_eq!(numbers(&mut sql, "SELECT Id FROM T").len(), 2);

    // RETURNING liefert Zeilen und geht deshalb über query_rows
    let zeilen: Vec<i64> = sql
        .query_rows(
            "INSERT INTO T (Name) VALUES (@n) RETURNING Id",
            &sqlite_params! { "n": "d" },
            |row| row.get(0),
        )
        .expect("Returning");
    assert_eq!(zeilen.len(), 1);

    // Auch hinter einem WITH zählt der Schreibvorgang
    assert_eq!(
        sql.execute(
            "WITH x AS (SELECT 1 AS a) UPDATE T SET Name = 'w' WHERE Id IN (SELECT a FROM x)",
            &Vec::new()
        )
        .expect("With-Update"),
        1
    );
}

#[test]
fn je_aufruf_nur_eine_anweisung() {
    let mut conn = verbindung();
    let mut sql = SqliteRunner::new(&mut conn);
    let fehler = sql
        .query_rows("SELECT 1; SELECT 2", &Vec::new(), |row| {
            row.get::<_, i64>(0)
        })
        .expect_err("zwei Anweisungen");
    assert_eq!(fehler.code(), Some("sqlite-multi-statement"));

    let fehler = sql
        .execute("DELETE FROM T; DELETE FROM T", &Vec::new())
        .expect_err("zwei Anweisungen");
    assert_eq!(fehler.code(), Some("sqlite-multi-statement"));

    // Ein abschließendes Semikolon und ein Semikolon im Text sind in Ordnung
    assert_eq!(numbers(&mut sql, "SELECT 1;"), vec![Some(1)]);
    assert_eq!(
        texts(&mut sql, "SELECT ';' AS s")[0].as_deref(),
        Some(";")
    );
}
