//! Die Befehle der Kommandozeile. Die weiteren Fachbefehle (die Unterbefehle von `board`,
//! `hook`, `spec …`) kommen in den übrigen Tickets der Phase 3 dazu.

pub mod board;
pub mod customize;
pub mod dispatch_worker;
pub mod init;
pub mod install;
#[cfg(debug_assertions)]
pub mod selbsttest;
pub mod serve;
