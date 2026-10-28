//! Fabrik und geprüftes Öffnen ohne SQL Server; portiert aus
//! `packages/db/tests/BoardDatabaseFactory.test.ts`, den datenbankfreien Fällen aus
//! `packages/db/tests/BoardRuntime.test.ts` und den Fällen aus
//! `packages/db/tests/sqlite/SqliteBoardRuntime.test.ts`, die keine Dienste brauchen.
//!
//! Aufruf: `cargo test -p nexus-db --test factory` – braucht keine Datenbank. Statt der
//! Spione von vitest arbeiten die Fälle mit einer Ersatz-Datenbank ([`FakeDb`]), die
//! Schema-Version und Erweiterungen vorgibt und die Aufrufe mitschreibt.

use std::sync::{Arc, Mutex};

use nexus_core::board::config::{BoardConfig, BoardProvider};
use nexus_core::board::database::{BoardDatabase, NewProject, TransactionWork};
use nexus_core::board::types::BoardUser;
use nexus_core::{tx, Result};
use nexus_db::factory::{open_checked, open_checked_with, AnyBoardDatabase, BoardDatabaseFactory};
use nexus_db::{
    ConnectOptions, InMemoryBoardDatabase, ManagedBoardDatabase, EXPECTED_SCHEMA_VERSION,
    REQUIRED_SCHEMA_FEATURES,
};
use tempfile::TempDir;

/// Entwickler-GUID der Fälle, wie `USER_ID` in den TypeScript-Tests
const USER_ID: &str = "11111111-1111-1111-1111-111111111111";

/// Konfig für SQL Server, die auf einen Port zeigt, auf dem nichts hört (wie `CONFIG` in
/// `BoardDatabaseFactory.test.ts`)
fn mssql_config() -> BoardConfig {
    BoardConfig {
        provider: BoardProvider::Mssql,
        file: String::new(),
        server: "127.0.0.1".into(),
        port: 1,
        database: "AiHarness".into(),
        user: "u".into(),
        password: "p".into(),
        encrypt: false,
        trust_server_certificate: true,
        user_id: USER_ID.into(),
        user_name: "F".into(),
        active_board: None,
    }
}

/// Vollständige Konfig für SQLite; die MSSQL-Felder bleiben leer und werden nicht gelesen
fn sqlite_config(file: &str) -> BoardConfig {
    BoardConfig {
        provider: BoardProvider::Sqlite,
        file: file.to_string(),
        server: String::new(),
        port: 1433,
        database: String::new(),
        user: String::new(),
        password: String::new(),
        encrypt: true,
        trust_server_certificate: true,
        user_id: USER_ID.into(),
        user_name: "Test".into(),
        active_board: None,
    }
}

/// Alle verlangten Erweiterungen; aus dem Guard gelesen, damit eine weitere Stufe diese Tests
/// nicht erneut bricht
fn alle() -> Vec<String> {
    REQUIRED_SCHEMA_FEATURES
        .iter()
        .map(|(name, _)| (*name).to_string())
        .collect()
}

/// Ein temporärer Ordner samt Pfad einer noch nicht angelegten `board.db`
fn temp_file() -> (TempDir, String) {
    let dir = TempDir::with_prefix("nexus-factory-").expect("temporärer Ordner");
    let file = dir.path().join("board.db").to_string_lossy().to_string();
    (dir, file)
}

// =============================================================================== Ersatz-DB

/// Datenbank-Ersatz: gibt Schema-Version und Erweiterungen vor, schreibt die Aufrufe mit und
/// führt Transaktionen auf einer Speicher-Datenbank aus (Vorbild: `fake` in
/// `BoardRuntime.test.ts`).
///
/// Die Verträge erfüllt [`Shared`], eine geteilte Sicht darauf: der Fall prüft die
/// mitgeschriebenen Aufrufe hinterher, auch wenn das geprüfte Öffnen die Datenbank bei einem
/// Fehler schließt und fallen lässt.
struct FakeDb {
    inner: InMemoryBoardDatabase,
    version: Option<i32>,
    /// Die Erweiterungen, die die DB gerade meldet
    features: Mutex<Vec<String>>,
    /// Was ein Einspielen tatsächlich hinzufügt (leer = kein DDL-Recht)
    applies: Vec<String>,
    /// Die Listen, mit denen `ensure_schema_features` aufgerufen wurde
    ensured: Mutex<Vec<Vec<String>>>,
    /// Die Listen, mit denen `set_missing_features` aufgerufen wurde
    missing: Mutex<Vec<Vec<String>>>,
    /// Wie oft `close` gerufen wurde
    closed: Mutex<u32>,
}

impl FakeDb {
    fn new(features: Vec<String>, version: Option<i32>, applies: Vec<String>) -> Self {
        Self {
            inner: InMemoryBoardDatabase::new(),
            version,
            features: Mutex::new(features),
            applies,
            ensured: Mutex::new(Vec::new()),
            missing: Mutex::new(Vec::new()),
            closed: Mutex::new(0),
        }
    }

    /// Eine DB mit der erwarteten Version
    fn aktuell(features: Vec<String>, applies: Vec<String>) -> Self {
        Self::new(features, Some(EXPECTED_SCHEMA_VERSION), applies)
    }

    fn ensured(&self) -> Vec<Vec<String>> {
        self.ensured.lock().expect("ensured").clone()
    }

    fn gemerkt(&self) -> Vec<Vec<String>> {
        self.missing.lock().expect("missing").clone()
    }

    fn schliessungen(&self) -> u32 {
        *self.closed.lock().expect("closed")
    }
}

/// Geteilte Sicht auf eine [`FakeDb`]; eigener Typ, weil die Verträge nicht für `Arc<…>`
/// erfüllt werden dürfen (Waisenregel)
struct Shared(Arc<FakeDb>);

impl BoardDatabase for Shared {
    async fn transaction<'a, T, W>(&'a self, work: W) -> Result<T>
    where
        T: Send + 'static,
        W: TransactionWork<T> + 'a,
    {
        self.0.inner.transaction(work).await
    }

    async fn change_marker(&self, board_id: i32) -> Result<String> {
        self.0.inner.change_marker(board_id).await
    }

    async fn question_marker(&self, project_id: i32) -> Result<String> {
        self.0.inner.question_marker(project_id).await
    }

    async fn close(&self) -> Result<()> {
        *self.0.closed.lock().expect("closed") += 1;
        self.0.inner.close().await
    }
}

impl ManagedBoardDatabase for Shared {
    async fn schema_version(&self) -> Result<Option<i32>> {
        Ok(self.0.version)
    }

    async fn schema_features(&self) -> Result<Vec<String>> {
        Ok(self.0.features.lock().expect("features").clone())
    }

    async fn ensure_schema_features<'a>(&'a self, missing: &'a [String]) -> Result<Vec<String>> {
        self.0
            .ensured
            .lock()
            .expect("ensured")
            .push(missing.to_vec());
        let mut features = self.0.features.lock().expect("features");
        features.extend(self.0.applies.iter().cloned());
        Ok(self.0.applies.clone())
    }

    fn set_missing_features(&self, names: Vec<String>) {
        self.0.missing.lock().expect("missing").push(names);
    }
}

/// Öffnet mit der Ersatz-DB; die DB bleibt dem Fall zur Prüfung erhalten
async fn mit_ersatz(db: FakeDb) -> (Result<Vec<String>>, Arc<FakeDb>) {
    let db = Arc::new(db);
    let config = mssql_config();
    let ergebnis = open_checked_with(&config, ConnectOptions::default(), async |_c, _o| {
        Ok(Shared(Arc::clone(&db)))
    })
    .await
    .map(|geoeffnet| geoeffnet.missing_features);
    (ergebnis, db)
}

// =============================================================================== Fabrik

#[tokio::test]
async fn liefert_fur_mssql_die_sql_server_datenbank() {
    // Ohne SQL Server zeigt der Zweig sich daran, dass er den Server anspricht: der Port ist
    // zu, die Meldung nennt Server und Port – bei SQLite würde niemand verbinden.
    let options = ConnectOptions {
        connection_timeout_ms: Some(1500),
    };
    let fehler = BoardDatabaseFactory::open(&mssql_config(), options)
        .await
        .err()
        .expect("SQL Server auf Port 1 ist nicht erreichbar");
    assert_eq!(fehler.code(), Some("board-db-unreachable"));
    assert!(fehler.to_string().contains("127.0.0.1:1"), "{fehler}");
}

#[tokio::test]
async fn liefert_fur_sqlite_die_datei_datenbank() {
    let (_dir, file) = temp_file();
    let db = BoardDatabaseFactory::open(&sqlite_config(&file), ConnectOptions::default())
        .await
        .expect("Datei-Datenbank");
    assert_eq!(db.provider(), BoardProvider::Sqlite);
    assert!(matches!(db, AnyBoardDatabase::Sqlite(_)));
    assert!(std::path::Path::new(&file).exists());
    db.close().await.expect("schließen");
}

#[test]
fn meldet_einen_unbekannten_anbieter_verstandlich() {
    let fehler = BoardDatabaseFactory::provider("oracle").unwrap_err();
    assert_eq!(fehler.code(), Some("board-provider-unknown"));
    assert!(fehler.to_string().contains("oracle"), "{fehler}");
    assert_eq!(
        BoardDatabaseFactory::provider("mssql").unwrap(),
        BoardProvider::Mssql
    );
    assert_eq!(
        BoardDatabaseFactory::provider("sqlite").unwrap(),
        BoardProvider::Sqlite
    );
}

// =============================================================================== Erweiterungen

#[tokio::test]
async fn spielt_die_fehlende_erweiterung_selbst_ein_und_lauft_danach_ohne_einschrankung() {
    let (ergebnis, db) = mit_ersatz(FakeDb::aktuell(Vec::new(), alle())).await;
    assert_eq!(ergebnis.expect("öffnen"), Vec::<String>::new());
    assert_eq!(db.ensured(), vec![alle()]);
    assert_eq!(db.gemerkt(), vec![Vec::<String>::new()]);
    assert_eq!(db.schliessungen(), 0);
}

#[tokio::test]
async fn verbindet_auch_ohne_ddl_recht_und_merkt_sich_was_weiter_fehlt() {
    let erste = alle()[0].clone();
    let rest = alle()[1..].to_vec();
    let (ergebnis, db) = mit_ersatz(FakeDb::aktuell(vec![erste], Vec::new())).await;
    assert_eq!(ergebnis.expect("öffnen"), rest);
    assert_eq!(db.ensured(), vec![rest.clone()]);
    assert_eq!(db.gemerkt(), vec![rest]);
}

#[tokio::test]
async fn versucht_nichts_einzuspielen_wenn_alles_da_ist() {
    let (ergebnis, db) = mit_ersatz(FakeDb::aktuell(alle(), Vec::new())).await;
    assert_eq!(ergebnis.expect("öffnen"), Vec::<String>::new());
    assert_eq!(db.ensured(), Vec::<Vec<String>>::new());
    assert_eq!(db.gemerkt(), Vec::<Vec<String>>::new());
}

#[tokio::test]
async fn bricht_bei_falscher_schema_version_mit_hinweis_auf_das_skript_ab() {
    let (ergebnis, db) = mit_ersatz(FakeDb::new(
        alle(),
        Some(EXPECTED_SCHEMA_VERSION - 1),
        Vec::new(),
    ))
    .await;
    let fehler = ergebnis.expect_err("veraltetes Schema");
    assert_eq!(fehler.code(), Some("board-schema-outdated"));
    assert!(fehler.to_string().contains("003_"), "{fehler}");
    assert_eq!(db.ensured(), Vec::<Vec<String>>::new());
    // Bei einem Fehler wird die Verbindung wieder geschlossen
    assert_eq!(db.schliessungen(), 1);
}

#[tokio::test]
async fn legt_den_benutzer_der_konfig_an() {
    let (ergebnis, db) = mit_ersatz(FakeDb::aktuell(alle(), Vec::new())).await;
    ergebnis.expect("öffnen");
    let geteilt = Shared(Arc::clone(&db));
    let benutzer = tx!(geteilt, |u| { u.users().by_id(USER_ID).await })
        .await
        .expect("lesen");
    assert_eq!(
        benutzer,
        Some(BoardUser {
            id: USER_ID.into(),
            name: "F".into()
        })
    );
}

// =============================================================================== SQLite

#[tokio::test]
async fn legt_in_einer_leeren_datei_das_schema_an_und_tragt_den_benutzer_ein() {
    let (dir, _file) = temp_file();
    // Auch ein fehlender Unterordner wird angelegt – wie in SqliteBoardRuntime.test.ts
    let file = dir
        .path()
        .join("unter")
        .join("board.db")
        .to_string_lossy()
        .to_string();
    let geoeffnet = open_checked(&sqlite_config(&file), ConnectOptions::default())
        .await
        .expect("öffnen");
    assert!(std::path::Path::new(&file).exists());
    assert_eq!(geoeffnet.missing_features, Vec::<String>::new());

    let benutzer = tx!(geoeffnet.db, |u| { u.users().by_id(USER_ID).await })
        .await
        .expect("lesen");
    assert_eq!(
        benutzer,
        Some(BoardUser {
            id: USER_ID.into(),
            name: "Test".into()
        })
    );
    geoeffnet.db.close().await.expect("schließen");
}

#[tokio::test]
async fn offnet_dieselbe_datei_erneut_mit_den_gespeicherten_daten() {
    let (_dir, file) = temp_file();
    let config = sqlite_config(&file);

    let erste = open_checked(&config, ConnectOptions::default())
        .await
        .expect("öffnen");
    tx!(erste.db, |u| {
        u.projects()
            .insert(&NewProject {
                key: "TST".into(),
                name: "Test".into(),
                created_at: "2026-10-28T10:00:00.000Z".into(),
                created_by: USER_ID.into(),
            })
            .await
    })
    .await
    .expect("Projekt anlegen");
    erste.db.close().await.expect("schließen");

    let zweite = open_checked(&config, ConnectOptions::default())
        .await
        .expect("erneut öffnen");
    let schluessel: Vec<String> = tx!(zweite.db, |u| { u.projects().list().await })
        .await
        .expect("lesen")
        .into_iter()
        .map(|projekt| projekt.key)
        .collect();
    assert_eq!(schluessel, ["TST"]);
    zweite.db.close().await.expect("schließen");
}

#[tokio::test]
async fn meldet_eine_datei_die_keine_board_datei_ist() {
    let (dir, _file) = temp_file();
    let file = dir.path().join("fremd.db").to_string_lossy().to_string();
    // Eine SQLite-Datei ohne Board-Schema legt der Schema-Runner beim Öffnen selbst an; ein
    // Fehler entsteht nur bei einer Datei, die SQLite gar nicht lesen kann.
    std::fs::write(&file, b"kein SQLite").expect("Datei schreiben");
    assert!(
        open_checked(&sqlite_config(&file), ConnectOptions::default())
            .await
            .is_err()
    );
}
