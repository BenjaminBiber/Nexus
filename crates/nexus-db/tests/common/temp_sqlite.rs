#![allow(dead_code)]
//! Temporäre SQLite-Board-Datei je Test: anlegen, befüllen, wegräumen; portiert aus
//! `packages/db/tests/sqlite/helpers/TempSqlite.ts`.
//!
//! Befüllt wird mit rohem SQL, damit P2-6 bis P2-9 ihre Repositories ohne die jeweils anderen
//! testen können.

use std::path::Path;
use std::sync::Arc;

use nexus_core::board::database::BoardDatabase;
use nexus_core::Result;
use nexus_db::sqlite::{Params, SqliteBoardDatabase};
use nexus_db::sqlite_params;
use rusqlite::{Connection, Row};
use tempfile::TempDir;

/// Der Benutzer, den die Helfer anlegen
pub const TEST_USER_ID: &str = "11111111-1111-1111-1111-111111111111";
/// Sein Name
pub const TEST_USER_NAME: &str = "Test";
/// Ein fester Zeitpunkt, damit Tests nicht von der Uhr abhängen
pub const TEST_NOW: &str = "2026-10-13T10:00:00.000Z";

/// Nutzer, Projekt und Board, die [`TempSqlite::seed_board`] angelegt hat
pub struct SeededBoard {
    pub user_id: String,
    pub project_id: i32,
    pub board_id: i32,
}

/// Mindestangaben eines Tickets; der Rest kommt aus den Spalten-Defaults
pub struct SeedTicket {
    pub id: String,
    pub project_id: i32,
    pub board_id: i32,
    pub number: i32,
    pub title: String,
    pub status: String,
    pub sort_order: i32,
}

impl SeedTicket {
    /// Ticket mit Titel „Titel“, Status `backlog` und Sortierung 0
    pub fn new(id: &str, board: &SeededBoard, number: i32) -> Self {
        Self {
            id: id.to_string(),
            project_id: board.project_id,
            board_id: board.board_id,
            number,
            title: "Titel".to_string(),
            status: "backlog".to_string(),
            sort_order: 0,
        }
    }

    /// Dasselbe Ticket mit einem anderen Status (auch einem ungültigen, für Fehlerfälle)
    pub fn with_status(mut self, status: &str) -> Self {
        self.status = status.to_string();
        self
    }

    /// Dasselbe Ticket in einem anderen Board (auch einem, das es nicht gibt)
    pub fn with_board(mut self, board_id: i32) -> Self {
        self.board_id = board_id;
        self
    }
}

/// Board-Datei in einem temporären Ordner; der Ordner verschwindet mit dem Helfer
pub struct TempSqlite {
    dir: TempDir,
    file: String,
    db: SqliteBoardDatabase,
}

impl TempSqlite {
    /// Neuer Ordner unter dem Temp-Verzeichnis, Datei `board.db`, Schema angelegt
    pub fn create() -> Result<Self> {
        let dir = TempDir::with_prefix("harness-sqlite-").map_err(nexus_core::Error::unexpected)?;
        let file = dir.path().join("board.db").to_string_lossy().to_string();
        let db = SqliteBoardDatabase::open(&file)?;
        Ok(Self { dir, file, db })
    }

    /// Die geöffnete Board-Datenbank
    pub fn db(&self) -> &SqliteBoardDatabase {
        &self.db
    }

    /// Der Pfad der Datei
    pub fn file(&self) -> &str {
        &self.file
    }

    /// Der temporäre Ordner, in dem die Datei liegt
    pub fn dir(&self) -> &Path {
        self.dir.path()
    }

    /// Rohes SQL in eigener Transaktion (genau eine Anweisung); liefert die geänderten Zeilen
    pub async fn execute(&self, sql: &str, params: Params) -> Result<usize> {
        let sql = sql.to_string();
        self.db
            .run_in_transaction(move |runner| {
                let sql = sql.clone();
                let params = params.clone();
                Box::pin(async move { runner.execute(&sql, &params) })
            })
            .await
    }

    /// Zeilen einer Abfrage in eigener Transaktion (genau eine Anweisung)
    pub async fn rows<T, F>(&self, sql: &str, params: Params, map: F) -> Result<Vec<T>>
    where
        T: Send + 'static,
        F: Fn(&Row<'_>) -> rusqlite::Result<T> + Send + Sync + 'static,
    {
        let sql = sql.to_string();
        let map = Arc::new(map);
        self.db
            .run_in_transaction(move |runner| {
                let sql = sql.clone();
                let params = params.clone();
                let map = Arc::clone(&map);
                Box::pin(async move { runner.query_rows(&sql, &params, |row| map(row)) })
            })
            .await
    }

    /// Die erste Spalte der ersten Zeile als ganze Zahl (`None`, wenn die Spalte NULL ist)
    pub async fn number(&self, sql: &str, params: Params) -> Result<Option<i64>> {
        let zeilen = self
            .rows(sql, params, |row| row.get::<_, Option<i64>>(0))
            .await?;
        Ok(zeilen.into_iter().next().flatten())
    }

    /// Die erste Spalte der ersten Zeile als Text
    pub async fn text(&self, sql: &str, params: Params) -> Result<Option<String>> {
        let zeilen = self
            .rows(sql, params, |row| row.get::<_, Option<String>>(0))
            .await?;
        Ok(zeilen.into_iter().next().flatten())
    }

    /// Zahl der Zeilen einer Tabelle
    pub async fn count(&self, table: &str) -> Result<i64> {
        let sql = format!("SELECT COUNT(*) AS N FROM {table}");
        Ok(self.number(&sql, Vec::new()).await?.unwrap_or(0))
    }

    /// Legt den Test-Benutzer an, falls es ihn noch nicht gibt; liefert seine Id
    pub async fn seed_user(&self) -> Result<String> {
        self.execute(
            "INSERT INTO Users (Id, Name) VALUES (@id, @name) ON CONFLICT(Id) DO NOTHING",
            sqlite_params! { "id": TEST_USER_ID, "name": TEST_USER_NAME },
        )
        .await?;
        Ok(TEST_USER_ID.to_string())
    }

    /// Legt den Test-Benutzer und ein Projekt an; liefert die Projekt-Id
    pub async fn seed_project(&self, key: &str) -> Result<i32> {
        let user_id = self.seed_user().await?;
        let id = self
            .number(
                "INSERT INTO Projects (\"Key\", Name, CreatedAt, CreatedBy) VALUES (@key, @name, @createdAt, @createdBy) RETURNING Id",
                sqlite_params! {
                    "key": key,
                    "name": format!("Projekt {key}"),
                    "createdAt": TEST_NOW,
                    "createdBy": user_id,
                },
            )
            .await?;
        Ok(i32::try_from(id.unwrap_or_default()).unwrap_or_default())
    }

    /// Legt ein Board an (ohne Projekt-Id zuerst Benutzer und Projekt `ARC`)
    pub async fn seed_board(&self, project_id: Option<i32>, name: &str) -> Result<SeededBoard> {
        let project_id = match project_id {
            Some(id) => id,
            None => self.seed_project("ARC").await?,
        };
        let user_id = self.seed_user().await?;
        let id = self
            .number(
                "INSERT INTO Boards (ProjectId, OwnerId, Name, RepoPath, CreatedAt) VALUES (@projectId, @ownerId, @name, NULL, @createdAt) RETURNING Id",
                sqlite_params! {
                    "projectId": project_id,
                    "ownerId": user_id.clone(),
                    "name": name,
                    "createdAt": TEST_NOW,
                },
            )
            .await?;
        Ok(SeededBoard {
            user_id,
            project_id,
            board_id: i32::try_from(id.unwrap_or_default()).unwrap_or_default(),
        })
    }

    /// Legt ein Ticket mit Mindestfeldern an; Ersteller ist der Test-Benutzer
    pub async fn seed_ticket(&self, ticket: &SeedTicket) -> Result<()> {
        self.execute(
            "INSERT INTO Tickets (Id, ProjectId, BoardId, Number, Title, Status, Priority, SortOrder, NeedsPlanning,
               CreatedAt, CreatedById, CreatedByName, UpdatedAt, Rev)
             VALUES (@id, @projectId, @boardId, @number, @title, @status, 'normal', @sortOrder, 0,
               @now, @userId, @userName, @now, 1)",
            sqlite_params! {
                "id": ticket.id.clone(),
                "projectId": ticket.project_id,
                "boardId": ticket.board_id,
                "number": ticket.number,
                "title": ticket.title.clone(),
                "status": ticket.status.clone(),
                "sortOrder": ticket.sort_order,
                "now": TEST_NOW,
                "userId": TEST_USER_ID,
                "userName": TEST_USER_NAME,
            },
        )
        .await?;
        Ok(())
    }

    /// Zweite, rohe Verbindung auf dieselbe Datei – für Tests, die einen anderen Prozess
    /// nachstellen
    pub fn raw(&self) -> Connection {
        let conn = Connection::open(&self.file).expect("Roh-Verbindung");
        conn.query_row("PRAGMA busy_timeout = 10000;", [], |_| Ok(()))
            .expect("busy_timeout");
        conn.execute_batch("PRAGMA foreign_keys = ON;")
            .expect("foreign_keys");
        conn
    }

    /// Schließt die Datenbank und löscht den Ordner
    pub async fn dispose(self) {
        let _ = self.db.close().await;
        drop(self.dir);
    }
}
