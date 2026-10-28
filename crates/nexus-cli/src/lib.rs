//! Kommandozeile von Nexus: Befehlsbaum, deutsche Hilfe, deutsche Fehler und die Ausgabe.
//! Das Binary `nexus` in `src/main.rs` ist nur die Hülle darum; so lassen sich alle Teile
//! einzeln testen.

pub mod cli;
pub mod commands;
pub mod context;
pub mod output;

pub use cli::PROGRAM;
pub use context::CliContext;
pub use output::{CliErrorText, Printer};
