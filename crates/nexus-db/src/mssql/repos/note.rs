//! Notizen eines Boards auf SQL Server; portiert aus
//! `packages/db/src/MssqlNoteRepository.ts`. Gerüst – gefüllt von P2-13.
//!
//! Jeder Zugriff hier verlangt die Erweiterung `note-structure` (Titel und Zähler der Notiz);
//! das Feature-Gate meldet sonst `board-schema-outdated` mit Hinweis auf
//! `packages/db/sql/005_note_structure.sql`.

use async_trait::async_trait;
use nexus_core::board::database::{NewNote, NoteFilter, NoteRepository};
use nexus_core::board::types::Note;
use nexus_core::Result;

use crate::mssql::repos::not_implemented;
use crate::mssql::unit_of_work::MssqlUnitOfWork;

#[async_trait]
impl NoteRepository for MssqlUnitOfWork<'_> {
    async fn list(&mut self, _filter: &NoteFilter) -> Result<Vec<Note>> {
        self.gate.require(&["note-structure"])?;
        Err(not_implemented("notes.list", "P2-13"))
    }

    async fn by_id(&mut self, _id: i32, _for_update: bool) -> Result<Option<Note>> {
        self.gate.require(&["note-structure"])?;
        Err(not_implemented("notes.by_id", "P2-13"))
    }

    async fn insert(&mut self, _note: &NewNote) -> Result<Note> {
        self.gate.require(&["note-structure"])?;
        Err(not_implemented("notes.insert", "P2-13"))
    }

    async fn update(&mut self, _note: &Note) -> Result<()> {
        self.gate.require(&["note-structure"])?;
        Err(not_implemented("notes.update", "P2-13"))
    }
}
