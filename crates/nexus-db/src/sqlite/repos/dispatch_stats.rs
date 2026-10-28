//! Grundlagen der Dispatcher-Auswertung; portiert aus
//! `packages/db/src/sqlite/SqliteDispatchStatsRepository.ts`.
//!
//! Vorsilben werden mit `substr`/`length` statt `LEFT`/`LEN` geprüft; SQLite vergleicht Text
//! exakt, „[DISPATCH] Start“ zählt also nicht.

use std::collections::BTreeMap;
use std::sync::LazyLock;

use async_trait::async_trait;

use nexus_core::board::database::DispatchStatsRepository;
use nexus_core::board::dispatch_run_types::{DispatchCommentCounts, DispatchRunFilter};
use nexus_core::board::types::{Comment, HistoryEntry};
use nexus_core::dispatch::markers::{ABORT, DISPATCH_PREFIX, START};
use nexus_core::Result;

use super::dispatch_run::{liste, namen};
use crate::sqlite::runner::{date, guid, iso, iso_millis};
use crate::sqlite::unit_of_work::SqliteUnitOfWork;
use crate::sqlite_params;
use crate::SqlChunks;

/// Parameternamen einer IN-Liste (`t0`…`t499`); der Runner verlangt Namen mit statischer
/// Lebensdauer, deshalb einmal je Prozess angelegt
static TICKET_NAMES: LazyLock<Vec<&'static str>> = LazyLock::new(|| namen("t"));

#[async_trait]
impl DispatchStatsRepository for SqliteUnitOfWork<'_> {
    async fn done_ticket_ids(&mut self, filter: &DispatchRunFilter) -> Result<Vec<String>> {
        let board = match filter.board_id {
            None => "",
            Some(_) => " AND t.BoardId = @boardId",
        };
        let sql = format!(
            "SELECT h.TicketId FROM History h JOIN Tickets t ON t.Id = h.TicketId
       WHERE h.Field = 'status' AND h.ToValue = 'done' AND t.ProjectId = @projectId{board}
       GROUP BY h.TicketId HAVING MAX(h.At) >= @from AND MAX(h.At) < @to ORDER BY h.TicketId"
        );
        let params = sqlite_params! {
            "projectId": filter.project_id,
            "from": date(&filter.from),
            "to": date(&filter.to),
            "boardId": filter.board_id,
        };
        self.sql
            .query_rows(&sql, &params, |row| row.get::<_, String>(0))
    }

    async fn status_history(&mut self, ticket_ids: &[String]) -> Result<Vec<HistoryEntry>> {
        let mut out: Vec<(i64, HistoryEntry)> = Vec::new();
        for chunk in SqlChunks::of(ticket_ids) {
            let (list, params) = liste(&TICKET_NAMES, chunk);
            let sql = format!(
                "SELECT Id, TicketId, At, ById, ByName, Field, FromValue, ToValue, Note FROM History \
                 WHERE Field IN ('status', 'blockedReason') AND TicketId IN ({list})"
            );
            out.extend(self.sql.query_rows(&sql, &params, |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    HistoryEntry {
                        ticket_id: row.get(1)?,
                        at: iso_millis(&row.get::<_, String>(2)?),
                        by_id: guid(&row.get::<_, String>(3)?),
                        by_name: row.get(4)?,
                        field: row.get(5)?,
                        from: row.get(6)?,
                        to: row.get(7)?,
                        note: row.get(8)?,
                    },
                ))
            })?);
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
            let (list, mut params) = liste(&TICKET_NAMES, chunk);
            params.extend(sqlite_params! { "start": START, "abort": ABORT });
            let sql = format!(
                "SELECT TicketId,
           SUM(CASE WHEN substr(Body, 1, length(@start)) = @start THEN 1 ELSE 0 END) AS Starts,
           SUM(CASE WHEN substr(Body, 1, length(@abort)) = @abort THEN 1 ELSE 0 END) AS Aborts
         FROM Comments
         WHERE TicketId IN ({list}) AND (substr(Body, 1, length(@start)) = @start OR substr(Body, 1, length(@abort)) = @abort)
         GROUP BY TicketId"
            );
            for (ticket_id, counts) in self.sql.query_rows(&sql, &params, |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    DispatchCommentCounts {
                        starts: row.get(1)?,
                        aborts: row.get(2)?,
                    },
                ))
            })? {
                result.insert(ticket_id, counts);
            }
        }
        Ok(result)
    }

    async fn dispatch_comments(&mut self, project_id: i32) -> Result<Vec<Comment>> {
        let sql = format!(
            "SELECT c.Id, c.TicketId, c.AuthorId, c.AuthorName, c.Body, c.CreatedAt, c.ReadByAgentAt
       FROM Comments c JOIN Tickets t ON t.Id = c.TicketId
       WHERE t.ProjectId = @projectId AND substr(c.Body, 1, {}) = @prefix
       ORDER BY c.Id",
            DISPATCH_PREFIX.len()
        );
        self.sql.query_rows(
            &sql,
            &sqlite_params! { "projectId": project_id, "prefix": DISPATCH_PREFIX },
            |row| {
                Ok(Comment {
                    id: row.get(0)?,
                    ticket_id: row.get(1)?,
                    author_id: guid(&row.get::<_, String>(2)?),
                    author_name: row.get(3)?,
                    body: row.get(4)?,
                    created_at: iso_millis(&row.get::<_, String>(5)?),
                    read_by_agent_at: iso(row.get(6)?),
                })
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn die_vorsilbe_der_dispatcher_kommentare_ist_elf_zeichen_lang() {
        // Die SQL-Abfrage schneidet mit substr(…, 1, 11) – wie `substr(c.Body, 1, 11)` in TypeScript
        assert_eq!(DISPATCH_PREFIX, "[dispatch] ");
        assert_eq!(DISPATCH_PREFIX.len(), 11);
    }

    #[test]
    fn die_ticket_namen_reichen_fur_einen_ganzen_block() {
        assert_eq!(TICKET_NAMES.len(), crate::SQL_CHUNK_SIZE);
        assert_eq!(TICKET_NAMES[0], "t0");
    }
}
