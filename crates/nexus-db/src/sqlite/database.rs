//! Board-Datenbank in einer SQLite-Datei; portiert aus
//! `packages/db/src/sqlite/SqliteBoardDatabase.ts`.
//!
//! Eine Verbindung je Instanz, und alle Zugriffe auf dieselbe Datei laufen im Prozess
//! nacheinander: SQLite wartet auf eine gesperrte Datei, zwei Instanzen würden sich sonst
//! gegenseitig `SQLITE_BUSY` melden.
//!
//! # Send-Grenzen von rusqlite
//!
//! `rusqlite::Connection` ist `Send`, aber nicht `Sync`; ein `rusqlite::Statement` ist weder das
//! eine noch das andere. Über ein `.await` darf deshalb nur `&mut Connection` gehalten werden –
//! die Repositories arbeiten innerhalb ihrer `async`-Methoden synchron.

use std::collections::HashMap;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::{Arc, LazyLock, Mutex as StdMutex};

use rusqlite::functions::FunctionFlags;
use rusqlite::types::{Value, ValueRef};
use rusqlite::Connection;
use tokio::sync::Mutex;

use nexus_core::board::database::{BoardDatabase, BoardUnitOfWork, TransactionWork};
use nexus_core::{Error, HarnessError, Result};

use super::error_map;
use super::runner::{self, SqliteRunner};
use super::schema_runner::SqliteSchemaRunner;
use super::unit_of_work::SqliteUnitOfWork;
use crate::managed::ManagedBoardDatabase;

/// Stand eines Boards: Ticketzahl und höchster RowVer-Zähler (Trigger, datenbankweit steigend) von Tickets,
/// Kommentaren, Notizen und dem Board selbst; dazu Zahl und höchster Zähler der Fragen zu seinen Tickets
/// sowie die Zahl der Session-Verknüpfungen seiner Tickets. Übersetzung von MssqlBoardDatabase (ohne board.).
const CHANGE_MARKER_SQL: &str = "
SELECT
  (SELECT COUNT(*) FROM Tickets WHERE BoardId = @boardId) AS Tickets,
  (SELECT MAX(RowVer) FROM Tickets WHERE BoardId = @boardId) AS TicketVer,
  (SELECT MAX(c.RowVer) FROM Comments c JOIN Tickets t ON t.Id = c.TicketId WHERE t.BoardId = @boardId) AS CommentVer,
  (SELECT MAX(RowVer) FROM Notes WHERE BoardId = @boardId) AS NoteVer,
  (SELECT RowVer FROM Boards WHERE Id = @boardId) AS BoardVer,
  (SELECT COUNT(*) FROM QuestionTickets qt JOIN Tickets t ON t.Id = qt.TicketId WHERE t.BoardId = @boardId) AS QuestionLinks,
  (SELECT MAX(q.RowVer) FROM Questions q
     JOIN QuestionTickets qt ON qt.QuestionId = q.Id JOIN Tickets t ON t.Id = qt.TicketId
     WHERE t.BoardId = @boardId) AS QuestionVer,
  (SELECT COUNT(*) FROM TicketSessions ts JOIN Tickets t ON t.Id = ts.TicketId WHERE t.BoardId = @boardId) AS SessionLinks";

/// Stand der Fragen eines Projekts: Anzahl und höchster Zähler
const QUESTION_MARKER_SQL: &str =
    "SELECT COUNT(*) AS Questions, MAX(RowVer) AS QuestionVer FROM Questions WHERE ProjectId = @projectId";

/// Name der Funktion, die die Suche zum Kleinschreiben benutzt (`SqliteSearchRepository`)
const SEARCH_LOWER: &str = "harness_lower";

/// Zeitlimit, das SQLite auf eine gesperrte Datei wartet, bevor es `SQLITE_BUSY` meldet
const BUSY_TIMEOUT_MS: i32 = 10000;

/// Warteschlange je Datei und Prozess: Schlüssel ist der absolute Pfad (unter Windows
/// kleingeschrieben, weil Pfade dort ohne Groß-/Kleinschreibung gelten)
static QUEUES: LazyLock<StdMutex<HashMap<PathBuf, Arc<Mutex<()>>>>> =
    LazyLock::new(|| StdMutex::new(HashMap::new()));

/// Ein abgelegtes Future mit Lebensdauer – die Form, die eine Arbeit mit dem rohen Runner liefert
pub type SqliteFut<'a, T> = Pin<Box<dyn Future<Output = Result<T>> + Send + 'a>>;

/// Die Arbeit einer Transaktion auf dem rohen [`SqliteRunner`]. `Fn` statt `FnOnce`, weil die
/// Arbeit bei einem Schreibkonflikt genau einmal wiederholt wird – sie darf außerhalb der
/// Datenbank keine Seiteneffekte haben.
pub trait SqliteWork<T>:
    for<'a> Fn(&'a mut SqliteRunner<'_>) -> SqliteFut<'a, T> + Send + Sync
{
}

impl<T, F> SqliteWork<T> for F where
    F: for<'a> Fn(&'a mut SqliteRunner<'_>) -> SqliteFut<'a, T> + Send + Sync
{
}

/// Board-Datenbank in einer SQLite-Datei
pub struct SqliteBoardDatabase {
    /// Pfad der Datei, wie der Aufrufer ihn angegeben hat (steht in Fehlermeldungen)
    file: String,
    /// Gemeinsame Warteschlange aller Instanzen auf dieser Datei
    queue: Arc<Mutex<()>>,
    /// Die eigene Verbindung; `None` nach [`SqliteBoardDatabase::close`]
    conn: Mutex<Option<Connection>>,
}

impl SqliteBoardDatabase {
    /// Öffnet (oder erzeugt) die Datei samt fehlendem Ordner, setzt die PRAGMAs und legt
    /// fehlende Schema-Stufen an
    pub fn open(file: &str) -> Result<Self> {
        let conn = Self::connect(file)?;
        if let Err(error) = Self::prepare(&conn) {
            // Die halb fertige Verbindung nicht offen lassen
            drop(conn);
            return Err(error_map::map(error));
        }
        Ok(Self {
            file: file.to_string(),
            queue: queue_of(file),
            conn: Mutex::new(Some(conn)),
        })
    }

    /// Der Pfad der Datei
    pub fn file(&self) -> &str {
        &self.file
    }

    /// Öffnet die Datei und legt den fehlenden Ordner an; jeder Fehler ist
    /// `board-db-unreachable`
    fn connect(file: &str) -> Result<Connection> {
        if file != ":memory:" {
            if let Some(ordner) = Path::new(file).parent().filter(|p| !p.as_os_str().is_empty()) {
                std::fs::create_dir_all(ordner).map_err(|error| unreachable(file, &error))?;
            }
        }
        Connection::open(file).map_err(|error| unreachable(file, &error))
    }

    /// PRAGMAs, die Suchfunktion und die Schema-Stufen – in dieser Reihenfolge.
    ///
    /// `busy_timeout` zuerst: das Umschalten auf WAL und die Schema-Stufen brauchen eine
    /// exklusive Sperre und würden sonst sofort mit `SQLITE_BUSY` scheitern, wenn ein anderer
    /// Prozess die Datei gerade schreibt.
    fn prepare(conn: &Connection) -> Result<()> {
        pragma(conn, &format!("PRAGMA busy_timeout = {BUSY_TIMEOUT_MS};"))?;
        pragma(conn, "PRAGMA journal_mode = WAL;")?;
        pragma(conn, "PRAGMA foreign_keys = ON;")?;
        register_search_lower(conn)?;
        SqliteSchemaRunner::new(conn).run()?;
        Ok(())
    }

    /// Führt Arbeit mit dem rohen [`SqliteRunner`] in einer Transaktion aus (Grundlage von
    /// [`BoardDatabase::transaction`], auch für Tests).
    ///
    /// Erst aufgeschoben (`BEGIN`): reine Lesezugriffe nehmen so keine Schreibsperre und warten
    /// nie auf einen anderen Prozess. Will die Arbeit danach schreiben, während ein anderer
    /// Prozess die Datei hält oder schon weitergeschrieben hat, meldet SQLite sofort
    /// `SQLITE_BUSY`; dann läuft sie genau einmal neu, diesmal mit sofortiger Schreibsperre
    /// (`BEGIN IMMEDIATE`).
    pub async fn run_in_transaction<T, W>(&self, work: W) -> Result<T>
    where
        T: Send + 'static,
        W: SqliteWork<T>,
    {
        let _queue = self.queue.lock().await;
        let mut guard = self.conn.lock().await;
        let conn = guard.as_mut().ok_or_else(closed)?;
        match Self::attempt_runner(conn, "BEGIN", &work).await {
            Ok(wert) => Ok(wert),
            Err(error) if error_map::is_sqlite_busy(&error) => {
                Self::attempt_runner(conn, "BEGIN IMMEDIATE", &work)
                    .await
                    .map_err(error_map::map)
            }
            Err(error) => Err(error_map::map(error)),
        }
    }

    /// Ein Durchlauf der Arbeit auf dem rohen Runner
    async fn attempt_runner<T, W>(conn: &mut Connection, begin: &str, work: &W) -> Result<T>
    where
        W: SqliteWork<T>,
    {
        conn.execute_batch(begin).map_err(error_map::wrap)?;
        let ergebnis = {
            let mut runner = SqliteRunner::new(&mut *conn);
            work(&mut runner).await
        };
        finish(conn, ergebnis)
    }

    /// Ein Durchlauf der Arbeit auf der Unit of Work
    async fn attempt_uow<T, W>(conn: &mut Connection, begin: &str, work: &W) -> Result<T>
    where
        W: TransactionWork<T>,
    {
        conn.execute_batch(begin).map_err(error_map::wrap)?;
        let ergebnis = {
            let mut uow = SqliteUnitOfWork::new(&mut *conn);
            work(&mut uow as &mut dyn BoardUnitOfWork).await
        };
        finish(conn, ergebnis)
    }

    /// Einen Zugriff ohne eigene Transaktion ausführen; wartet auf die Warteschlange der Datei
    async fn with_connection<T, F>(&self, action: F) -> Result<T>
    where
        F: FnOnce(&mut Connection) -> Result<T>,
    {
        let _queue = self.queue.lock().await;
        let mut guard = self.conn.lock().await;
        let conn = guard.as_mut().ok_or_else(closed)?;
        action(conn)
    }

    /// Kennwerte als Text; `None` wird zu „-“
    fn join(values: &[Option<i64>]) -> String {
        values
            .iter()
            .map(|v| v.map_or_else(|| "-".to_string(), |n| n.to_string()))
            .collect::<Vec<_>>()
            .join(":")
    }
}

impl BoardDatabase for SqliteBoardDatabase {
    /// Jede fachliche Aktion in genau einer Transaktion; jeder Fehler rollt alles zurück
    async fn transaction<'a, T, W>(&'a self, work: W) -> Result<T>
    where
        T: Send + 'static,
        W: TransactionWork<T> + 'a,
    {
        let _queue = self.queue.lock().await;
        let mut guard = self.conn.lock().await;
        let conn = guard.as_mut().ok_or_else(closed)?;
        match Self::attempt_uow(conn, "BEGIN", &work).await {
            Ok(wert) => Ok(wert),
            Err(error) if error_map::is_sqlite_busy(&error) => {
                Self::attempt_uow(conn, "BEGIN IMMEDIATE", &work)
                    .await
                    .map_err(error_map::map)
            }
            Err(error) => Err(error_map::map(error)),
        }
    }

    /// Kennwert für den Stand eines Boards (liest nur, außerhalb einer Transaktion)
    async fn change_marker(&self, board_id: i32) -> Result<String> {
        self.with_connection(move |conn| {
            let mut runner = SqliteRunner::new(conn);
            let zeilen = runner.query_rows(
                CHANGE_MARKER_SQL,
                &crate::sqlite_params! { "boardId": board_id },
                |row| {
                    Ok([
                        row.get::<_, Option<i64>>(0)?,
                        row.get::<_, Option<i64>>(1)?,
                        row.get::<_, Option<i64>>(2)?,
                        row.get::<_, Option<i64>>(3)?,
                        row.get::<_, Option<i64>>(4)?,
                        row.get::<_, Option<i64>>(5)?,
                        row.get::<_, Option<i64>>(6)?,
                        row.get::<_, Option<i64>>(7)?,
                    ])
                },
            )?;
            let werte = zeilen
                .into_iter()
                .next()
                .ok_or_else(|| runner::missing_row("den Stand des Boards"))?;
            Ok(Self::join(&werte))
        })
        .await
    }

    /// Kennwert für den Stand der Fragen eines Projekts (liest nur, außerhalb einer Transaktion)
    async fn question_marker(&self, project_id: i32) -> Result<String> {
        self.with_connection(move |conn| {
            let mut runner = SqliteRunner::new(conn);
            let zeilen = runner.query_rows(
                QUESTION_MARKER_SQL,
                &crate::sqlite_params! { "projectId": project_id },
                |row| Ok([row.get::<_, Option<i64>>(0)?, row.get::<_, Option<i64>>(1)?]),
            )?;
            let werte = zeilen
                .into_iter()
                .next()
                .ok_or_else(|| runner::missing_row("den Stand der Fragen"))?;
            Ok(Self::join(&werte))
        })
        .await
    }

    /// Wartet auf laufende Aktionen und schließt die Verbindung; mehrfacher Aufruf schadet nicht
    async fn close(&self) -> Result<()> {
        let _queue = self.queue.lock().await;
        let mut guard = self.conn.lock().await;
        if let Some(conn) = guard.take() {
            // Ein Fehler beim Schließen ist nichts, was der Aufrufer noch abstellen könnte
            drop(conn.close());
        }
        Ok(())
    }
}

impl ManagedBoardDatabase for SqliteBoardDatabase {
    /// Höchste eingespielte Schema-Version; `None`, solange es `SchemaVersion` nicht gibt
    async fn schema_version(&self) -> Result<Option<i32>> {
        self.with_connection(|conn| SqliteSchemaRunner::new(conn).schema_version())
            .await
    }

    /// Eingespielte additive Erweiterungen; leer, solange es `SchemaFeatures` nicht gibt
    async fn schema_features(&self) -> Result<Vec<String>> {
        self.with_connection(|conn| SqliteSchemaRunner::new(conn).schema_features())
            .await
    }

    /// Nichts zu tun: der Schema-Runner hat beim Öffnen schon alle Stufen und Erweiterungen
    /// angelegt
    async fn ensure_schema_features(&self, _missing: &[String]) -> Result<Vec<String>> {
        Ok(Vec::new())
    }

    /// Nichts zu merken: bei SQLite kann nach dem Öffnen keine Erweiterung fehlen
    fn set_missing_features(&self, _names: Vec<String>) {}
}

/// Bestätigt die Transaktion bzw. rollt sie zurück und gibt das Ergebnis der Arbeit weiter
fn finish<T>(conn: &mut Connection, result: Result<T>) -> Result<T> {
    match result {
        Ok(wert) => {
            conn.execute_batch("COMMIT").map_err(error_map::wrap)?;
            Ok(wert)
        }
        Err(error) => {
            // Rollback nur, wenn SQLite die Transaktion nicht bei dem Fehler selbst beendet hat
            if !conn.is_autocommit() {
                drop(conn.execute_batch("ROLLBACK"));
            }
            Err(error)
        }
    }
}

/// Die gemeinsame Warteschlange der Datei; der erste Aufrufer legt sie an
fn queue_of(file: &str) -> Arc<Mutex<()>> {
    let key = queue_key(file);
    let mut queues = QUEUES.lock().unwrap_or_else(|poison| poison.into_inner());
    Arc::clone(queues.entry(key).or_default())
}

/// Schlüssel der Warteschlange; unter Windows zählt Groß-/Kleinschreibung des Pfades nicht
fn queue_key(file: &str) -> PathBuf {
    if file == ":memory:" {
        return PathBuf::from(file);
    }
    let full = std::path::absolute(file).unwrap_or_else(|_| PathBuf::from(file));
    if cfg!(windows) {
        PathBuf::from(full.to_string_lossy().to_lowercase())
    } else {
        full
    }
}

/// Setzt ein PRAGMA; `journal_mode` und `busy_timeout` liefern dabei eine Ergebniszeile,
/// `foreign_keys` keine
fn pragma(conn: &Connection, sql: &str) -> Result<()> {
    match conn.query_row(sql, [], |_| Ok(())) {
        Ok(()) | Err(rusqlite::Error::QueryReturnedNoRows) => Ok(()),
        Err(error) => Err(error_map::wrap(error)),
    }
}

/// Meldet `harness_lower` an der Verbindung an; die Suche braucht sie in jeder Abfrage
fn register_search_lower(conn: &Connection) -> Result<()> {
    conn.create_scalar_function(
        SEARCH_LOWER,
        1,
        FunctionFlags::SQLITE_UTF8 | FunctionFlags::SQLITE_DETERMINISTIC,
        |ctx| {
            Ok(match ctx.get_raw(0) {
                ValueRef::Text(text) => {
                    Value::Text(String::from_utf8_lossy(text).to_lowercase())
                }
                // Alles, was kein Text ist, bleibt unverändert (wie `typeof value === 'string'`)
                ValueRef::Null => Value::Null,
                ValueRef::Integer(zahl) => Value::Integer(zahl),
                ValueRef::Real(zahl) => Value::Real(zahl),
                ValueRef::Blob(bytes) => Value::Blob(bytes.to_vec()),
            })
        },
    )
    .map_err(error_map::wrap)
}

/// Die Datei lässt sich nicht öffnen
fn unreachable(file: &str, error: &dyn std::error::Error) -> Error {
    HarnessError::new(
        "board-db-unreachable",
        format!("Board-Datei {file} lässt sich nicht öffnen: {error}"),
    )
    .into()
}

/// Die Datenbank ist schon geschlossen
fn closed() -> Error {
    HarnessError::new(
        "board-db-closed",
        "Die Board-Datenbank ist bereits geschlossen.",
    )
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn der_schlussel_der_warteschlange_ist_der_absolute_pfad() {
        assert_eq!(queue_key(":memory:"), PathBuf::from(":memory:"));
        let key = queue_key("board.db");
        assert!(key.is_absolute(), "{key:?}");
        // Zwei Schreibweisen derselben Datei teilen eine Warteschlange
        assert_eq!(queue_key("./board.db"), key);
        assert!(Arc::ptr_eq(&queue_of("./board.db"), &queue_of("board.db")));
    }

    #[test]
    fn kennwerte_schreiben_null_als_strich() {
        assert_eq!(
            SqliteBoardDatabase::join(&[Some(0), None, Some(7)]),
            "0:-:7"
        );
    }

    #[test]
    fn das_sql_der_kennwerte_bleibt_wortgleich() {
        // Beide Texte stehen so in MssqlBoardDatabase/SqliteBoardDatabase.ts
        assert!(CHANGE_MARKER_SQL.contains("AS SessionLinks"));
        assert!(CHANGE_MARKER_SQL.starts_with("\nSELECT\n"));
        assert!(QUESTION_MARKER_SQL.starts_with("SELECT COUNT(*) AS Questions"));
    }
}
