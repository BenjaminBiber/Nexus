//! Alles, was die Kommandozeile ausgibt: Ergebnisse, Fehlertexte, Hilfe.

pub mod dispatch_status_formatter;
pub mod error_text;
pub mod errors;
pub mod help;
pub mod printer;
pub mod wrap;

pub use dispatch_status_formatter::{DispatchStatusFormatter, DispatchStatusView};
pub use error_text::CliErrorText;
pub use printer::Printer;
