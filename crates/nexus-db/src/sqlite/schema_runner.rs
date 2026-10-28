//! Legt das Schema einer SQLite-Datei an bzw. ergänzt fehlende Stufen; portiert aus
//! `packages/db/src/sqlite/SqliteSchemaRunner.ts`.
//!
//! Der Runner pflegt `SchemaVersion` und `SchemaFeatures` so, dass der
//! [`crate::schema_version_guard::SchemaVersionGuard`] unverändert prüfen kann, und merkt sich in
//! `SchemaSteps`, welche Stufen schon gelaufen sind.

use rusqlite::Connection;

use nexus_core::{HarnessError, Result};

use super::error_map;
use super::schema::{SqliteSchemaStep, SQLITE_SCHEMA_STEPS};

/// Buchführung des Runners: welche Stufen schon eingespielt sind. Der Text ist Vertrag – er steht
/// so in `sqlite_master` jeder von TypeScript angelegten Datei.
const STEPS_TABLE: &str = "CREATE TABLE IF NOT EXISTS SchemaSteps (Id TEXT NOT NULL PRIMARY KEY, AppliedAt TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')))";

/// Spielt die Schema-Stufen einer SQLite-Datei ein und liest den erreichten Stand
pub struct SqliteSchemaRunner<'c> {
    conn: &'c Connection,
    steps: &'c [SqliteSchemaStep],
}

impl<'c> SqliteSchemaRunner<'c> {
    /// Runner mit allen Stufen aus [`SQLITE_SCHEMA_STEPS`]
    pub fn new(conn: &'c Connection) -> Self {
        Self {
            conn,
            steps: SQLITE_SCHEMA_STEPS,
        }
    }

    /// Runner mit einer eigenen Stufenliste (für Tests, die einzelne Stände prüfen)
    pub fn with_steps(conn: &'c Connection, steps: &'c [SqliteSchemaStep]) -> Self {
        Self { conn, steps }
    }

    /// Führt alle fehlenden Stufen der Reihe nach aus, jede in einer eigenen Transaktion;
    /// liefert die Ids der ausgeführten
    pub fn run(&self) -> Result<Vec<String>> {
        self.conn
            .execute_batch(STEPS_TABLE)
            .map_err(error_map::wrap)?;
        let mut applied = Vec::new();
        for step in self.steps {
            if !self.is_applied(step.id)? && self.apply(step)? {
                applied.push(step.id.to_string());
            }
        }
        Ok(applied)
    }

    /// Höchste eingetragene Schema-Version; `None`, solange es `SchemaVersion` nicht gibt
    pub fn schema_version(&self) -> Result<Option<i32>> {
        if !self.has_table("SchemaVersion")? {
            return Ok(None);
        }
        self.conn
            .query_row("SELECT MAX(Version) AS V FROM SchemaVersion", [], |row| {
                row.get::<_, Option<i32>>(0)
            })
            .map_err(error_map::wrap)
    }

    /// Eingetragene additive Erweiterungen; leer, solange es `SchemaFeatures` nicht gibt
    pub fn schema_features(&self) -> Result<Vec<String>> {
        if !self.has_table("SchemaFeatures")? {
            return Ok(Vec::new());
        }
        let mut statement = self
            .conn
            .prepare("SELECT Name FROM SchemaFeatures ORDER BY Name")
            .map_err(error_map::wrap)?;
        let names = statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(error_map::wrap)?
            .collect::<rusqlite::Result<Vec<String>>>()
            .map_err(error_map::wrap)?;
        Ok(names)
    }

    /// Eine Stufe in einer Transaktion; `false`, wenn ein anderer Prozess sie inzwischen
    /// eingespielt hat
    fn apply(&self, step: &SqliteSchemaStep) -> Result<bool> {
        self.conn
            .execute_batch("BEGIN IMMEDIATE")
            .map_err(error_map::wrap)?;
        match self.apply_in_transaction(step) {
            Ok(angewandt) => {
                self.conn.execute_batch("COMMIT").map_err(error_map::wrap)?;
                Ok(angewandt)
            }
            Err(error) => {
                // SQLite kann die Transaktion bei manchen Fehlern schon selbst beendet haben
                let _ = self.conn.execute_batch("ROLLBACK");
                Err(HarnessError::new(
                    "board-schema-failed",
                    format!(
                        "Schema-Stufe {} der SQLite-Datei ist gescheitert: {error}",
                        step.id
                    ),
                )
                .into())
            }
        }
    }

    /// Die Arbeit einer Stufe innerhalb der offenen Transaktion; `false`, wenn sie schon
    /// eingespielt ist
    fn apply_in_transaction(&self, step: &SqliteSchemaStep) -> Result<bool> {
        if self.is_applied(step.id)? {
            return Ok(false);
        }
        self.conn.execute_batch(step.sql).map_err(error_map::wrap)?;
        if let Some(version) = step.version {
            self.conn
                .execute("INSERT INTO SchemaVersion (Version) VALUES (?1)", [version])
                .map_err(error_map::wrap)?;
        }
        if let Some(feature) = step.feature {
            self.conn
                .execute(
                    "INSERT INTO SchemaFeatures (Name, Script) VALUES (?1, ?2)",
                    [feature, step.id],
                )
                .map_err(error_map::wrap)?;
        }
        self.conn
            .execute("INSERT INTO SchemaSteps (Id) VALUES (?1)", [step.id])
            .map_err(error_map::wrap)?;
        Ok(true)
    }

    fn is_applied(&self, id: &str) -> Result<bool> {
        self.exists("SELECT 1 AS X FROM SchemaSteps WHERE Id = ?1", id)
    }

    fn has_table(&self, name: &str) -> Result<bool> {
        self.exists(
            "SELECT 1 AS X FROM sqlite_master WHERE type = 'table' AND name = ?1",
            name,
        )
    }

    /// `true`, wenn die Abfrage mit diesem Wert mindestens eine Zeile liefert
    fn exists(&self, sql: &str, value: &str) -> Result<bool> {
        let mut statement = self.conn.prepare_cached(sql).map_err(error_map::wrap)?;
        let gefunden = statement.exists([value]).map_err(error_map::wrap)?;
        Ok(gefunden)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Der Standard-Ausdruck für „jetzt“, den die Stufen und die Buchführung verwenden
    const NOW: &str = "(strftime('%Y-%m-%dT%H:%M:%fZ','now'))";

    #[test]
    fn eine_leere_datei_hat_keine_version_und_keine_erweiterungen() {
        let conn = Connection::open_in_memory().unwrap();
        let runner = SqliteSchemaRunner::with_steps(&conn, &[]);
        assert_eq!(runner.schema_version().unwrap(), None);
        assert_eq!(runner.schema_features().unwrap(), Vec::<String>::new());
    }

    #[test]
    fn die_standardliste_lauft_ohne_fehler() {
        let conn = Connection::open_in_memory().unwrap();
        let runner = SqliteSchemaRunner::new(&conn);
        let ids = runner.run().unwrap();
        assert_eq!(ids.len(), SQLITE_SCHEMA_STEPS.len());
        assert_eq!(runner.schema_version().unwrap(), Some(3));
        // Ein zweiter Lauf auf derselben Verbindung ändert nichts
        assert!(runner.run().unwrap().is_empty());
    }

    #[test]
    fn der_text_der_buchfuhrung_bleibt_wortgleich() {
        // Der Text landet unverändert in sqlite_master und muss zu TypeScript passen
        assert!(STEPS_TABLE.starts_with("CREATE TABLE IF NOT EXISTS SchemaSteps (Id TEXT NOT NULL"));
        assert!(STEPS_TABLE.ends_with(&format!("DEFAULT {NOW})")));
    }
}
