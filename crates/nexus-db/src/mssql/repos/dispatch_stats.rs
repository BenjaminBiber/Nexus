//! Grundlagen der Dispatcher-Auswertung auf SQL Server; portiert aus
//! `packages/db/src/MssqlDispatchStatsRepository.ts`. Gerüst – gefüllt von P2-14.

use std::collections::BTreeMap;

use async_trait::async_trait;
use nexus_core::board::database::DispatchStatsRepository;
use nexus_core::board::dispatch_run_types::{DispatchCommentCounts, DispatchRunFilter};
use nexus_core::board::types::{Comment, HistoryEntry};
use nexus_core::Result;

use crate::mssql::repos::not_implemented;
use crate::mssql::unit_of_work::MssqlUnitOfWork;

#[async_trait]
impl DispatchStatsRepository for MssqlUnitOfWork<'_> {
    async fn done_ticket_ids(&mut self, _filter: &DispatchRunFilter) -> Result<Vec<String>> {
        Err(not_implemented("dispatch_stats.done_ticket_ids", "P2-14"))
    }

    async fn status_history(&mut self, _ticket_ids: &[String]) -> Result<Vec<HistoryEntry>> {
        Err(not_implemented("dispatch_stats.status_history", "P2-14"))
    }

    async fn dispatch_comment_counts(
        &mut self,
        _ticket_ids: &[String],
    ) -> Result<BTreeMap<String, DispatchCommentCounts>> {
        Err(not_implemented(
            "dispatch_stats.dispatch_comment_counts",
            "P2-14",
        ))
    }

    async fn dispatch_comments(&mut self, _project_id: i32) -> Result<Vec<Comment>> {
        Err(not_implemented("dispatch_stats.dispatch_comments", "P2-14"))
    }
}
