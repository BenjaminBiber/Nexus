//! Board-Datenbank auf SQL Server: ein Verbindungspool, jede Aktion in einer eigenen
//! Transaktion; portiert aus `packages/db/src/MssqlBoardDatabase.ts`.
//!
//! # Anmeldung
//!
//! Wie heute gibt es **nur** die Anmeldung mit SQL-Login: `BoardConfig` kennt `server`, `port`,
//! `database`, `user`, `password`, `encrypt` und `trustServerCertificate`, und
//! `BoardConfigStore` verlangt Benutzer und Passwort. Windows-, NTLM- und Kerberos-Anmeldung
//! sowie benannte Instanzen (`server\INSTANZ`) unterstützt auch die TypeScript-Fassung nicht;
//! sie werden hier bewusst nicht nachgebaut.

use std::sync::RwLock;
use std::time::Duration;

use async_trait::async_trait;
use bb8::{Pool, PooledConnection};
use bb8_tiberius::ConnectionManager;
use nexus_core::board::config::BoardConfig;
use nexus_core::board::database::{BoardDatabase, TransactionWork};
use nexus_core::{Error, HarnessError, Result};
use tiberius::EncryptionLevel;
use tokio::time::timeout;

use crate::managed::{ConnectOptions, ManagedBoardDatabase};
use crate::mssql::error_map::MssqlErrorMap;
use crate::mssql::runner::{MsValue, MssqlRunner, TiberiusRunner, QUERY_TIMEOUT};
use crate::mssql::schema_runner::{MssqlSchemaRunner, SchemaBatchError, SchemaBatchRunner};
use crate::mssql::unit_of_work::MssqlUnitOfWork;

/// Höchstzahl gleichzeitiger Verbindungen; entspricht `pool: { max: 5, min: 0 }` in TypeScript
const POOL_MAX: u32 = 5;

// Stand eines Boards: Ticketzahl (sinkt bei Löschungen) und höchste rowversion (datenbankweit
// steigend) von Tickets, Kommentaren, Notizen und dem Board selbst; dazu Zahl und höchste
// rowversion der Fragen, die Tickets des Boards betreffen (jede Änderung einer Frage erhöht ihre
// Rev und damit ihre rowversion); dazu die Zahl der Session-Verknüpfungen seiner Tickets
const CHANGE_MARKER_SQL: &str = r#"
SELECT
  (SELECT COUNT_BIG(*) FROM board.Tickets WHERE BoardId = @boardId) AS Tickets,
  (SELECT MAX(CONVERT(bigint, RowVer)) FROM board.Tickets WHERE BoardId = @boardId) AS TicketVer,
  (SELECT MAX(CONVERT(bigint, c.RowVer)) FROM board.Comments c JOIN board.Tickets t ON t.Id = c.TicketId WHERE t.BoardId = @boardId) AS CommentVer,
  (SELECT MAX(CONVERT(bigint, RowVer)) FROM board.Notes WHERE BoardId = @boardId) AS NoteVer,
  (SELECT CONVERT(bigint, RowVer) FROM board.Boards WHERE Id = @boardId) AS BoardVer,
  (SELECT COUNT_BIG(*) FROM board.QuestionTickets qt JOIN board.Tickets t ON t.Id = qt.TicketId WHERE t.BoardId = @boardId) AS QuestionLinks,
  (SELECT MAX(CONVERT(bigint, q.RowVer)) FROM board.Questions q
     JOIN board.QuestionTickets qt ON qt.QuestionId = q.Id JOIN board.Tickets t ON t.Id = qt.TicketId
     WHERE t.BoardId = @boardId) AS QuestionVer,
  (SELECT COUNT_BIG(*) FROM board.TicketSessions ts JOIN board.Tickets t ON t.Id = ts.TicketId WHERE t.BoardId = @boardId) AS SessionLinks;"#;

// Stand der Fragen eines Projekts: Anzahl und höchste rowversion
const QUESTION_MARKER_SQL: &str = r#"
SELECT COUNT_BIG(*) AS Questions, MAX(CONVERT(bigint, RowVer)) AS QuestionVer FROM board.Questions WHERE ProjectId = @projectId;"#;

/// Eigene Anweisungen je Zweig: so wird `board.SchemaVersion` nur aufgelöst, wenn die Tabelle
/// existiert
const SCHEMA_VERSION_SQL: &str = "IF OBJECT_ID(N'board.SchemaVersion') IS NULL SELECT CAST(NULL AS int) AS V ELSE SELECT MAX(Version) AS V FROM board.SchemaVersion;";

/// Eingespielte additive Erweiterungen; leer, wenn es die Tabelle noch nicht gibt
const SCHEMA_FEATURES_SQL: &str = "IF OBJECT_ID(N'board.SchemaFeatures') IS NULL SELECT CAST(NULL AS varchar(50)) AS Name WHERE 1 = 0 ELSE SELECT Name FROM board.SchemaFeatures;";

/// Vorspann jeder Transaktion. Das `IF @@TRANCOUNT > 0 ROLLBACK` ist nötig, weil eine
/// abgebrochene Arbeit (Future wird mitten in der Transaktion fallen gelassen) die Verbindung
/// mit offener Transaktion in den Pool zurückgeben kann – der nächste Nutzer würde sonst in
/// deren Sperren laufen.
const BEGIN_SQL: &str = "IF @@TRANCOUNT > 0 ROLLBACK; SET TRANSACTION ISOLATION LEVEL READ COMMITTED; BEGIN TRANSACTION";

/// Board-Datenbank auf SQL Server: ein Verbindungspool, jede Aktion in einer eigenen Transaktion.
pub struct MssqlBoardDatabase {
    /// `None`, sobald geschlossen wurde; bb8 schließt die Verbindungen beim Fallenlassen
    pool: RwLock<Option<Pool<ConnectionManager>>>,
    database: String,
    server: String,
    port: u16,
    /// Erweiterungen, die der DB fehlen und die auch nicht eingespielt werden konnten
    missing_features: RwLock<Vec<String>>,
    query_timeout: Duration,
}

impl MssqlBoardDatabase {
    /// Verbindet mit der DB aus der Konfig; nicht erreichbar ergibt `board-db-unreachable` mit
    /// Server und Datenbank im Text.
    ///
    /// Geprüft wird mit einer eigenen Verbindung außerhalb des Pools: `Pool::get` würde den
    /// wahren Grund („Connection refused“) hinter dem Zeitlimit des Pools verstecken.
    pub async fn connect(config: &BoardConfig, options: ConnectOptions) -> Result<Self> {
        let limit = Duration::from_millis(options.connection_timeout_ms());
        let pool = Pool::builder()
            .max_size(POOL_MAX)
            .min_idle(None)
            .connection_timeout(limit)
            .build_unchecked(ConnectionManager::new(tiberius_config(config)));
        let probe = timeout(limit, pool.dedicated_connection()).await;
        let grund = match probe {
            Ok(Ok(_)) => None,
            Ok(Err(error)) => Some(error.to_string()),
            Err(_) => Some(format!(
                "Zeitlimit von {} ms überschritten",
                limit.as_millis()
            )),
        };
        if let Some(grund) = grund {
            return Err(HarnessError::new(
                "board-db-unreachable",
                format!(
                    "Board-Datenbank {} auf {}:{} ist nicht erreichbar: {grund}",
                    config.database, config.server, config.port
                ),
            )
            .into());
        }
        Ok(Self {
            pool: RwLock::new(Some(pool)),
            database: config.database.clone(),
            server: config.server.clone(),
            port: config.port,
            missing_features: RwLock::new(Vec::new()),
            query_timeout: QUERY_TIMEOUT,
        })
    }

    /// Server und Port, auf die diese Datenbank zeigt (für Diagnoseausgaben)
    pub fn address(&self) -> (&str, u16) {
        (&self.server, self.port)
    }

    /// Der Pool; nach [`BoardDatabase::close`] ist das `board-db-closed`
    fn pool(&self) -> Result<Pool<ConnectionManager>> {
        self.pool
            .read()
            .ok()
            .and_then(|pool| pool.clone())
            .ok_or_else(|| {
                HarnessError::new(
                    "board-db-closed",
                    "Die Board-Datenbank ist bereits geschlossen.",
                )
                .into()
            })
    }

    /// Eine Verbindung aus dem Pool.
    ///
    /// `get_owned` statt `get`: der Pool wird aus dem `RwLock` herausgeklont, die Leihe könnte
    /// also nicht an der geklonten Hülle hängen. Die eigene Fassung hält den Pool selbst fest
    /// und gibt die Verbindung beim Fallenlassen genauso zurück.
    async fn connection(&self) -> Result<PooledConnection<'static, ConnectionManager>> {
        let pool = self.pool()?;
        pool.get_owned().await.map_err(|error| match error {
            bb8::RunError::User(error) => MssqlErrorMap::map_pool(error),
            bb8::RunError::TimedOut => Error::from(HarnessError::new(
                "board-db-unreachable",
                format!(
                    "Board-Datenbank {} auf {}:{} ist nicht erreichbar: keine freie Verbindung im Zeitlimit",
                    self.database, self.server, self.port
                ),
            )),
        })
    }

    /// Die Erweiterungen, die weiter fehlen
    fn missing(&self) -> Vec<String> {
        self.missing_features
            .read()
            .map(|names| names.clone())
            .unwrap_or_default()
    }

    /// Werte als Text; `None` wird zu „-“ (`join` in TypeScript)
    fn join(values: &[Option<i64>]) -> String {
        values
            .iter()
            .map(|value| match value {
                Some(value) => value.to_string(),
                None => "-".to_string(),
            })
            .collect::<Vec<_>>()
            .join(":")
    }
}

/// Die tiberius-Konfig zur Board-Konfig: SQL-Login, Verschlüsselung, Zertifikat.
///
/// `encrypt: false` wird zu [`EncryptionLevel::Off`] – wie bei tedious bleibt dann nur die
/// Anmeldung verschlüsselt. Bei `trust_server_certificate: false` braucht der Server ein
/// gültiges Zertifikat; das ist gewollt.
fn tiberius_config(config: &BoardConfig) -> tiberius::Config {
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
    cfg.application_name("nexus");
    cfg
}

impl BoardDatabase for MssqlBoardDatabase {
    async fn transaction<T, W>(&self, work: W) -> Result<T>
    where
        T: Send + 'static,
        W: TransactionWork<T> + 'static,
    {
        {
            let missing = self.missing();
            let database = self.database.clone();
            let mut conn = self.connection().await?;
            simple(&mut conn, BEGIN_SQL)
                .await
                .map_err(MssqlErrorMap::map)?;
            let ergebnis = {
                let mut runner = TiberiusRunner::with_timeout(&mut conn, self.query_timeout);
                let mut uow = MssqlUnitOfWork::new(&mut runner, missing, database);
                work(&mut uow).await
            };
            match ergebnis {
                Ok(wert) => {
                    simple(&mut conn, "COMMIT TRANSACTION")
                        .await
                        .map_err(MssqlErrorMap::map)?;
                    Ok(wert)
                }
                Err(fehler) => {
                    // Der Rollback kann scheitern, wenn SQL Server die Transaktion schon
                    // abgebrochen hat (XACT_ABORT, Deadlock-Opfer) – dann gilt der erste Fehler
                    let _ = simple(&mut conn, "IF @@TRANCOUNT > 0 ROLLBACK").await;
                    Err(fehler)
                }
            }
        }
    }

    async fn change_marker(&self, board_id: i32) -> Result<String> {
        {
            let mut conn = self.connection().await?;
            let mut runner = TiberiusRunner::with_timeout(&mut conn, self.query_timeout);
            let rows = runner
                .query_rows(CHANGE_MARKER_SQL, &[("boardId", MsValue::Int(board_id))])
                .await?;
            let row = rows.first().ok_or_else(|| {
                Error::unexpected_message("Der Kennwert des Boards liefert keine Zeile.")
            })?;
            let mut werte = Vec::with_capacity(8);
            for column in [
                "Tickets",
                "TicketVer",
                "CommentVer",
                "NoteVer",
                "BoardVer",
                "QuestionLinks",
                "QuestionVer",
                "SessionLinks",
            ] {
                werte.push(row.get_opt_i64(column)?);
            }
            Ok(Self::join(&werte))
        }
    }

    async fn question_marker(&self, project_id: i32) -> Result<String> {
        {
            let mut conn = self.connection().await?;
            let mut runner = TiberiusRunner::with_timeout(&mut conn, self.query_timeout);
            let rows = runner
                .query_rows(
                    QUESTION_MARKER_SQL,
                    &[("projectId", MsValue::Int(project_id))],
                )
                .await?;
            let row = rows.first().ok_or_else(|| {
                Error::unexpected_message("Der Kennwert der Fragen liefert keine Zeile.")
            })?;
            Ok(Self::join(&[
                row.get_opt_i64("Questions")?,
                row.get_opt_i64("QuestionVer")?,
            ]))
        }
    }

    async fn close(&self) -> Result<()> {
        {
            // Den Pool fallen lassen schließt die Verbindungen; mehrfaches Schließen ist erlaubt
            if let Ok(mut pool) = self.pool.write() {
                pool.take();
            }
            Ok(())
        }
    }
}

impl ManagedBoardDatabase for MssqlBoardDatabase {
    async fn schema_version(&self) -> Result<Option<i32>> {
        {
            let mut conn = self.connection().await?;
            let mut runner = TiberiusRunner::with_timeout(&mut conn, self.query_timeout);
            let rows = runner.query_rows(SCHEMA_VERSION_SQL, &[]).await?;
            match rows.first() {
                Some(row) => row.get_opt_i32("V"),
                None => Ok(None),
            }
        }
    }

    async fn schema_features(&self) -> Result<Vec<String>> {
        {
            let mut conn = self.connection().await?;
            let mut runner = TiberiusRunner::with_timeout(&mut conn, self.query_timeout);
            let rows = runner.query_rows(SCHEMA_FEATURES_SQL, &[]).await?;
            rows.iter().map(|row| row.get_str("Name")).collect()
        }
    }

    async fn ensure_schema_features<'a>(&'a self, missing: &'a [String]) -> Result<Vec<String>> {
        {
            let pool = self.pool()?;
            let mut runner = MssqlSchemaRunner::new(PoolBatchRunner { pool });
            Ok(runner.apply(missing).await)
        }
    }

    fn set_missing_features(&self, names: Vec<String>) {
        if let Ok(mut missing) = self.missing_features.write() {
            *missing = names;
        }
    }
}

/// Setzt einen ganzen Batch auf dieser Verbindung ab und verwirft die Ergebnismengen.
/// Die Ergebnismengen müssen abgeholt werden, sonst bleibt die Leihe der Verbindung offen.
async fn simple(
    conn: &mut PooledConnection<'_, ConnectionManager>,
    sql: &str,
) -> std::result::Result<(), tiberius::error::Error> {
    conn.simple_query(sql).await?.into_results().await?;
    Ok(())
}

/// Schema-Batches über dem Pool. Jeder Batch bekommt eine eigene Verbindung: schlägt ein Skript
/// fehl, kann die Verbindung eine halb abgebrochene Transaktion tragen.
struct PoolBatchRunner {
    pool: Pool<ConnectionManager>,
}

#[async_trait]
impl SchemaBatchRunner for PoolBatchRunner {
    async fn batch(&mut self, sql: &str) -> std::result::Result<(), SchemaBatchError> {
        let mut conn = self.pool.get().await.map_err(|error| match error {
            bb8::RunError::User(bb8_tiberius::Error::Tiberius(error)) => {
                SchemaBatchError::from(error)
            }
            bb8::RunError::User(bb8_tiberius::Error::Io(error)) => {
                SchemaBatchError::plain(error.to_string())
            }
            bb8::RunError::TimedOut => {
                SchemaBatchError::plain("keine freie Verbindung im Zeitlimit")
            }
        })?;
        simple(&mut conn, sql).await.map_err(SchemaBatchError::from)
    }
}
