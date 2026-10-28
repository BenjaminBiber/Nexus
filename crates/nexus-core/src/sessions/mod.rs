//! Sessions eines Projekts. Hier stehen die Nummern und der Sitzungsname; der Rest von
//! `packages/core/src/sessions` (Session, Speicher, Verlauf) folgt in einem späteren Ticket.

pub mod numbers;
pub mod title;

pub use numbers::SessionNumbers;
pub use title::SessionTitle;
