//! Prüfungen gegen eine echte SQL-Server-Datenbank. Sie laufen nur mit gesetztem
//! `HARNESS_TEST_DB`; ohne die Variable melden sie „übersprungen“ und kehren zurück.
//!
//! Aufruf:
//! `HARNESS_TEST_DB='<json>' cargo test -p nexus-db --test mssql_connect -- --test-threads=1`
//!
//! Seriell, weil alle Tests dieselbe Datenbank leeren.

use nexus_core::board::database::{BoardDatabase, BoxFut};
use nexus_core::Result;
use nexus_db::mssql::{MsValue, MssqlBoardDatabase, MssqlRunner, TiberiusRunner};
use nexus_db::{ConnectOptions, ManagedBoardDatabase, SchemaVersionGuard, EXPECTED_SCHEMA_VERSION};

mod common;

use common::test_db;

/// Verbindet mit der Test-Datenbank; `None`, solange es keine gibt
async fn datenbank(test: &str) -> Option<MssqlBoardDatabase> {
    let config = test_db::config_or_skip(test)?;
    test_db::reset(&config).await;
    Some(
        MssqlBoardDatabase::connect(&config, ConnectOptions::default())
            .await
            .expect("Verbindung zur Test-Datenbank"),
    )
}

#[tokio::test]
async fn verbindet_sich_und_meldet_die_erwartete_schema_version() {
    let Some(db) = datenbank("verbindet_sich_und_meldet_die_erwartete_schema_version").await else {
        return;
    };
    assert_eq!(db.schema_version().await.unwrap(), Some(3));
    assert_eq!(EXPECTED_SCHEMA_VERSION, 3);
    let (server, port) = db.address();
    assert!(!server.is_empty());
    assert!(port > 0);
    db.close().await.unwrap();
}

#[tokio::test]
async fn meldet_die_eingespielten_erweiterungen() {
    let Some(db) = datenbank("meldet_die_eingespielten_erweiterungen").await else {
        return;
    };
    let features = db.schema_features().await.unwrap();
    let fehlend = SchemaVersionGuard::missing(&features);
    // Was fehlt, spielt der Runner selbst ein, wenn der Login DDL auf board darf
    let eingespielt = db.ensure_schema_features(&fehlend).await.unwrap();
    let danach = SchemaVersionGuard::missing(&db.schema_features().await.unwrap());
    for name in &eingespielt {
        assert!(!danach.contains(name), "{name} fehlt weiter");
    }
    db.set_missing_features(danach);
    db.close().await.unwrap();
}

#[tokio::test]
async fn liefert_die_kennwerte_einer_leeren_datenbank() {
    let Some(db) = datenbank("liefert_die_kennwerte_einer_leeren_datenbank").await else {
        return;
    };
    // Leere Datenbank: keine Zeilen, also überall NULL bzw. 0 – NULL schreibt der Kennwert als „-“
    assert_eq!(db.change_marker(1).await.unwrap(), "0:-:-:-:-:0:-:0");
    assert_eq!(db.question_marker(1).await.unwrap(), "0:-");
    db.close().await.unwrap();
}

#[tokio::test]
async fn eine_abgebrochene_transaktion_lasst_die_kennwerte_unverandert() {
    let Some(db) = datenbank("eine_abgebrochene_transaktion_lasst_die_kennwerte_unverandert").await
    else {
        return;
    };
    let vorher = db.change_marker(1).await.unwrap();

    // Die Arbeit scheitert; die Transaktion wird zurückgerollt
    let fehler = db
        .transaction(|_uow| -> BoxFut<'_, Result<()>> {
            Box::pin(async { Err(nexus_core::Error::unexpected_message("Abbruch")) })
        })
        .await
        .unwrap_err();
    assert_eq!(fehler.to_string(), "Abbruch");

    // Eine erfolgreiche Transaktion danach zeigt, dass die Verbindung sauber zurückkam
    db.transaction(|_uow| -> BoxFut<'_, Result<()>> { Box::pin(async { Ok(()) }) })
        .await
        .unwrap();
    assert_eq!(db.change_marker(1).await.unwrap(), vorher);
    db.close().await.unwrap();
}

#[tokio::test]
async fn nach_dem_schliessen_meldet_jeder_zugriff_board_db_closed() {
    let Some(db) = datenbank("nach_dem_schliessen_meldet_jeder_zugriff_board_db_closed").await
    else {
        return;
    };
    db.close().await.unwrap();
    // Mehrfaches Schließen ist erlaubt
    db.close().await.unwrap();
    let fehler = db.change_marker(1).await.unwrap_err();
    assert_eq!(fehler.code(), Some("board-db-closed"));
}

#[tokio::test]
async fn bindet_parameter_mit_ihren_typen_und_liest_sie_zuruck() {
    let Some(config) =
        test_db::config_or_skip("bindet_parameter_mit_ihren_typen_und_liest_sie_zuruck")
    else {
        return;
    };
    let mut client = test_db::client(&config).await;
    let mut runner = TiberiusRunner::new(&mut client);
    let rows = runner
        .query_rows(
            "SELECT @text AS T, @zahl AS Z, @gross AS G, @komma AS K, @ja AS J, @zeit AS D, @leer AS L",
            &[
                // nvarchar(max), damit Umlaute nicht über varchar verloren gehen
                ("text", MsValue::text("Prüfung mit Größe ß")),
                ("zahl", MsValue::Int(42)),
                ("gross", MsValue::BigInt(9_007_199_254_740_993)),
                ("komma", MsValue::Decimal(1.5, 6)),
                ("ja", MsValue::Bit(true)),
                ("zeit", MsValue::date("2026-01-02T03:04:05.678Z").unwrap()),
                ("leer", MsValue::text_opt(None)),
            ],
        )
        .await
        .unwrap();
    let row = &rows[0];
    assert_eq!(row.get_str("T").unwrap(), "Prüfung mit Größe ß");
    assert_eq!(row.get_i32("Z").unwrap(), 42);
    assert_eq!(row.get_i64("G").unwrap(), 9_007_199_254_740_993);
    assert_eq!(row.get_f64("K").unwrap(), 1.5);
    assert!(row.get_bool("J").unwrap());
    // datetime2, nicht datetimeoffset: der Zeitpunkt kommt unverändert in UTC zurück
    assert_eq!(row.get_iso("D").unwrap(), "2026-01-02T03:04:05.678Z");
    assert_eq!(row.get_opt_str("L").unwrap(), None);
}

#[tokio::test]
async fn die_verbindung_ist_nach_einem_sql_fehler_wieder_nutzbar() {
    let Some(config) =
        test_db::config_or_skip("die_verbindung_ist_nach_einem_sql_fehler_wieder_nutzbar")
    else {
        return;
    };
    let mut client = test_db::client(&config).await;
    let mut runner = TiberiusRunner::new(&mut client);
    // Division durch null: ein Fehler des Servers, kein Abbruch der Verbindung
    assert!(runner.query_rows("SELECT 1 / 0 AS X", &[]).await.is_err());
    let rows = runner
        .query_rows("SELECT @wert AS X", &[("wert", MsValue::Int(7))])
        .await
        .unwrap();
    assert_eq!(rows[0].get_i32("X").unwrap(), 7);
}

#[tokio::test]
async fn zahlt_die_betroffenen_zeilen_einer_anderung() {
    let Some(config) = test_db::config_or_skip("zahlt_die_betroffenen_zeilen_einer_anderung")
    else {
        return;
    };
    test_db::reset(&config).await;
    let mut client = test_db::client(&config).await;
    let mut runner = TiberiusRunner::new(&mut client);
    let affected = runner
        .execute(
            "INSERT INTO board.Users (Id, Name) VALUES (@id, @name)",
            &[
                ("id", MsValue::text(test_db::USER_ID)),
                ("name", MsValue::text("Prüfer")),
            ],
        )
        .await
        .unwrap();
    assert_eq!(affected, 1);
    let rows = runner
        .query_rows(
            "SELECT Id, Name FROM board.Users WHERE Id = @id",
            &[("id", MsValue::text(test_db::USER_ID))],
        )
        .await
        .unwrap();
    assert_eq!(rows[0].get_guid("Id").unwrap(), test_db::USER_ID);
    assert_eq!(rows[0].get_str("Name").unwrap(), "Prüfer");
    // Ein Zugriff ohne Treffer meldet 0 betroffene Zeilen
    assert_eq!(
        runner
            .execute(
                "DELETE FROM board.Users WHERE Id = @id",
                &[("id", MsValue::text("22222222-2222-2222-2222-222222222222"))]
            )
            .await
            .unwrap(),
        0
    );
    runner
        .execute(
            "DELETE FROM board.Users WHERE Id = @id",
            &[("id", MsValue::text(test_db::USER_ID))],
        )
        .await
        .unwrap();
}
