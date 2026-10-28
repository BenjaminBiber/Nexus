//! Läufe des Dispatchers und die Fassungen seiner Einstellung; Gerüst, gefüllt von P2-9 (Port
//! von `packages/db/src/sqlite/SqliteDispatchRunRepository.ts`).

use async_trait::async_trait;

use nexus_core::board::database::DispatchRunRepository;
use nexus_core::board::dispatch_run_types::{
    DispatchConfigVersion, DispatchRun, DispatchRunFilter, DispatchRunSource,
};
use nexus_core::Result;

use super::folgt;
use crate::sqlite::unit_of_work::SqliteUnitOfWork;

#[async_trait]
impl DispatchRunRepository for SqliteUnitOfWork<'_> {
    async fn insert(&mut self, _run: &DispatchRun) -> Result<bool> {
        Err(folgt("DispatchRunRepository.insert"))
    }

    async fn list(&mut self, _filter: &DispatchRunFilter) -> Result<Vec<DispatchRun>> {
        Err(folgt("DispatchRunRepository.list"))
    }

    async fn list_all(&mut self, _from: &str, _to: &str) -> Result<Vec<DispatchRun>> {
        Err(folgt("DispatchRunRepository.list_all"))
    }

    async fn first_started(
        &mut self,
        _project_id: i32,
        _source: DispatchRunSource,
    ) -> Result<Option<String>> {
        Err(folgt("DispatchRunRepository.first_started"))
    }

    async fn existing(&mut self, _ids: &[String]) -> Result<Vec<String>> {
        Err(folgt("DispatchRunRepository.existing"))
    }

    async fn record_config_version(
        &mut self,
        _version: &DispatchConfigVersion,
    ) -> Result<bool> {
        Err(folgt("DispatchRunRepository.record_config_version"))
    }

    async fn list_config_versions(
        &mut self,
        _from: &str,
        _to: &str,
    ) -> Result<Vec<DispatchConfigVersion>> {
        Err(folgt("DispatchRunRepository.list_config_versions"))
    }
}
