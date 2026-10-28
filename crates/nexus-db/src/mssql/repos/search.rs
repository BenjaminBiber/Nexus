//! Suche über alle Projekte und Boards auf SQL Server; portiert aus
//! `packages/db/src/MssqlSearchRepository.ts`. Gerüst – gefüllt von P2-14.
//!
//! Die Suche verlangt keine Erweiterung: fehlt `note-structure`, lässt sie die Notizen weg
//! (`gate.has("note-structure")`), statt die ganze Suche scheitern zu lassen.

use async_trait::async_trait;
use nexus_core::board::database::BoardSearchRepository;
use nexus_core::board::search_types::{SearchRequest, SearchRows};
use nexus_core::Result;

use crate::mssql::repos::not_implemented;
use crate::mssql::unit_of_work::MssqlUnitOfWork;

#[async_trait]
impl BoardSearchRepository for MssqlUnitOfWork<'_> {
    async fn find(&mut self, _request: &SearchRequest) -> Result<SearchRows> {
        Err(not_implemented("search.find", "P2-14"))
    }
}
