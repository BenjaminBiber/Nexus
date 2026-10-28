//! Recherchen zu einer Frage auf SQL Server; portiert aus
//! `packages/db/src/MssqlQuestionResearchRepository.ts`. Gerüst – gefüllt von P2-13.

use std::collections::BTreeMap;

use async_trait::async_trait;
use nexus_core::board::database::{BoardQuestionResearchRepository, NewQuestionResearch};
use nexus_core::board::question_types::{BoardQuestionResearch, BoardQuestionResearchSummary};
use nexus_core::Result;

use crate::mssql::repos::not_implemented;
use crate::mssql::unit_of_work::MssqlUnitOfWork;

#[async_trait]
impl BoardQuestionResearchRepository for MssqlUnitOfWork<'_> {
    async fn list(&mut self, _question_id: &str) -> Result<Vec<BoardQuestionResearch>> {
        Err(not_implemented("question_research.list", "P2-13"))
    }

    async fn insert(&mut self, _research: &NewQuestionResearch) -> Result<BoardQuestionResearch> {
        Err(not_implemented("question_research.insert", "P2-13"))
    }

    async fn summaries(
        &mut self,
        _question_ids: &[String],
    ) -> Result<BTreeMap<String, BoardQuestionResearchSummary>> {
        Err(not_implemented("question_research.summaries", "P2-13"))
    }
}
