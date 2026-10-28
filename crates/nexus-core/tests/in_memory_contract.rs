//! Die drei gemeinsamen Verträge gegen die Speicher-Datenbank.
//!
//! Dieselben Makros laufen später gegen SQLite (P2-10) und SQL Server (P2-15); dort kommen
//! `reset` und – bei SQL Server – eine Sperre dazu.

nexus_core::board_database_contract!(in_memory, || async {
    nexus_core::board::in_memory::InMemoryBoardDatabase::new()
});

nexus_core::question_repository_contract!(in_memory_questions, || async {
    nexus_core::board::in_memory::InMemoryBoardDatabase::new()
});

nexus_core::dispatch_run_contract!(in_memory_runs, || async {
    nexus_core::board::in_memory::InMemoryBoardDatabase::new()
});
