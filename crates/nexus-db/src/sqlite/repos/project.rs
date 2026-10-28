//! Projekte samt der fortlaufenden Nummern; Gerüst, gefüllt von P2-6 (Port von
//! `packages/db/src/sqlite/SqliteProjectRepository.ts`).

use async_trait::async_trait;

use nexus_core::board::database::{NewProject, ProjectRepository};
use nexus_core::board::types::Project;
use nexus_core::Result;

use super::folgt;
use crate::sqlite::unit_of_work::SqliteUnitOfWork;

#[async_trait]
impl ProjectRepository for SqliteUnitOfWork<'_> {
    async fn list(&mut self) -> Result<Vec<Project>> {
        Err(folgt("ProjectRepository.list"))
    }

    async fn by_id(&mut self, _id: i32) -> Result<Option<Project>> {
        Err(folgt("ProjectRepository.by_id"))
    }

    async fn by_key(&mut self, _key: &str) -> Result<Option<Project>> {
        Err(folgt("ProjectRepository.by_key"))
    }

    async fn insert(&mut self, _project: &NewProject) -> Result<Project> {
        Err(folgt("ProjectRepository.insert"))
    }

    async fn rename(&mut self, _id: i32, _name: &str) -> Result<()> {
        Err(folgt("ProjectRepository.rename"))
    }

    async fn take_ticket_number(&mut self, _id: i32) -> Result<i32> {
        Err(folgt("ProjectRepository.take_ticket_number"))
    }

    async fn take_question_number(&mut self, _id: i32) -> Result<i32> {
        Err(folgt("ProjectRepository.take_question_number"))
    }
}
