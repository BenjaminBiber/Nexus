//! Gemeinsame Helfer der Prozess-Tests der CLI.
//!
//! Jede Testdatei bindet die Helfer als eigenes Modul ein und braucht nur einen Teil davon;
//! ungenutzte Helfer sind deshalb kein Fehler (wie in `crates/nexus-server/tests/common`).
#![allow(dead_code)]

pub mod temp_project;
