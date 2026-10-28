//! Alles, was die Kommandozeile ausgibt: Ergebnisse, Fehlertexte, Hilfe.

pub mod error_text;
pub mod errors;
pub mod help;
pub mod printer;
pub mod wrap;

pub use error_text::CliErrorText;
pub use printer::Printer;
