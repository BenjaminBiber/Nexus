//! Die Befehle der Kommandozeile. Die weiteren Fachbefehle (die übrigen Unterbefehle von
//! `board` und `hook`) kommen in den restlichen Tickets der Phase 3 dazu.

pub mod board;
pub mod customize;
pub mod dispatch_worker;
pub mod hook;
pub mod init;
pub mod install;
#[cfg(debug_assertions)]
pub mod selbsttest;
pub mod serve;
pub mod spec;
pub mod stop;
