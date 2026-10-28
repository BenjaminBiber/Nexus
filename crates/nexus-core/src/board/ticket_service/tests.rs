//! Tests der Ticket-Dienste; portiert aus `packages/core/tests/unit/board/`.
//! Je Modul eine der Testdateien aus TypeScript.

mod claim;
mod harness_fields;
mod move_to_board;
mod question_blocking;
mod steps;
mod tickets;

use crate::board::ticket_service::{NewTicketInput, TicketPatch, TicketView, WriteOptions};
use crate::error::Result;
use crate::testing::BoardFixture;

/// Legt ein Ticket mit diesem Titel auf dem Board „Sprint“ an
async fn add(f: &BoardFixture, title: &str) -> TicketView {
    create(f, NewTicketInput::titled(title)).await.unwrap()
}

/// Legt ein Ticket auf dem Board „Sprint“ an, ohne den Fehler zu verschweigen
async fn create(f: &BoardFixture, input: NewTicketInput) -> Result<TicketView> {
    f.tickets
        .create(f.board_id, &input, &f.actor, &WriteOptions::default())
        .await
}

/// Ändert ein Ticket mit dem Handelnden der Umgebung
async fn set(f: &BoardFixture, id: &str, patch: TicketPatch) -> Result<TicketView> {
    f.tickets
        .update(id, &patch, &f.actor, &WriteOptions::default())
        .await
}

/// Die Felder des Verlaufs eines Tickets als (Feld, Von, Nach)
async fn history(f: &BoardFixture, id: &str) -> Vec<(String, Option<String>, Option<String>)> {
    f.tickets
        .get(id)
        .await
        .unwrap()
        .history
        .into_iter()
        .map(|entry| (entry.field, entry.from, entry.to))
        .collect()
}
