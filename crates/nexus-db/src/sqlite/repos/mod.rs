//! Die 17 Repositories von SQLite, je eines in einer eigenen Datei.
//!
//! Jede Datei trägt genau eine `impl …Repository for SqliteUnitOfWork`. Dieses Ticket legt sie
//! als Gerüst an: alle Methoden melden `not-implemented`. Die Dateien sind damit die Grenze
//! zwischen den Tickets P2-6 bis P2-9, die sie nacheinander und parallel füllen – im Kopf jeder
//! Datei steht, welches Ticket sie übernimmt.

use nexus_core::{Error, HarnessError};

pub mod board;
pub mod comment;
pub mod deletion;
pub mod dispatch_run;
pub mod dispatch_stats;
pub mod history;
pub mod note;
pub mod project;
pub mod question;
pub mod question_comment;
pub mod question_history;
pub mod question_research;
pub mod search;
pub mod ticket;
pub mod ticket_session;
pub mod ticket_step;
pub mod user;

/// Der Fehler eines noch nicht gefüllten Zugriffs; `what` ist „Repository.methode“
pub(crate) fn folgt(what: &str) -> Error {
    HarnessError::new(
        "not-implemented",
        format!("SQLite: {what} folgt in P2-6…P2-9"),
    )
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ein_noch_leerer_zugriff_meldet_not_implemented() {
        let fehler = folgt("UserRepository.upsert");
        assert_eq!(fehler.code(), Some("not-implemented"));
        assert_eq!(
            fehler.to_string(),
            "SQLite: UserRepository.upsert folgt in P2-6…P2-9"
        );
    }
}
