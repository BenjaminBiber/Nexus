//! Grundlagen der Dispatcher-Auswertung; Gerüst, gefüllt von P2-9 (Port von
//! `packages/db/src/sqlite/SqliteDispatchStatsRepository.ts`).

use std::collections::BTreeMap;

use async_trait::async_trait;

use nexus_core::board::database::DispatchStatsRepository;
use nexus_core::board::dispatch_run_types::{DispatchCommentCounts, DispatchRunFilter};
use nexus_core::board::types::{Comment, HistoryEntry};
use nexus_core::Result;

use super::folgt;
use crate::sqlite::unit_of_work::SqliteUnitOfWork;

#[async_trait]
impl DispatchStatsRepository for SqliteUnitOfWork<'_> {
    async fn done_ticket_ids(&mut self, _filter: &DispatchRunFilter) -> Result<Vec<String>> {
        Err(folgt("DispatchStatsRepository.done_ticket_ids"))
    }

    async fn status_history(&mut self, _ticket_ids: &[String]) -> Result<Vec<HistoryEntry>> {
        Err(folgt("DispatchStatsRepository.status_history"))
    }

    async fn dispatch_comment_counts(
        &mut self,
        _ticket_ids: &[String],
    ) -> Result<BTreeMap<String, DispatchCommentCounts>> {
        Err(folgt("DispatchStatsRepository.dispatch_comment_counts"))
    }

    async fn dispatch_comments(&mut self, _project_id: i32) -> Result<Vec<Comment>> {
        Err(folgt("DispatchStatsRepository.dispatch_comments"))
    }
}
