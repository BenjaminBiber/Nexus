//! Die Befehle der Kommandozeile. Die übrigen Fachbefehle (`board …`, `hook`, `spec …`) kommen
//! in P3-15 bis P3-18 dazu.

pub mod init;
pub mod install;

#[cfg(debug_assertions)]
pub mod selbsttest;
