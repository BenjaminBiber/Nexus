//! Zugang zur Test-Datenbank aus `HARNESS_TEST_DB`; portiert aus
//! `packages/db/tests/helpers/TestDb.ts`.
//!
//! Ohne die Umgebungsvariable gibt es keine Test-Datenbank: die Tests, die eine brauchen,
//! geben „übersprungen“ aus und kehren zurück. Der Harness legt selbst keine Datenbank und
//! keine Zugangsdaten an – bereitgestellt wird sie heute mit
//! `packages/db/docker/compose.yaml` und einer `.env`, die ein Mensch anlegt.

#![allow(dead_code)] // nicht jedes Testprogramm braucht jede Hilfe

use bb8::Pool;
use bb8_tiberius::ConnectionManager;
use nexus_core::board::config::{BoardConfig, BoardProvider};
use nexus_db::mssql::MssqlClient;
use tiberius::EncryptionLevel;

/// Entwickler-GUID, mit der die Tests schreiben; wie `USER_ID` in `TestDb.ts`
pub const USER_ID: &str = "11111111-1111-1111-1111-111111111111";

/// Name der Umgebungsvariable mit dem Zugang zur Test-Datenbank
pub const ENV_VAR: &str = "HARNESS_TEST_DB";

/// Leert alle Tabellen des Schemas `board`; die Reihenfolge ist die der Fremdschlüssel
const RESET_SQL: &str = "
DELETE FROM board.QuestionOptions; DELETE FROM board.QuestionResearch; DELETE FROM board.QuestionComments;
DELETE FROM board.QuestionHistory; DELETE FROM board.QuestionDependencies; DELETE FROM board.QuestionTickets;
DELETE FROM board.QuestionTags; DELETE FROM board.Questions;
DELETE FROM board.TicketSteps; DELETE FROM board.TicketSessions; DELETE FROM board.DispatchRuns;
DELETE FROM board.History; DELETE FROM board.Comments; DELETE FROM board.TicketLabels;
DELETE FROM board.TicketDependencies; DELETE FROM board.Notes; DELETE FROM board.Tickets;
DELETE FROM board.Boards; DELETE FROM board.Projects; DELETE FROM board.Deletions; DELETE FROM board.Users;";

/// Setzt die `IDENTITY`-Zähler aller Tabellen des Schemas `board` zurück, damit die nächste
/// Zeile wieder die Id 1 bekommt. `DELETE` lässt die Zähler stehen; die Gegenprobe braucht
/// aber auf beiden Seiten dieselben Ids, weil der Datensatz sie (als `projectId`, `boardId`,
/// `noteById(1)` …) nennt.
const RESEED_SQL: &str = "
DECLARE @sql nvarchar(max) = N'';
SELECT @sql = @sql + N'DBCC CHECKIDENT(''board.' + t.name + N''', RESEED, 0) WITH NO_INFOMSGS;'
FROM sys.identity_columns c
JOIN sys.tables t ON t.object_id = c.object_id
JOIN sys.schemas s ON s.schema_id = t.schema_id
WHERE s.name = 'board';
EXEC sp_executesql @sql;";

/// Die Konfig aus `HARNESS_TEST_DB` oder `None`, wenn die Variable fehlt.
///
/// Zeigt die Variable auf eine Datenbank ohne die Endung `_Test`, bricht der Test ab: ein
/// Lauf gegen die echte Board-Datenbank würde sie leeren.
pub fn config() -> Option<BoardConfig> {
    let raw = std::env::var(ENV_VAR).ok().filter(|raw| !raw.is_empty())?;
    let data: serde_json::Value = serde_json::from_str(&raw)
        .unwrap_or_else(|error| panic!("{ENV_VAR} ist kein JSON: {error}"));
    let text = |key: &str| {
        data.get(key)
            .and_then(serde_json::Value::as_str)
            .unwrap_or_else(|| panic!("{ENV_VAR} braucht den Eintrag {key}."))
            .to_string()
    };
    let flag = |key: &str, standard: bool| {
        data.get(key)
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(standard)
    };
    let database = text("database");
    assert!(
        database.ends_with("_Test"),
        "{ENV_VAR} muss auf eine Datenbank mit Endung _Test zeigen."
    );
    Some(BoardConfig {
        provider: BoardProvider::Mssql,
        // file gilt nur bei Anbieter sqlite
        file: String::new(),
        server: text("server"),
        port: data
            .get("port")
            .and_then(serde_json::Value::as_u64)
            .map_or(1433, |port| port as u16),
        database,
        user: text("user"),
        password: text("password"),
        encrypt: flag("encrypt", true),
        trust_server_certificate: flag("trustServerCertificate", true),
        user_id: USER_ID.to_string(),
        user_name: "Test".to_string(),
        active_board: None,
    })
}

/// Meldet den übersprungenen Test und liefert `None`, solange es keine Test-Datenbank gibt.
///
/// Rust kennt kein `describe.skip`: die Tests prüfen das Ergebnis am Anfang und kehren zurück.
pub fn config_or_skip(test: &str) -> Option<BoardConfig> {
    let config = config();
    if config.is_none() {
        println!("{test} übersprungen: {ENV_VAR} fehlt");
    }
    config
}

/// Die tiberius-Konfig zur Board-Konfig; dieselben Einstellungen wie beim Verbindungsaufbau
/// der Board-Datenbank
pub fn tiberius_config(config: &BoardConfig) -> tiberius::Config {
    let mut cfg = tiberius::Config::new();
    cfg.host(&config.server);
    cfg.port(config.port);
    cfg.database(&config.database);
    cfg.authentication(tiberius::AuthMethod::sql_server(
        &config.user,
        &config.password,
    ));
    cfg.encryption(if config.encrypt {
        EncryptionLevel::Required
    } else {
        EncryptionLevel::Off
    });
    if config.trust_server_certificate {
        cfg.trust_cert();
    }
    cfg.application_name("nexus-test");
    cfg
}

/// Eine eigene Verbindung neben der Board-Datenbank – für Tests, die rohes SQL brauchen
pub async fn client(config: &BoardConfig) -> MssqlClient {
    Pool::builder()
        .max_size(1)
        .build_unchecked(ConnectionManager::new(tiberius_config(config)))
        .dedicated_connection()
        .await
        .expect("Verbindung zur Test-Datenbank")
}

/// Leert alle Tabellen des Schemas `board`
pub async fn reset(config: &BoardConfig) {
    batch(config, RESET_SQL, "Test-Datenbank leeren").await;
}

/// Leert alle Tabellen und setzt zusätzlich die `IDENTITY`-Zähler zurück; für die Gegenprobe,
/// die auf beiden Seiten dieselben Ids erwartet
pub async fn reset_with_identities(config: &BoardConfig) {
    reset(config).await;
    batch(config, RESEED_SQL, "IDENTITY-Zähler zurücksetzen").await;
}

/// Setzt einen Batch auf einer eigenen Verbindung ab und holt alle Ergebnismengen ab
async fn batch(config: &BoardConfig, sql: &str, was: &str) {
    let mut client = client(config).await;
    client
        .simple_query(sql)
        .await
        .unwrap_or_else(|error| panic!("{was}: {error}"))
        .into_results()
        .await
        .unwrap_or_else(|error| panic!("{was}: {error}"));
}
