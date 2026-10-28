#![allow(dead_code)]
//! Gemeinsame Testhelfer der SQLite-Tests; portiert aus
//! `packages/db/tests/sqlite/helpers/`.
//!
//! Jede Testdatei bindet nur ein, was sie braucht – ungenutzte Helfer sind deshalb erlaubt.

pub mod schema_inspect;
pub mod temp_sqlite;
