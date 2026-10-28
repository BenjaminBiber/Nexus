//! Projekte auf SQL Server; portiert aus `packages/db/src/MssqlProjectRepository.ts`.
//! Gerüst – gefüllt von P2-12.

use async_trait::async_trait;
use nexus_core::board::database::{NewProject, ProjectRepository};
use nexus_core::board::types::Project;
use nexus_core::Result;

use crate::mssql::repos::not_implemented;
use crate::mssql::unit_of_work::MssqlUnitOfWork;

#[async_trait]
impl ProjectRepository for MssqlUnitOfWork<'_> {
    async fn list(&mut self) -> Result<Vec<Project>> {
        Err(not_implemented("projects.list", "P2-12"))
    }

    async fn by_id(&mut self, _id: i32) -> Result<Option<Project>> {
        Err(not_implemented("projects.by_id", "P2-12"))
    }

    async fn by_key(&mut self, _key: &str) -> Result<Option<Project>> {
        Err(not_implemented("projects.by_key", "P2-12"))
    }

    async fn insert(&mut self, _project: &NewProject) -> Result<Project> {
        Err(not_implemented("projects.insert", "P2-12"))
    }

    async fn rename(&mut self, _id: i32, _name: &str) -> Result<()> {
        Err(not_implemented("projects.rename", "P2-12"))
    }

    async fn take_ticket_number(&mut self, _id: i32) -> Result<i32> {
        Err(not_implemented("projects.take_ticket_number", "P2-12"))
    }

    async fn take_question_number(&mut self, _id: i32) -> Result<i32> {
        Err(not_implemented("projects.take_question_number", "P2-12"))
    }
}
