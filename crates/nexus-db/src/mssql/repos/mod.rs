//! Die 17 Repositories auf SQL Server, je Trait eine Datei – wie in TypeScript je Klasse eine
//! Datei (`MssqlUserRepository.ts` …).
//!
//! Dieses Ticket (P2-11) legt nur die Gerüste an: jeder Zugriff meldet `not-implemented`.
//! Gefüllt werden sie von
//!
//! * **P2-12**: [`user`], [`project`], [`board`], [`ticket`], [`ticket_step`], [`ticket_session`]
//! * **P2-13**: [`comment`], [`note`], [`history`], [`deletion`], [`question`],
//!   [`question_comment`], [`question_history`], [`question_research`]
//! * **P2-14**: [`dispatch_run`], [`dispatch_stats`], [`search`]
//!
//! So können die drei Tickets parallel arbeiten, ohne sich in derselben Datei zu begegnen.

use nexus_core::{Error, HarnessError};

use crate::mssql::runner::{MsRow, MsValue};

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

/// Fehler eines Zugriffs, den dieses Ticket noch nicht umsetzt; `ticket` nennt das Ticket des
/// Migrationsplans, das ihn füllt
pub(crate) fn not_implemented(access: &str, ticket: &str) -> Error {
    HarnessError::new(
        "not-implemented",
        format!("Der Zugriff {access} auf SQL Server ist noch nicht umgesetzt ({ticket})."),
    )
    .into()
}

/// Parameter mit erzeugten Namen (IN-Listen, Statuslisten) in die Form, die der Runner nimmt.
///
/// Die Namen entstehen zur Laufzeit und müssen deshalb als `String` gehalten werden; der Runner
/// nimmt `&str`. Die Liste bleibt dabei in ihrer Reihenfolge.
pub(crate) fn borrow_params(params: &[(String, MsValue)]) -> Vec<(&str, MsValue)> {
    params
        .iter()
        .map(|(name, value)| (name.as_str(), value.clone()))
        .collect()
}

/// Die erste Zeile einer Abfrage, die genau eine liefern muss (`OUTPUT inserted.Id`)
pub(crate) fn first_row<'r>(rows: &'r [MsRow], access: &str) -> Result<&'r MsRow, Error> {
    rows.first().ok_or_else(|| {
        Error::unexpected_message(format!("Die Anweisung {access} hat keine Zeile geliefert."))
    })
}
