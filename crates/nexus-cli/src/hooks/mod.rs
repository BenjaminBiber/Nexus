//! Die Hooks der CLI: die Eingabe von Claude Code lesen, die Board-Laufzeit öffnen und den
//! Ablauf eines Ereignisses ausführen. Portiert aus `packages/cli/src/hooks`.

pub mod board_access;
pub mod runner;
pub mod stdin;

pub use board_access::{BoardAccess, BoardOpener, RuntimeOpener};
pub use runner::{HookRunner, Launcher, DEFAULT_BOARD_BUDGET_MS, DEFAULT_SERVER_START_MS};
pub use stdin::StdinReader;
