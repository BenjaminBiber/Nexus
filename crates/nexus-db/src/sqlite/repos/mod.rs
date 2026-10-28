//! Die 17 Repositories von SQLite, je eines in einer eigenen Datei.
//!
//! Jede Datei trägt genau eine `impl …Repository for SqliteUnitOfWork`. Die Dateien waren die
//! Grenze zwischen den Tickets P2-6 bis P2-9, die sie nacheinander und parallel gefüllt haben –
//! im Kopf jeder Datei steht, welches Ticket sie übernommen hat.

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
