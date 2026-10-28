//! Suche über alle Projekte und Boards; Gerüst, gefüllt von P2-9 (Port von
//! `packages/db/src/sqlite/SqliteSearchRepository.ts`). Die Funktion `harness_lower`, die jede
//! Abfrage der Suche braucht, meldet [`crate::sqlite::SqliteBoardDatabase`] schon beim Öffnen an.

use async_trait::async_trait;

use nexus_core::board::database::BoardSearchRepository;
use nexus_core::board::search_types::{SearchRequest, SearchRows};
use nexus_core::Result;

use super::folgt;
use crate::sqlite::unit_of_work::SqliteUnitOfWork;

#[async_trait]
impl BoardSearchRepository for SqliteUnitOfWork<'_> {
    async fn find(&mut self, _request: &SearchRequest) -> Result<SearchRows> {
        Err(folgt("BoardSearchRepository.find"))
    }
}
