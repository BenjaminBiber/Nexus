//! Notizen eines Boards auf SQL Server; portiert aus
//! `packages/db/src/MssqlNoteRepository.ts`.
//!
//! Jeder Zugriff hier verlangt die Erweiterung `note-structure` (Titel und Zähler der Notiz);
//! das Feature-Gate meldet sonst `board-schema-outdated` mit Hinweis auf
//! `packages/db/sql/005_note_structure.sql`.
//!
//! `Labels` und `SeenIn` stehen als kommagetrennter Text in der Spalte (Labels und Ticket-IDs
//! enthalten nie Kommas).

use std::str::FromStr;

use async_trait::async_trait;
use nexus_core::board::basics::NoteState;
use nexus_core::board::database::{NewNote, NoteFilter, NoteRepository};
use nexus_core::board::types::Note;
use nexus_core::{Error, Result};

use crate::mssql::repos::{borrow_params, first_row};
use crate::mssql::runner::{MsRow, MsValue};
use crate::mssql::unit_of_work::MssqlUnitOfWork;

const COLUMNS: &str = "Id, BoardId, AuthorId, AuthorName, Title, Body, Source, Labels, SeenCount, SeenIn, State, TicketId, Reason, CreatedAt";

/// Kommagetrennter Text → Liste; leere Teile fallen weg (wie `split(',').filter(Boolean)`)
fn split_list(value: &str) -> Vec<String> {
    value
        .split(',')
        .filter(|part| !part.is_empty())
        .map(str::to_string)
        .collect()
}

/// Zeile → Notiz; ein unbekannter Stand ist ein Fehler, kein stiller Cast wie in TypeScript
fn map_row(row: &MsRow) -> Result<Note> {
    let id = row.get_i32("Id")?;
    let state = row.get_str("State")?;
    Ok(Note {
        id,
        board_id: row.get_i32("BoardId")?,
        author_id: row.get_guid("AuthorId")?,
        author_name: row.get_str("AuthorName")?,
        title: row.get_str("Title")?,
        body: row.get_str("Body")?,
        source: row.get_opt_str("Source")?,
        labels: split_list(&row.get_str("Labels")?),
        seen_count: row.get_i32("SeenCount")?,
        seen_in: split_list(&row.get_str("SeenIn")?),
        state: NoteState::from_str(&state).map_err(|_| {
            Error::unexpected_message(format!("Notiz {id}: Stand „{state}“ ist unbekannt."))
        })?,
        ticket_id: row.get_opt_str("TicketId")?,
        reason: row.get_opt_str("Reason")?,
        created_at: row.get_iso("CreatedAt")?,
    })
}

#[async_trait]
impl NoteRepository for MssqlUnitOfWork<'_> {
    /// Eine leere Statusliste ergibt keine Notizen; ohne Statusliste wird nicht nach Stand
    /// gefiltert
    async fn list(&mut self, filter: &NoteFilter) -> Result<Vec<Note>> {
        self.gate.require(&["note-structure"])?;
        let states = filter.states.clone().unwrap_or_default();
        if filter.states.is_some() && states.is_empty() {
            return Ok(Vec::new());
        }
        let mut params: Vec<(String, MsValue)> = vec![
            ("boardId".to_string(), MsValue::Int(filter.board_id)),
            (
                "source".to_string(),
                MsValue::text_opt(filter.source.as_deref()),
            ),
        ];
        for (index, state) in states.iter().enumerate() {
            params.push((format!("s{index}"), MsValue::text(state.as_str())));
        }
        let state_clause = if states.is_empty() {
            String::new()
        } else {
            let list = (0..states.len())
                .map(|index| format!("@s{index}"))
                .collect::<Vec<_>>()
                .join(", ");
            format!("AND State IN ({list})")
        };
        let sql = format!(
            "SELECT {COLUMNS} FROM board.Notes WHERE BoardId = @boardId {state_clause} AND (@source IS NULL OR Source = @source) ORDER BY Id"
        );
        let rows = self.sql.query_rows(&sql, &borrow_params(&params)).await?;
        rows.iter().map(map_row).collect()
    }

    /// `for_update` sperrt die Zeile bis zum Ende der Transaktion (gegen gleichzeitiges
    /// Übernehmen/Verwerfen)
    async fn by_id(&mut self, id: i32, for_update: bool) -> Result<Option<Note>> {
        self.gate.require(&["note-structure"])?;
        let hint = if for_update {
            "WITH (UPDLOCK, ROWLOCK)"
        } else {
            ""
        };
        let sql = format!("SELECT {COLUMNS} FROM board.Notes {hint} WHERE Id = @id");
        let rows = self
            .sql
            .query_rows(&sql, &[("id", MsValue::Int(id))])
            .await?;
        rows.first().map(map_row).transpose()
    }

    async fn insert(&mut self, note: &NewNote) -> Result<Note> {
        self.gate.require(&["note-structure"])?;
        let rows = self
            .sql
            .query_rows(
                "INSERT INTO board.Notes (BoardId, AuthorId, AuthorName, Title, Body, Source, Labels, SeenCount, SeenIn, State, TicketId, Reason, CreatedAt)
       OUTPUT inserted.Id VALUES (@boardId, @authorId, @authorName, @title, @body, @source, @labels, @seenCount, @seenIn, @state, @ticketId, @reason, @createdAt)",
                &[
                    ("boardId", MsValue::Int(note.board_id)),
                    ("authorId", MsValue::text(&note.author_id)),
                    ("authorName", MsValue::text(&note.author_name)),
                    ("title", MsValue::text(&note.title)),
                    ("body", MsValue::text(&note.body)),
                    ("source", MsValue::text_opt(note.source.as_deref())),
                    ("labels", MsValue::text(note.labels.join(","))),
                    ("seenCount", MsValue::Int(note.seen_count)),
                    ("seenIn", MsValue::text(note.seen_in.join(","))),
                    ("state", MsValue::text(note.state.as_str())),
                    ("ticketId", MsValue::text_opt(note.ticket_id.as_deref())),
                    ("reason", MsValue::text_opt(note.reason.as_deref())),
                    ("createdAt", MsValue::date(&note.created_at)?),
                ],
            )
            .await?;
        let id = first_row(&rows, "notes.insert")?.get_i32("Id")?;
        Ok(Note {
            id,
            board_id: note.board_id,
            author_id: note.author_id.clone(),
            author_name: note.author_name.clone(),
            title: note.title.clone(),
            body: note.body.clone(),
            source: note.source.clone(),
            labels: note.labels.clone(),
            seen_count: note.seen_count,
            seen_in: note.seen_in.clone(),
            state: note.state,
            ticket_id: note.ticket_id.clone(),
            reason: note.reason.clone(),
            created_at: note.created_at.clone(),
        })
    }

    async fn update(&mut self, note: &Note) -> Result<()> {
        self.gate.require(&["note-structure"])?;
        self.sql
            .execute(
                "UPDATE board.Notes SET Title = @title, Body = @body, Labels = @labels, SeenCount = @seenCount, SeenIn = @seenIn, State = @state, TicketId = @ticketId, Reason = @reason WHERE Id = @id",
                &[
                    ("id", MsValue::Int(note.id)),
                    ("title", MsValue::text(&note.title)),
                    ("body", MsValue::text(&note.body)),
                    ("labels", MsValue::text(note.labels.join(","))),
                    ("seenCount", MsValue::Int(note.seen_count)),
                    ("seenIn", MsValue::text(note.seen_in.join(","))),
                    ("state", MsValue::text(note.state.as_str())),
                    ("ticketId", MsValue::text_opt(note.ticket_id.as_deref())),
                    ("reason", MsValue::text_opt(note.reason.as_deref())),
                ],
            )
            .await?;
        Ok(())
    }
}
