//! Grundlagen der Dispatcher-Auswertung auf SQL Server; portiert aus
//! `packages/db/src/MssqlDispatchStatsRepository.ts`.
//!
//! Die Vorsilben der `[dispatch]`-Kommentare werden mit `LEFT`/`LEN` statt mit `LIKE` geprüft:
//! in einem `LIKE`-Muster wäre „[“ ein Platzhalter.

use std::collections::BTreeMap;

use async_trait::async_trait;
use nexus_core::board::database::DispatchStatsRepository;
use nexus_core::board::dispatch_run_types::{DispatchCommentCounts, DispatchRunFilter};
use nexus_core::board::types::{Comment, HistoryEntry};
use nexus_core::dispatch::markers::{ABORT, START};
use nexus_core::Result;

use super::dispatch_run::geliehen;
use crate::mssql::runner::MsValue;
use crate::mssql::unit_of_work::MssqlUnitOfWork;
use crate::SqlChunks;

#[async_trait]
impl DispatchStatsRepository for MssqlUnitOfWork<'_> {
    async fn done_ticket_ids(&mut self, filter: &DispatchRunFilter) -> Result<Vec<String>> {
        let board = match filter.board_id {
            None => "",
            Some(_) => " AND t.BoardId = @boardId",
        };
        let sql = format!(
            "SELECT h.TicketId FROM board.History h JOIN board.Tickets t ON t.Id = h.TicketId
       WHERE h.Field = 'status' AND h.ToValue = N'done' AND t.ProjectId = @projectId{board}
       GROUP BY h.TicketId HAVING MAX(h.At) >= @from AND MAX(h.At) < @to ORDER BY h.TicketId"
        );
        let mut params = vec![
            ("projectId", MsValue::Int(filter.project_id)),
            ("from", MsValue::date(&filter.from)?),
            ("to", MsValue::date(&filter.to)?),
        ];
        if let Some(board_id) = filter.board_id {
            params.push(("boardId", MsValue::Int(board_id)));
        }
        let rows = self.sql.query_rows(&sql, &params).await?;
        rows.iter().map(|row| row.get_str("TicketId")).collect()
    }

    async fn status_history(&mut self, ticket_ids: &[String]) -> Result<Vec<HistoryEntry>> {
        let mut out: Vec<(i64, HistoryEntry)> = Vec::new();
        for chunk in SqlChunks::of(ticket_ids) {
            let werte: Vec<MsValue> = chunk.iter().map(|id| MsValue::text(id.clone())).collect();
            let (list, params) = SqlChunks::params("t", &werte);
            let sql = format!(
                "SELECT Id, TicketId, At, ById, ByName, Field, FromValue, ToValue, Note FROM board.History \
                 WHERE Field IN ('status', 'blockedReason') AND TicketId IN ({list})"
            );
            for row in self.sql.query_rows(&sql, &geliehen(&params)).await? {
                out.push((
                    row.get_i64("Id")?,
                    HistoryEntry {
                        ticket_id: row.get_str("TicketId")?,
                        at: row.get_iso("At")?,
                        by_id: row.get_guid("ById")?,
                        by_name: row.get_str("ByName")?,
                        field: row.get_str("Field")?,
                        from: row.get_opt_str("FromValue")?,
                        to: row.get_opt_str("ToValue")?,
                        note: row.get_opt_str("Note")?,
                    },
                ));
            }
        }
        // Reihenfolge des Eintrags, nicht die der Blöcke
        out.sort_by_key(|(id, _)| *id);
        Ok(out.into_iter().map(|(_, entry)| entry).collect())
    }

    async fn dispatch_comment_counts(
        &mut self,
        ticket_ids: &[String],
    ) -> Result<BTreeMap<String, DispatchCommentCounts>> {
        let mut result = BTreeMap::new();
        for chunk in SqlChunks::of(ticket_ids) {
            let werte: Vec<MsValue> = chunk.iter().map(|id| MsValue::text(id.clone())).collect();
            let (list, params) = SqlChunks::params("t", &werte);
            let mut params = geliehen(&params);
            params.push(("start", MsValue::text(START)));
            params.push(("abort", MsValue::text(ABORT)));
            let sql = format!(
                "SELECT TicketId,
           SUM(CASE WHEN LEFT(Body, LEN(@start)) = @start THEN 1 ELSE 0 END) AS Starts,
           SUM(CASE WHEN LEFT(Body, LEN(@abort)) = @abort THEN 1 ELSE 0 END) AS Aborts
         FROM board.Comments
         WHERE TicketId IN ({list}) AND (LEFT(Body, LEN(@start)) = @start OR LEFT(Body, LEN(@abort)) = @abort)
         GROUP BY TicketId"
            );
            for row in self.sql.query_rows(&sql, &params).await? {
                result.insert(
                    row.get_str("TicketId")?,
                    DispatchCommentCounts {
                        starts: row.get_i32("Starts")?,
                        aborts: row.get_i32("Aborts")?,
                    },
                );
            }
        }
        Ok(result)
    }

    async fn dispatch_comments(&mut self, project_id: i32) -> Result<Vec<Comment>> {
        let rows = self
            .sql
            .query_rows(
                "SELECT c.Id, c.TicketId, c.AuthorId, c.AuthorName, c.Body, c.CreatedAt, c.ReadByAgentAt
       FROM board.Comments c JOIN board.Tickets t ON t.Id = c.TicketId
       WHERE t.ProjectId = @projectId AND LEFT(c.Body, 11) = N'[dispatch] ' AND SUBSTRING(c.Body, 11, 1) = N' '
       ORDER BY c.Id",
                &[("projectId", MsValue::Int(project_id))],
            )
            .await?;
        rows.iter()
            .map(|row| {
                Ok(Comment {
                    id: row.get_i64("Id")?,
                    ticket_id: row.get_str("TicketId")?,
                    author_id: row.get_guid("AuthorId")?,
                    author_name: row.get_str("AuthorName")?,
                    body: row.get_str("Body")?,
                    created_at: row.get_iso("CreatedAt")?,
                    read_by_agent_at: row.get_opt_iso("ReadByAgentAt")?,
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nexus_core::dispatch::markers::DISPATCH_PREFIX;

    #[test]
    fn die_vorsilbe_der_dispatcher_kommentare_ist_elf_zeichen_lang() {
        // Die Abfrage schneidet mit LEFT(c.Body, 11) – wie `LEFT(c.Body, 11)` in TypeScript
        assert_eq!(DISPATCH_PREFIX, "[dispatch] ");
        assert_eq!(DISPATCH_PREFIX.len(), 11);
        assert_eq!(START, "[dispatch] Start");
        assert_eq!(ABORT, "[dispatch] Abbruch");
    }
}
