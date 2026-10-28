//! Notizen eines Boards; portiert aus `packages/db/src/sqlite/SqliteNoteRepository.ts`.
//!
//! `Labels` und `SeenIn` stehen als kommagetrennter Text in der Spalte (Labels und Ticket-IDs
//! enthalten nie Kommas).

use std::str::FromStr;

use async_trait::async_trait;

use nexus_core::board::basics::NoteState;
use nexus_core::board::database::{NewNote, NoteFilter, NoteRepository};
use nexus_core::board::types::Note;
use nexus_core::{Error, Result};

use crate::sqlite::runner::{date, guid, iso_millis, missing_row};
use crate::sqlite::unit_of_work::SqliteUnitOfWork;
use crate::sqlite::{Params, SqlValue};
use crate::sqlite_params;

const COLUMNS: &str = "Id, BoardId, AuthorId, AuthorName, Title, Body, Source, Labels, SeenCount, SeenIn, State, TicketId, Reason, CreatedAt";

/// Parameternamen der Statusliste; mehr Stände als [`NoteState::ALL`] gibt es nicht
const STATE_NAMES: [&str; NoteState::ALL.len()] = ["s0", "s1", "s2"];

/// Kommagetrennter Text → Liste; leere Teile fallen weg (wie `split(',').filter(Boolean)`)
fn split_list(value: &str) -> Vec<String> {
    value
        .split(',')
        .filter(|part| !part.is_empty())
        .map(str::to_string)
        .collect()
}

/// Zeile → Notiz; ein unbekannter Stand ist ein Fehler, kein stiller Cast wie in TypeScript
fn map_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Result<Note>> {
    let state: String = row.get(10)?;
    let note = Note {
        id: row.get(0)?,
        board_id: row.get(1)?,
        author_id: guid(&row.get::<_, String>(2)?),
        author_name: row.get(3)?,
        title: row.get(4)?,
        body: row.get(5)?,
        source: row.get(6)?,
        labels: split_list(&row.get::<_, String>(7)?),
        seen_count: row.get(8)?,
        seen_in: split_list(&row.get::<_, String>(9)?),
        state: NoteState::Open,
        ticket_id: row.get(11)?,
        reason: row.get(12)?,
        created_at: iso_millis(&row.get::<_, String>(13)?),
    };
    Ok(NoteState::from_str(&state)
        .map_err(|_| {
            Error::unexpected_message(format!("Notiz {}: Stand „{state}“ ist unbekannt.", note.id))
        })
        .map(|state| Note { state, ..note }))
}

#[async_trait]
impl NoteRepository for SqliteUnitOfWork<'_> {
    /// Eine leere Statusliste ergibt keine Notizen; ohne Statusliste wird nicht nach Stand
    /// gefiltert
    async fn list(&mut self, filter: &NoteFilter) -> Result<Vec<Note>> {
        let states = filter.states.clone().unwrap_or_default();
        if filter.states.is_some() && states.is_empty() {
            return Ok(Vec::new());
        }
        let mut params: Params = sqlite_params! {
            "boardId": filter.board_id,
            "source": filter.source.clone(),
        };
        for (name, state) in STATE_NAMES.iter().zip(states.iter()) {
            params.push((name, SqlValue::from(state.as_str())));
        }
        let state_clause = if states.is_empty() {
            String::new()
        } else {
            let list = STATE_NAMES[..states.len()]
                .iter()
                .map(|name| format!("@{name}"))
                .collect::<Vec<_>>()
                .join(", ");
            format!("AND State IN ({list})")
        };
        let sql = format!(
            "SELECT {COLUMNS} FROM Notes WHERE BoardId = @boardId {state_clause} AND (@source IS NULL OR Source = @source) ORDER BY Id"
        );
        self.sql
            .query_rows(&sql, &params, map_row)?
            .into_iter()
            .collect()
    }

    /// `for_update` entfällt: `BEGIN IMMEDIATE` sperrt die ganze Datei
    async fn by_id(&mut self, id: i32, _for_update: bool) -> Result<Option<Note>> {
        let sql = format!("SELECT {COLUMNS} FROM Notes WHERE Id = @id");
        let zeilen = self
            .sql
            .query_rows(&sql, &sqlite_params! { "id": id }, map_row)?;
        zeilen.into_iter().next().transpose()
    }

    async fn insert(&mut self, note: &NewNote) -> Result<Note> {
        let params: Params = sqlite_params! {
            "boardId": note.board_id,
            "authorId": note.author_id.to_lowercase(),
            "authorName": note.author_name.clone(),
            "title": note.title.clone(),
            "body": note.body.clone(),
            "source": note.source.clone(),
            "labels": note.labels.join(","),
            "seenCount": note.seen_count,
            "seenIn": note.seen_in.join(","),
            "state": note.state.as_str(),
            "ticketId": note.ticket_id.clone(),
            "reason": note.reason.clone(),
            "createdAt": date(&note.created_at),
        };
        let ids = self.sql.query_rows(
            "INSERT INTO Notes (BoardId, AuthorId, AuthorName, Title, Body, Source, Labels, SeenCount, SeenIn, State, TicketId, Reason, CreatedAt)
       VALUES (@boardId, @authorId, @authorName, @title, @body, @source, @labels, @seenCount, @seenIn, @state, @ticketId, @reason, @createdAt) RETURNING Id",
            &params,
            |row| row.get::<_, i32>(0),
        )?;
        let id = ids
            .into_iter()
            .next()
            .ok_or_else(|| missing_row("Notes.insert"))?;
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
        let params: Params = sqlite_params! {
            "id": note.id,
            "title": note.title.clone(),
            "body": note.body.clone(),
            "labels": note.labels.join(","),
            "seenCount": note.seen_count,
            "seenIn": note.seen_in.join(","),
            "state": note.state.as_str(),
            "ticketId": note.ticket_id.clone(),
            "reason": note.reason.clone(),
        };
        self.sql.execute(
            "UPDATE Notes SET Title = @title, Body = @body, Labels = @labels, SeenCount = @seenCount, SeenIn = @seenIn, State = @state, TicketId = @ticketId, Reason = @reason WHERE Id = @id",
            &params,
        )?;
        Ok(())
    }
}
