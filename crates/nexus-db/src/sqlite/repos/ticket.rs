//! Tickets samt Labels, Voraussetzungen und Sortierung; Gerüst, gefüllt von P2-6 (Port von
//! `packages/db/src/sqlite/SqliteTicketRepository.ts`).

use std::collections::BTreeMap;

use async_trait::async_trait;

use nexus_core::board::basics::TicketStatus;
use nexus_core::board::database::{TicketFilter, TicketRepository};
use nexus_core::board::types::Ticket;
use nexus_core::Result;

use super::folgt;
use crate::sqlite::unit_of_work::SqliteUnitOfWork;

#[async_trait]
impl TicketRepository for SqliteUnitOfWork<'_> {
    async fn by_id(&mut self, _id: &str, _for_update: bool) -> Result<Option<Ticket>> {
        Err(folgt("TicketRepository.by_id"))
    }

    async fn list(&mut self, _filter: &TicketFilter) -> Result<Vec<Ticket>> {
        Err(folgt("TicketRepository.list"))
    }

    async fn insert(&mut self, _ticket: &Ticket) -> Result<()> {
        Err(folgt("TicketRepository.insert"))
    }

    async fn update(&mut self, _ticket: &Ticket) -> Result<()> {
        Err(folgt("TicketRepository.update"))
    }

    async fn delete(&mut self, _id: &str) -> Result<()> {
        Err(folgt("TicketRepository.delete"))
    }

    async fn dependents(&mut self, _id: &str) -> Result<Vec<String>> {
        Err(folgt("TicketRepository.dependents"))
    }

    async fn remove_dependency_on(&mut self, _id: &str) -> Result<Vec<String>> {
        Err(folgt("TicketRepository.remove_dependency_on"))
    }

    async fn max_sort_order(
        &mut self,
        _board_id: i32,
        _status: TicketStatus,
    ) -> Result<Option<i32>> {
        Err(folgt("TicketRepository.max_sort_order"))
    }

    async fn open_count_by_board(&mut self, _board_ids: &[i32]) -> Result<BTreeMap<i32, i64>> {
        Err(folgt("TicketRepository.open_count_by_board"))
    }
}
