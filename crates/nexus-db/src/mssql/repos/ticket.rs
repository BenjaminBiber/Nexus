//! Tickets auf SQL Server; portiert aus `packages/db/src/MssqlTicketRepository.ts`.
//! Gerüst – gefüllt von P2-12.
//!
//! Dieses Repository braucht das Feature-Gate: ohne die Erweiterung `target-branch` liest und
//! schreibt es Tickets ohne die Spalte `TargetBranch`, und nur ein gesetzter Zielbranch
//! verlangt das Skript (siehe `MssqlFeatureGate.test.ts`).

use std::collections::BTreeMap;

use async_trait::async_trait;
use nexus_core::board::basics::TicketStatus;
use nexus_core::board::database::{TicketFilter, TicketRepository};
use nexus_core::board::types::Ticket;
use nexus_core::Result;

use crate::mssql::repos::not_implemented;
use crate::mssql::unit_of_work::MssqlUnitOfWork;

#[async_trait]
impl TicketRepository for MssqlUnitOfWork<'_> {
    async fn by_id(&mut self, _id: &str, _for_update: bool) -> Result<Option<Ticket>> {
        Err(not_implemented("tickets.by_id", "P2-12"))
    }

    async fn list(&mut self, _filter: &TicketFilter) -> Result<Vec<Ticket>> {
        Err(not_implemented("tickets.list", "P2-12"))
    }

    async fn insert(&mut self, _ticket: &Ticket) -> Result<()> {
        Err(not_implemented("tickets.insert", "P2-12"))
    }

    async fn update(&mut self, _ticket: &Ticket) -> Result<()> {
        Err(not_implemented("tickets.update", "P2-12"))
    }

    async fn delete(&mut self, _id: &str) -> Result<()> {
        Err(not_implemented("tickets.delete", "P2-12"))
    }

    async fn dependents(&mut self, _id: &str) -> Result<Vec<String>> {
        Err(not_implemented("tickets.dependents", "P2-12"))
    }

    async fn remove_dependency_on(&mut self, _id: &str) -> Result<Vec<String>> {
        Err(not_implemented("tickets.remove_dependency_on", "P2-12"))
    }

    async fn max_sort_order(
        &mut self,
        _board_id: i32,
        _status: TicketStatus,
    ) -> Result<Option<i32>> {
        Err(not_implemented("tickets.max_sort_order", "P2-12"))
    }

    async fn open_count_by_board(&mut self, _board_ids: &[i32]) -> Result<BTreeMap<i32, i64>> {
        Err(not_implemented("tickets.open_count_by_board", "P2-12"))
    }
}
