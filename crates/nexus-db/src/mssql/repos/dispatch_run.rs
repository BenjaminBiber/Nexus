//! Läufe des Dispatchers auf SQL Server; portiert aus
//! `packages/db/src/MssqlDispatchRunRepository.ts`. Gerüst – gefüllt von P2-14.
//!
//! Jeder Zugriff verlangt `dispatch-runs` **und** `dispatch-run-details`: die Abfragen lesen die
//! Spalten aus 006 mit, ohne die Erweiterung scheiterten sie sonst als SQL-Fehler statt mit
//! einem Hinweis auf das Skript.

use async_trait::async_trait;
use nexus_core::board::database::DispatchRunRepository;
use nexus_core::board::dispatch_run_types::{
    DispatchConfigVersion, DispatchRun, DispatchRunFilter, DispatchRunSource,
};
use nexus_core::Result;

use crate::mssql::repos::not_implemented;
use crate::mssql::unit_of_work::MssqlUnitOfWork;

/// Die Erweiterungen, ohne die kein Zugriff auf die Läufe geht (Reihenfolge der Skripte)
const NEEDED: [&str; 2] = ["dispatch-runs", "dispatch-run-details"];

#[async_trait]
impl DispatchRunRepository for MssqlUnitOfWork<'_> {
    async fn insert(&mut self, _run: &DispatchRun) -> Result<bool> {
        self.gate.require(&NEEDED)?;
        Err(not_implemented("dispatch_runs.insert", "P2-14"))
    }

    async fn list(&mut self, _filter: &DispatchRunFilter) -> Result<Vec<DispatchRun>> {
        self.gate.require(&NEEDED)?;
        Err(not_implemented("dispatch_runs.list", "P2-14"))
    }

    async fn list_all(&mut self, _from: &str, _to: &str) -> Result<Vec<DispatchRun>> {
        self.gate.require(&NEEDED)?;
        Err(not_implemented("dispatch_runs.list_all", "P2-14"))
    }

    async fn first_started(
        &mut self,
        _project_id: i32,
        _source: DispatchRunSource,
    ) -> Result<Option<String>> {
        self.gate.require(&NEEDED)?;
        Err(not_implemented("dispatch_runs.first_started", "P2-14"))
    }

    async fn existing(&mut self, _ids: &[String]) -> Result<Vec<String>> {
        self.gate.require(&NEEDED)?;
        Err(not_implemented("dispatch_runs.existing", "P2-14"))
    }

    async fn record_config_version(&mut self, _version: &DispatchConfigVersion) -> Result<bool> {
        self.gate.require(&NEEDED)?;
        Err(not_implemented(
            "dispatch_runs.record_config_version",
            "P2-14",
        ))
    }

    async fn list_config_versions(
        &mut self,
        _from: &str,
        _to: &str,
    ) -> Result<Vec<DispatchConfigVersion>> {
        self.gate.require(&NEEDED)?;
        Err(not_implemented(
            "dispatch_runs.list_config_versions",
            "P2-14",
        ))
    }
}
