//! Ersatz-Bausteine für Tests ohne SQL Server; entspricht den Attrappen in
//! `packages/db/tests/MssqlFeatureGate.test.ts` und `MssqlSchemaRunner.test.ts`.
//!
//! Das Modul ist **immer** übersetzt, nicht nur unter `#[cfg(test)]`: die Tests unter
//! `crates/nexus-db/tests/` sind eigene Programme und sehen nur, was die Bibliothek öffentlich
//! anbietet. P2-12 bis P2-14 portieren damit die Gate-Tests ihrer Repositories ohne Datenbank.

use async_trait::async_trait;
use nexus_core::Result;

use crate::mssql::runner::{MsRow, MsValue, MssqlRunner};
use crate::mssql::schema_runner::{SchemaBatchError, SchemaBatchRunner};

/// Runner-Ersatz: merkt sich die abgesetzten Anweisungen und liefert vorgegebene Zeilen
#[derive(Debug, Default)]
pub struct FakeRunner {
    /// Die Anweisungen in der Reihenfolge des Absetzens, schon mit `@P<n>` statt der Namen
    pub texts: Vec<String>,
    /// Die Namen der gebundenen Parameter je Anweisung, in der Reihenfolge der Bindung
    pub bound: Vec<Vec<String>>,
    /// Was jede Abfrage liefert
    pub rows: Vec<MsRow>,
    /// Was jede Änderung als Zahl betroffener Zeilen liefert
    pub affected: u64,
}

impl FakeRunner {
    /// Runner, der keine Zeilen liefert
    pub fn new() -> Self {
        Self::default()
    }

    /// Runner, der auf jede Abfrage diese Zeilen liefert
    pub fn with_rows(rows: Vec<MsRow>) -> Self {
        Self {
            affected: rows.len() as u64,
            rows,
            ..Self::default()
        }
    }

    /// Die zuletzt abgesetzte Anweisung
    pub fn last(&self) -> Option<&str> {
        self.texts.last().map(String::as_str)
    }

    /// Merkt die Anweisung samt umgeschriebenen Parametern
    fn record(&mut self, sql: &str, params: &[(&str, MsValue)]) {
        let names: Vec<&str> = params.iter().map(|(name, _)| *name).collect();
        let (text, order) = crate::mssql::runner::rewrite_parameters(sql, &names);
        self.texts.push(text);
        self.bound.push(
            order
                .iter()
                .map(|index| names[*index].to_string())
                .collect(),
        );
    }
}

#[async_trait]
impl MssqlRunner for FakeRunner {
    async fn query_rows(&mut self, sql: &str, params: &[(&str, MsValue)]) -> Result<Vec<MsRow>> {
        self.record(sql, params);
        Ok(self.rows.clone())
    }

    async fn execute(&mut self, sql: &str, params: &[(&str, MsValue)]) -> Result<u64> {
        self.record(sql, params);
        Ok(self.affected)
    }
}

/// Entscheidet je Batchtext, ob der Batch scheitert
type FailWith = Box<dyn Fn(&str) -> Option<SchemaBatchError> + Send + Sync>;

/// Pool-Ersatz für den Schema-Runner: merkt sich die Batches und meldet, was der Test vorgibt
pub struct FakeBatchRunner {
    /// Die ausgeführten Batches in ihrer Reihenfolge
    pub batches: Vec<String>,
    fail: FailWith,
}

impl FakeBatchRunner {
    /// Runner, dem jeder Batch gelingt
    pub fn new() -> Self {
        Self {
            batches: Vec::new(),
            fail: Box::new(|_| None),
        }
    }

    /// Runner, der je Batchtext den vorgegebenen Fehler meldet
    pub fn failing(
        fail: impl Fn(&str) -> Option<SchemaBatchError> + Send + Sync + 'static,
    ) -> Self {
        Self {
            batches: Vec::new(),
            fail: Box::new(fail),
        }
    }
}

impl Default for FakeBatchRunner {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SchemaBatchRunner for FakeBatchRunner {
    async fn batch(&mut self, sql: &str) -> std::result::Result<(), SchemaBatchError> {
        self.batches.push(sql.to_string());
        match (self.fail)(sql) {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }
}
