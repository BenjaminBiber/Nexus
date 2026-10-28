//! Notizen eines Boards; Gerüst, gefüllt von P2-7 (Port von
//! `packages/db/src/sqlite/SqliteNoteRepository.ts`).

use async_trait::async_trait;

use nexus_core::board::database::{NewNote, NoteFilter, NoteRepository};
use nexus_core::board::types::Note;
use nexus_core::Result;

use super::folgt;
use crate::sqlite::unit_of_work::SqliteUnitOfWork;

#[async_trait]
impl NoteRepository for SqliteUnitOfWork<'_> {
    async fn list(&mut self, _filter: &NoteFilter) -> Result<Vec<Note>> {
        Err(folgt("NoteRepository.list"))
    }

    async fn by_id(&mut self, _id: i32, _for_update: bool) -> Result<Option<Note>> {
        Err(folgt("NoteRepository.by_id"))
    }

    async fn insert(&mut self, _note: &NewNote) -> Result<Note> {
        Err(folgt("NoteRepository.insert"))
    }

    async fn update(&mut self, _note: &Note) -> Result<()> {
        Err(folgt("NoteRepository.update"))
    }
}
