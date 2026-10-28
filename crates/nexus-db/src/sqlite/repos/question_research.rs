//! Recherchen zu einer Frage; Gerüst, gefüllt von P2-8 (Port von
//! `packages/db/src/sqlite/SqliteQuestionResearchRepository.ts`).

use std::collections::BTreeMap;

use async_trait::async_trait;

use nexus_core::board::database::{BoardQuestionResearchRepository, NewQuestionResearch};
use nexus_core::board::question_types::{BoardQuestionResearch, BoardQuestionResearchSummary};
use nexus_core::Result;

use super::folgt;
use crate::sqlite::unit_of_work::SqliteUnitOfWork;

#[async_trait]
impl BoardQuestionResearchRepository for SqliteUnitOfWork<'_> {
    async fn list(&mut self, _question_id: &str) -> Result<Vec<BoardQuestionResearch>> {
        Err(folgt("BoardQuestionResearchRepository.list"))
    }

    async fn insert(&mut self, _research: &NewQuestionResearch) -> Result<BoardQuestionResearch> {
        Err(folgt("BoardQuestionResearchRepository.insert"))
    }

    async fn summaries(
        &mut self,
        _question_ids: &[String],
    ) -> Result<BTreeMap<String, BoardQuestionResearchSummary>> {
        Err(folgt("BoardQuestionResearchRepository.summaries"))
    }
}
