//! Die Schema-Stufen der SQLite-Datei; portiert aus `packages/db/src/sqlite/schema/`.
//!
//! Der SQL-Text jeder Stufe steht zeichengenau in der Datei `00N_*.sql` daneben und wird mit
//! [`include_str!`] eingebunden. Das ist Vertrag: eine von TypeScript angelegte `board.db` und
//! eine von Nexus angelegte müssen in `sqlite_master` übereinstimmen, damit beide Stände
//! dieselbe Datei lesen und schreiben können. Kein Zeichen daran ändern, keine Stufe entfernen,
//! neue Stufen nur hinten anfügen.

/// Eine Schema-Stufe der SQLite-Datei; der Schema-Runner führt sie genau einmal in einer
/// Transaktion aus
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SqliteSchemaStep {
    /// Eindeutiger Name, gleich dem Dateinamen ohne Endung (z. B. `001_initial`)
    pub id: &'static str,
    /// Schema-Version, die der Runner nach der Stufe in `SchemaVersion` einträgt (Stufen 001–003)
    pub version: Option<i32>,
    /// Additive Erweiterung, die der Runner nach der Stufe in `SchemaFeatures` einträgt
    /// (z. B. `dispatch-runs`)
    pub feature: Option<&'static str>,
    /// DDL, Anweisungen mit `;` getrennt; ohne BEGIN/COMMIT und ohne Einträge in
    /// `SchemaVersion`/`SchemaFeatures`
    pub sql: &'static str,
}

/// Alle Schema-Stufen der SQLite-Datei in Ausführungsreihenfolge; neue Stufen nur hinten anfügen
pub const SQLITE_SCHEMA_STEPS: &[SqliteSchemaStep] = &[
    // Schema-Version 1: Nutzer, Projekte, Boards, Tickets, Labels, Voraussetzungen, Kommentare,
    // Verlauf, Notizen, Löschungen
    SqliteSchemaStep {
        id: "001_initial",
        version: Some(1),
        feature: None,
        sql: include_str!("001_initial.sql"),
    },
    // Schema-Version 2: Fragen mit Tags, betroffenen Tickets, Abhängigkeiten, Recherchen,
    // Optionen, Kommentaren und Verlauf
    SqliteSchemaStep {
        id: "002_questions",
        version: Some(2),
        feature: None,
        sql: include_str!("002_questions.sql"),
    },
    // Schema-Version 3: Harness-Felder am Ticket, Schritte, Sessions, Lesemarke an Kommentaren
    SqliteSchemaStep {
        id: "003_harness",
        version: Some(3),
        feature: None,
        sql: include_str!("003_harness.sql"),
    },
    // Erweiterung dispatch-runs; Version bleibt 3
    SqliteSchemaStep {
        id: "004_dispatch_runs",
        version: None,
        feature: Some("dispatch-runs"),
        sql: include_str!("004_dispatch_runs.sql"),
    },
    // Nur SQLite: Ersatz für rowversion (Zähler und Trigger) – weder Version noch Erweiterung
    SqliteSchemaStep {
        id: "005_rowver",
        version: None,
        feature: None,
        sql: include_str!("005_rowver.sql"),
    },
    // Erweiterung note-structure; Version bleibt 3
    SqliteSchemaStep {
        id: "006_note_structure",
        version: None,
        feature: Some("note-structure"),
        sql: include_str!("006_note_structure.sql"),
    },
    // Erweiterung dispatch-run-details; Version bleibt 3
    SqliteSchemaStep {
        id: "007_dispatch_run_details",
        version: None,
        feature: Some("dispatch-run-details"),
        sql: include_str!("007_dispatch_run_details.sql"),
    },
    // Erweiterung target-branch; Version bleibt 3
    SqliteSchemaStep {
        id: "008_target_branch",
        version: None,
        feature: Some("target-branch"),
        sql: include_str!("008_target_branch.sql"),
    },
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema_version_guard::{EXPECTED_SCHEMA_VERSION, REQUIRED_SCHEMA_FEATURES};

    #[test]
    fn die_stufen_stehen_in_der_reihenfolge_der_dateinamen() {
        let ids: Vec<&str> = SQLITE_SCHEMA_STEPS.iter().map(|s| s.id).collect();
        assert_eq!(
            ids,
            [
                "001_initial",
                "002_questions",
                "003_harness",
                "004_dispatch_runs",
                "005_rowver",
                "006_note_structure",
                "007_dispatch_run_details",
                "008_target_branch",
            ]
        );
    }

    #[test]
    fn die_stufen_erreichen_die_erwartete_version_und_alle_erweiterungen() {
        let hochste = SQLITE_SCHEMA_STEPS.iter().filter_map(|s| s.version).max();
        assert_eq!(hochste, Some(EXPECTED_SCHEMA_VERSION));

        let features: Vec<&str> = SQLITE_SCHEMA_STEPS.iter().filter_map(|s| s.feature).collect();
        let verlangt: Vec<&str> = REQUIRED_SCHEMA_FEATURES.iter().map(|(name, _)| *name).collect();
        assert_eq!(features, verlangt);
    }

    #[test]
    fn jede_stufe_bringt_sql_mit() {
        for step in SQLITE_SCHEMA_STEPS {
            assert!(!step.sql.trim().is_empty(), "{} ist leer", step.id);
            // Der Runner klammert die Stufe selbst in eine Transaktion
            let gross = step.sql.to_uppercase();
            assert!(!gross.contains("BEGIN IMMEDIATE"), "{}", step.id);
            assert!(!gross.contains("COMMIT"), "{}", step.id);
        }
    }
}
