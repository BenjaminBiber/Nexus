#![allow(dead_code)]
//! Frische SQLite-Datei mit den angegebenen Schema-Stufen und Lesehilfen für Struktur-Tests;
//! portiert aus `packages/db/tests/sqlite/helpers/SqliteSchemaInspect.ts`.

use std::collections::BTreeMap;

use nexus_db::sqlite::{SqliteSchemaRunner, SqliteSchemaStep};
use rusqlite::{Connection, Row};
use tempfile::TempDir;

/// Ein fester Zeitpunkt für die Testdaten
pub const NOW: &str = "2026-10-21T08:00:00.000Z";
/// Der Benutzer der Testdaten
pub const USER: &str = "11111111-1111-1111-1111-111111111111";

/// Fremdschlüssel einer Spalte, wie `PRAGMA foreign_key_list` ihn meldet
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForeignKey {
    pub from: String,
    pub table: String,
    pub to: String,
    pub on_delete: String,
}

impl ForeignKey {
    /// Kurzform „Spalte:Kaskade“, wie die Tests sie vergleichen
    pub fn short(&self) -> String {
        format!("{}:{}", self.from, self.on_delete)
    }
}

/// Eine Spalte mit Typ, NOT NULL (0/1) und Default-Ausdruck
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColumnInfo {
    pub typ: String,
    pub notnull: i64,
    pub dflt_value: Option<String>,
}

/// Temporäre Datei mit eingespielten Stufen; die Datei verschwindet mit dem Helfer
pub struct SchemaInspect {
    dir: TempDir,
    file: String,
    conn: Connection,
    steps: Vec<SqliteSchemaStep>,
}

impl SchemaInspect {
    /// Legt eine Temp-Datei an und spielt die Stufen ein
    pub fn fresh(steps: &[SqliteSchemaStep]) -> Self {
        let dir = TempDir::with_prefix("harness-sqlite-").expect("Temp-Ordner");
        let file = dir.path().join("board.db").to_string_lossy().to_string();
        let conn = Connection::open(&file).expect("Datei öffnen");
        let inspect = Self {
            dir,
            file,
            conn,
            steps: steps.to_vec(),
        };
        inspect.runner().run().expect("Schema-Stufen");
        inspect
    }

    /// Der Runner über derselben Verbindung und Stufenliste
    pub fn runner(&self) -> SqliteSchemaRunner<'_> {
        SqliteSchemaRunner::with_steps(&self.conn, &self.steps)
    }

    /// Die offene Verbindung
    pub fn connection(&mut self) -> &mut Connection {
        &mut self.conn
    }

    /// Der Pfad der Datei
    pub fn file(&self) -> &str {
        &self.file
    }

    /// Führt eine Anweisung aus; der Fehler kommt roh zurück, damit Tests seinen Text prüfen
    pub fn run(&self, sql: &str) -> rusqlite::Result<usize> {
        self.conn.execute(sql, [])
    }

    /// Führt eine Anweisung mit positionellen Parametern (`?1`, `?2` …) aus
    pub fn run_with(&self, sql: &str, params: &[&dyn rusqlite::ToSql]) -> rusqlite::Result<usize> {
        self.conn.execute(sql, params)
    }

    /// Zeilen einer Abfrage
    pub fn rows<T, F>(&self, sql: &str, params: &[&dyn rusqlite::ToSql], map: F) -> Vec<T>
    where
        F: FnMut(&Row<'_>) -> rusqlite::Result<T>,
    {
        let mut statement = self.conn.prepare(sql).expect("prepare");
        let zeilen = statement
            .query_map(params, map)
            .expect("query")
            .collect::<rusqlite::Result<Vec<T>>>();
        zeilen.expect("Zeilen")
    }

    /// Die erste Spalte aller Zeilen als Text
    fn texts(&self, sql: &str, params: &[&dyn rusqlite::ToSql]) -> Vec<String> {
        self.rows(sql, params, |row| row.get::<_, String>(0))
    }

    /// Die erste Spalte der ersten Zeile als ganze Zahl
    pub fn number(&self, sql: &str) -> Option<i64> {
        self.rows(sql, &[], |row| row.get::<_, Option<i64>>(0))
            .into_iter()
            .next()
            .flatten()
    }

    /// Die erste Spalte der ersten Zeile als Text
    pub fn text(&self, sql: &str) -> Option<String> {
        self.rows(sql, &[], |row| row.get::<_, Option<String>>(0))
            .into_iter()
            .next()
            .flatten()
    }

    /// Zahl der Zeilen einer Tabelle
    pub fn count(&self, table: &str) -> i64 {
        self.number(&format!("SELECT COUNT(*) AS N FROM {table}"))
            .unwrap_or(0)
    }

    /// Tabellen ohne interne (`sqlite_*`) und ohne die Buchführung des Runners
    pub fn tables(&self) -> Vec<String> {
        self.texts(
            "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' AND name <> 'SchemaSteps' ORDER BY name",
            &[],
        )
    }

    /// Spaltennamen in Tabellenreihenfolge
    pub fn columns(&self, table: &str) -> Vec<String> {
        self.texts(
            "SELECT name FROM pragma_table_info(?1) ORDER BY cid",
            &[&table],
        )
    }

    /// Eine Spalte mit Typ, NOT NULL und Default-Ausdruck
    pub fn column(&self, table: &str, name: &str) -> Option<ColumnInfo> {
        self.rows(
            "SELECT type, \"notnull\", dflt_value FROM pragma_table_info(?1) WHERE name = ?2",
            &[&table, &name],
            |row| {
                Ok(ColumnInfo {
                    typ: row.get(0)?,
                    notnull: row.get(1)?,
                    dflt_value: row.get(2)?,
                })
            },
        )
        .into_iter()
        .next()
    }

    /// Spalten des Primärschlüssels in Schlüsselreihenfolge
    pub fn primary_key(&self, table: &str) -> Vec<String> {
        self.texts(
            "SELECT name FROM pragma_table_info(?1) WHERE pk > 0 ORDER BY pk",
            &[&table],
        )
    }

    /// Explizit angelegte Indizes (`CREATE INDEX`) mit ihren Spalten
    pub fn indexes(&self, table: &str) -> BTreeMap<String, Vec<String>> {
        self.texts(
            "SELECT name FROM pragma_index_list(?1) WHERE origin = 'c'",
            &[&table],
        )
        .into_iter()
        .map(|name| {
            let columns = self.index_columns(&name);
            (name, columns)
        })
        .collect()
    }

    /// Spaltenlisten der UNIQUE-Constraints, sortiert
    pub fn uniques(&self, table: &str) -> Vec<Vec<String>> {
        let mut result: Vec<Vec<String>> = self
            .texts(
                "SELECT name FROM pragma_index_list(?1) WHERE origin = 'u'",
                &[&table],
            )
            .into_iter()
            .map(|name| self.index_columns(&name))
            .collect();
        result.sort_by_key(|columns| columns.join(","));
        result
    }

    /// Fremdschlüssel der Tabelle, sortiert nach Spalte
    pub fn foreign_keys(&self, table: &str) -> Vec<ForeignKey> {
        let mut result = self.rows(
            "SELECT \"from\", \"table\", \"to\", on_delete FROM pragma_foreign_key_list(?1)",
            &[&table],
            |row| {
                Ok(ForeignKey {
                    from: row.get(0)?,
                    table: row.get(1)?,
                    to: row.get(2)?,
                    on_delete: row.get(3)?,
                })
            },
        );
        result.sort_by(|a, b| a.from.cmp(&b.from));
        result
    }

    /// Kurzformen „Spalte:Kaskade“ der Fremdschlüssel
    pub fn foreign_keys_short(&self, table: &str) -> Vec<String> {
        self.foreign_keys(table)
            .iter()
            .map(ForeignKey::short)
            .collect()
    }

    /// Namen aller Trigger
    pub fn triggers(&self) -> Vec<String> {
        self.texts(
            "SELECT name FROM sqlite_master WHERE type = 'trigger' ORDER BY name",
            &[],
        )
    }

    /// Zahl der Verstöße gegen Fremdschlüssel in der ganzen Datei (0 = alles gültig)
    pub fn foreign_key_violations(&self) -> usize {
        self.rows("PRAGMA foreign_key_check", &[], |row| {
            row.get::<_, String>(0)
        })
        .len()
    }

    /// Nutzer USER, Projekt AIH (Id 1) und Board (Id 1)
    pub fn seed_board(&self) {
        self.run_with("INSERT INTO Users (Id, Name) VALUES (?1, 'Test')", &[&USER])
            .expect("Users");
        self.run_with(
            "INSERT INTO Projects (\"Key\", Name, CreatedAt, CreatedBy) VALUES ('AIH', 'Projekt', ?1, ?2)",
            &[&NOW, &USER],
        )
        .expect("Projects");
        self.run_with(
            "INSERT INTO Boards (ProjectId, OwnerId, Name, CreatedAt) VALUES (1, ?1, 'Board', ?2)",
            &[&USER, &NOW],
        )
        .expect("Boards");
    }

    /// Ticket in Projekt 1 / Board 1 mit allen Pflichtfeldern
    pub fn insert_ticket(&self, id: &str, number: i32) -> rusqlite::Result<usize> {
        self.insert_ticket_with(id, number, "backlog", "normal", 1)
    }

    /// Dasselbe mit abweichendem Status, Priorität oder Board (auch ungültigen, für Fehlerfälle)
    pub fn insert_ticket_with(
        &self,
        id: &str,
        number: i32,
        status: &str,
        priority: &str,
        board_id: i32,
    ) -> rusqlite::Result<usize> {
        self.run_with(
            "INSERT INTO Tickets (Id, ProjectId, BoardId, Number, Title, Status, Priority, SortOrder, NeedsPlanning, CreatedAt, CreatedById, CreatedByName, UpdatedAt, Rev)
             VALUES (?1, 1, ?2, ?3, 'Titel', ?4, ?5, ?3, 0, ?6, ?7, 'Test', ?6, 1)",
            &[&id, &board_id, &number, &status, &priority, &NOW, &USER],
        )
    }

    /// Spalten eines Index in Schlüsselreihenfolge
    fn index_columns(&self, index: &str) -> Vec<String> {
        self.texts(
            "SELECT name FROM pragma_index_info(?1) ORDER BY seqno",
            &[&index],
        )
    }

    /// Schließt die Datei (unter Windows Pflicht vor dem Löschen) und räumt den Ordner weg
    pub fn dispose(self) {
        let Self { dir, conn, .. } = self;
        drop(conn.close());
        drop(dir);
    }
}
