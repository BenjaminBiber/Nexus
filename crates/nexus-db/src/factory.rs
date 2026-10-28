//! Öffnet die Board-Datenbank des in der Konfig gewählten Anbieters; portiert aus
//! `packages/db/src/BoardDatabaseFactory.ts` samt dem Datenbank-Teil von
//! `BoardRuntime.connect` (`packages/db/src/BoardRuntime.ts`).
//!
//! Die Laufzeit selbst (`BoardRuntime` mit Konfigdatei, Diensten und Handelndem) entsteht erst
//! in Phase 3; hier steht nur, was CLI und Server zum Öffnen brauchen: Anbieterwahl,
//! Schema-Prüfung, fehlende Erweiterungen und der eigene Benutzer.

use nexus_core::board::config::{BoardConfig, BoardProvider};
use nexus_core::board::database::{BoardDatabase, BoardUnitOfWork, BoxFut, TransactionWork};
use nexus_core::board::types::BoardUser;
use nexus_core::{HarnessError, Result};

use crate::managed::{ConnectOptions, ManagedBoardDatabase};
use crate::mssql::MssqlBoardDatabase;
use crate::schema_version_guard::SchemaVersionGuard;
use crate::sqlite::SqliteBoardDatabase;

/// Board-Datenbank eines Anbieters; leitet alle Aufrufe weiter.
///
/// [`BoardDatabase`] ist wegen der generischen `transaction` nicht objektsicher – statt
/// `Box<dyn BoardDatabase>` gibt es deshalb dieses Enum über die Anbieter.
pub enum AnyBoardDatabase {
    Sqlite(SqliteBoardDatabase),
    Mssql(MssqlBoardDatabase),
}

impl AnyBoardDatabase {
    /// Der Anbieter dieser Datenbank
    pub fn provider(&self) -> BoardProvider {
        match self {
            AnyBoardDatabase::Sqlite(_) => BoardProvider::Sqlite,
            AnyBoardDatabase::Mssql(_) => BoardProvider::Mssql,
        }
    }
}

impl BoardDatabase for AnyBoardDatabase {
    async fn transaction<'a, T, W>(&'a self, work: W) -> Result<T>
    where
        T: Send + 'static,
        W: TransactionWork<T> + 'a,
    {
        match self {
            AnyBoardDatabase::Sqlite(db) => db.transaction(work).await,
            AnyBoardDatabase::Mssql(db) => db.transaction(work).await,
        }
    }

    async fn change_marker(&self, board_id: i32) -> Result<String> {
        match self {
            AnyBoardDatabase::Sqlite(db) => db.change_marker(board_id).await,
            AnyBoardDatabase::Mssql(db) => db.change_marker(board_id).await,
        }
    }

    async fn question_marker(&self, project_id: i32) -> Result<String> {
        match self {
            AnyBoardDatabase::Sqlite(db) => db.question_marker(project_id).await,
            AnyBoardDatabase::Mssql(db) => db.question_marker(project_id).await,
        }
    }

    async fn close(&self) -> Result<()> {
        match self {
            AnyBoardDatabase::Sqlite(db) => db.close().await,
            AnyBoardDatabase::Mssql(db) => db.close().await,
        }
    }
}

impl ManagedBoardDatabase for AnyBoardDatabase {
    async fn schema_version(&self) -> Result<Option<i32>> {
        match self {
            AnyBoardDatabase::Sqlite(db) => db.schema_version().await,
            AnyBoardDatabase::Mssql(db) => db.schema_version().await,
        }
    }

    async fn schema_features(&self) -> Result<Vec<String>> {
        match self {
            AnyBoardDatabase::Sqlite(db) => db.schema_features().await,
            AnyBoardDatabase::Mssql(db) => db.schema_features().await,
        }
    }

    async fn ensure_schema_features<'a>(&'a self, missing: &'a [String]) -> Result<Vec<String>> {
        match self {
            AnyBoardDatabase::Sqlite(db) => db.ensure_schema_features(missing).await,
            AnyBoardDatabase::Mssql(db) => db.ensure_schema_features(missing).await,
        }
    }

    fn set_missing_features(&self, names: Vec<String>) {
        match self {
            AnyBoardDatabase::Sqlite(db) => db.set_missing_features(names),
            AnyBoardDatabase::Mssql(db) => db.set_missing_features(names),
        }
    }
}

/// Öffnet die Board-Datenbank des in der Konfig gewählten Anbieters.
pub struct BoardDatabaseFactory;

impl BoardDatabaseFactory {
    /// mssql: Verbindung zum SQL Server; sqlite: lokale Datei aus `config.file` (legt Datei und
    /// Schema selbst an)
    pub async fn open(config: &BoardConfig, options: ConnectOptions) -> Result<AnyBoardDatabase> {
        match config.provider {
            BoardProvider::Mssql => Ok(AnyBoardDatabase::Mssql(
                MssqlBoardDatabase::connect(config, options).await?,
            )),
            // options (Verbindungs-Zeitlimit) gilt nur für SQL Server; eine lokale Datei wird
            // ohne Zeitlimit geöffnet.
            BoardProvider::Sqlite => Ok(AnyBoardDatabase::Sqlite(SqliteBoardDatabase::open(
                &config.file,
            )?)),
        }
    }

    /// Der Anbieter zu seiner Schreibweise, für Angaben, die nicht aus `board.yaml` kommen
    /// (Befehlsschalter, HTTP-Aufruf). Ein unbekannter Text ergibt `board-provider-unknown` –
    /// in Rust kann `BoardConfig::provider` selbst keinen unbekannten Anbieter tragen, weil
    /// [`BoardProvider`] ein Enum ist.
    ///
    /// Beim Lesen von `board.yaml` meldet `BoardConfigStore` denselben Fall mit dem Code
    /// `board-config-invalid` und dem Dateinamen im Text (wie in TypeScript).
    pub fn provider(name: &str) -> Result<BoardProvider> {
        BoardProvider::ALL
            .into_iter()
            .find(|provider| provider.as_str() == name)
            .ok_or_else(|| {
                HarnessError::new(
                    "board-provider-unknown",
                    format!("Unbekannter Anbieter der Board-Datenbank „{name}“ – erlaubt sind mssql und sqlite."),
                )
                .into()
            })
    }
}

/// Eine geöffnete und geprüfte Board-Datenbank samt der Erweiterungen, die ihr weiter fehlen
pub struct OpenedBoard<D = AnyBoardDatabase> {
    pub db: D,
    /// Erweiterungen, die auch nach dem Einspielen fehlen; nur Zugriffe darauf verweigern sich
    pub missing_features: Vec<String>,
}

/// Öffnet die Board-Datenbank der Konfig und bringt sie in einen benutzbaren Stand:
/// Schema-Version prüfen, fehlende Erweiterungen einspielen, den Rest merken und den eigenen
/// Benutzer anlegen. Der Datenbank-Teil von `BoardRuntime.connect`.
pub async fn open_checked(
    config: &BoardConfig,
    options: ConnectOptions,
) -> Result<OpenedBoard<AnyBoardDatabase>> {
    open_checked_with(config, options, BoardDatabaseFactory::open).await
}

/// Wie [`open_checked`], aber mit eigenem Öffner – damit Tests ohne echte Datenbank prüfen
/// können (wie `BoardRuntime.connect(…, open)` in TypeScript).
pub async fn open_checked_with<D, O>(
    config: &BoardConfig,
    options: ConnectOptions,
    open: O,
) -> Result<OpenedBoard<D>>
where
    D: ManagedBoardDatabase,
    O: AsyncFnOnce(&BoardConfig, ConnectOptions) -> Result<D>,
{
    let db = open(config, options).await?;
    match prepare(&db, config).await {
        Ok(missing_features) => Ok(OpenedBoard {
            db,
            missing_features,
        }),
        Err(error) => {
            // Eine halb fertige Datenbank nicht offen lassen; der erste Fehler zählt
            let _ = db.close().await;
            Err(error)
        }
    }
}

/// Prüft das Schema, spielt fehlende Erweiterungen ein und legt den eigenen Benutzer an;
/// liefert die Erweiterungen, die danach weiter fehlen
async fn prepare<D: ManagedBoardDatabase>(db: &D, config: &BoardConfig) -> Result<Vec<String>> {
    let database = if config.provider == BoardProvider::Sqlite {
        &config.file
    } else {
        &config.database
    };
    let mut missing = SchemaVersionGuard.check(
        db.schema_version().await?,
        database,
        &db.schema_features().await?,
        config.provider,
    )?;
    // Fehlende Erweiterungen sind additiv: erst selbst einspielen, und was danach bleibt,
    // sperrt nur den Zugriff darauf
    if !missing.is_empty() {
        db.ensure_schema_features(&missing).await?;
        missing = SchemaVersionGuard::missing(&db.schema_features().await?);
        db.set_missing_features(missing.clone());
    }
    let user = BoardUser {
        id: config.user_id.clone(),
        name: config.user_name.clone(),
    };
    db.transaction(
        move |u: &mut dyn BoardUnitOfWork| -> BoxFut<'_, Result<()>> {
            let user = user.clone();
            Box::pin(async move { u.users().upsert(&user).await })
        },
    )
    .await?;
    Ok(missing)
}
