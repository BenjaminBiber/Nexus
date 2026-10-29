//! Prüfung des Schema-Stands einer Board-Datenbank; portiert aus
//! `packages/db/src/SchemaVersionGuard.ts`. Die Meldungen sind wortgleich zu TypeScript,
//! weil Tests, CLI und Oberfläche sie zeigen.

use nexus_core::board::BoardProvider;
use nexus_core::{HarnessError, Result};

/// Schema-Version, die dieser Stand von Nexus erwartet (`crates/nexus-db/sql/00N_….sql`)
pub const EXPECTED_SCHEMA_VERSION: i32 = 3;

/// Additive Erweiterungen (`board.SchemaFeatures`), die dieser Stand verlangt, mit ihrem Skript.
/// Sie erhöhen die Version nicht, damit ältere Stände mit der erweiterten DB weiterlaufen.
/// Die Reihenfolge ist die der Skripte und zugleich die der Meldungen.
pub const REQUIRED_SCHEMA_FEATURES: [(&str, &str); 4] = [
    ("dispatch-runs", "004_dispatch_runs.sql"),
    ("note-structure", "005_note_structure.sql"),
    ("dispatch-run-details", "006_dispatch_run_details.sql"),
    ("target-branch", "007_target_branch.sql"),
];

/// Prüft die Schema-Version der DB. Bei SQL Server spielt der Nutzer Versionsskripte von Hand ein,
/// bei SQLite legt Nexus sie beim Öffnen selbst an – die Meldungen unterscheiden sich deshalb
/// je Anbieter, die Prüfung und die Fehlercodes nicht.
#[derive(Debug, Clone, Copy, Default)]
pub struct SchemaVersionGuard;

impl SchemaVersionGuard {
    /// Meldet den Fehler mit Hinweis auf das passende Skript (SQL Server) bzw. auf die Datei
    /// (SQLite), wenn die Version nicht passt.
    ///
    /// Fehlende Erweiterungen melden keinen Fehler, sondern werden zurückgegeben: sie sind additiv,
    /// die übrigen Befehle laufen weiter und nur der Zugriff, der die neuen Spalten braucht,
    /// verweigert sich (siehe [`SchemaVersionGuard::feature_message`]).
    /// `database` ist bei SQLite der Dateipfad.
    pub fn check(
        &self,
        actual: Option<i32>,
        database: &str,
        features: &[String],
        provider: BoardProvider,
    ) -> Result<Vec<String>> {
        let sqlite = provider == BoardProvider::Sqlite;
        let Some(actual) = actual else {
            return Err(HarnessError::new(
                "board-schema-missing",
                if sqlite {
                    format!("In der SQLite-Datei {database} fehlt das Board-Schema, obwohl der Harness es beim Öffnen selbst anlegt. Die Datei ist vermutlich keine Board-Datei – unter Einstellungen → Board-Datenbank einen anderen Pfad wählen.")
                } else {
                    format!("In der Datenbank {database} fehlt das Schema „board“. Skript crates/nexus-db/sql/001_initial.sql ausführen.")
                },
            )
            .into());
        };
        if actual < EXPECTED_SCHEMA_VERSION {
            let next = format!("{:03}", actual + 1);
            return Err(HarnessError::new(
                "board-schema-outdated",
                if sqlite {
                    format!("SQLite-Datei {database} hat Schema-Version {actual}, erwartet {EXPECTED_SCHEMA_VERSION}. Der Harness spielt fehlende Stufen beim Öffnen selbst ein – Befehl bzw. Server neu starten; bleibt die Meldung, ist die Datei beschädigt.")
                } else {
                    format!("Datenbank {database} hat Schema-Version {actual}, erwartet {EXPECTED_SCHEMA_VERSION}. Skript crates/nexus-db/sql/{next}_….sql (und folgende) ausführen.")
                },
            )
            .into());
        }
        if actual > EXPECTED_SCHEMA_VERSION {
            return Err(HarnessError::new(
                "board-schema-newer",
                if sqlite {
                    format!("SQLite-Datei {database} hat Schema-Version {actual}, dieser Harness kennt nur {EXPECTED_SCHEMA_VERSION}. Im Harness-Ordner das Plugin neu bauen und „nexus install“ ausführen.")
                } else {
                    format!("Datenbank {database} hat Schema-Version {actual}, dieser Harness kennt nur {EXPECTED_SCHEMA_VERSION}. Im Harness-Ordner das Plugin neu bauen und „nexus install“ ausführen.")
                },
            )
            .into());
        }
        Ok(Self::missing(features))
    }

    /// Verlangte Erweiterungen, die in der DB fehlen – in der Reihenfolge von
    /// [`REQUIRED_SCHEMA_FEATURES`] (= Reihenfolge der Skripte)
    pub fn missing(features: &[String]) -> Vec<String> {
        REQUIRED_SCHEMA_FEATURES
            .iter()
            .filter(|(name, _)| !features.iter().any(|vorhanden| vorhanden == name))
            .map(|(name, _)| (*name).to_string())
            .collect()
    }

    /// Skript, das die Erweiterung einspielt; leer bei unbekanntem Namen
    pub fn script(feature: &str) -> &'static str {
        REQUIRED_SCHEMA_FEATURES
            .iter()
            .find(|(name, _)| *name == feature)
            .map_or("", |(_, script)| *script)
    }

    /// Meldung für einen Zugriff, der ohne die Erweiterung nicht geht.
    /// `database` ist bei SQLite der Dateipfad.
    pub fn feature_message(feature: &str, database: &str, provider: BoardProvider) -> String {
        if provider == BoardProvider::Sqlite {
            format!("Dafür fehlt der SQLite-Datei {database} die Erweiterung {feature}. Der Harness spielt sie beim Öffnen selbst ein – Befehl bzw. Server neu starten; bleibt die Meldung, ist die Datei beschädigt.")
        } else {
            format!(
                "Dafür fehlt der Datenbank {database} die Erweiterung {feature}. Skript crates/nexus-db/sql/{} ausführen (als Administrator). Die übrigen Board-Befehle laufen weiter.",
                Self::script(feature)
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nexus_core::board::BoardProvider::{Mssql, Sqlite};

    /// Alle verlangten Erweiterungen, wie sie eine vollständig eingespielte DB meldet
    fn features() -> Vec<String> {
        REQUIRED_SCHEMA_FEATURES
            .iter()
            .map(|(name, _)| (*name).to_string())
            .collect()
    }

    /// Meldungstext des erwarteten Fehlers, um Textbestandteile ausschließen zu können
    fn message(result: Result<Vec<String>>) -> String {
        result.unwrap_err().to_string()
    }

    #[test]
    fn akzeptiert_die_erwartete_version_mit_allen_erweiterungen() {
        let guard = SchemaVersionGuard;
        assert_eq!(
            guard
                .check(
                    Some(EXPECTED_SCHEMA_VERSION),
                    "AiHarness",
                    &features(),
                    Mssql
                )
                .unwrap(),
            Vec::<String>::new()
        );
    }

    #[test]
    fn nennt_das_startskript_wenn_das_schema_fehlt() {
        let guard = SchemaVersionGuard;
        let fehler = guard
            .check(None, "AiHarness", &features(), Mssql)
            .unwrap_err();
        assert_eq!(fehler.code(), Some("board-schema-missing"));
        assert!(fehler.to_string().contains("001_initial.sql"));
    }

    #[test]
    fn bleibt_bei_schema_version_3_damit_altere_stande_weiterlaufen() {
        assert_eq!(EXPECTED_SCHEMA_VERSION, 3);
        assert_eq!(
            REQUIRED_SCHEMA_FEATURES,
            [
                ("dispatch-runs", "004_dispatch_runs.sql"),
                ("note-structure", "005_note_structure.sql"),
                ("dispatch-run-details", "006_dispatch_run_details.sql"),
                ("target-branch", "007_target_branch.sql"),
            ]
        );
    }

    #[test]
    fn nennt_das_nachste_skript_bei_veralteter_db() {
        let guard = SchemaVersionGuard;
        let zwei = guard
            .check(Some(2), "AiHarness", &features(), Mssql)
            .unwrap_err();
        assert_eq!(zwei.code(), Some("board-schema-outdated"));
        assert!(zwei.to_string().contains("003_"));
        let eins = guard
            .check(Some(1), "AiHarness", &features(), Mssql)
            .unwrap_err();
        assert_eq!(eins.code(), Some("board-schema-outdated"));
        assert!(eins.to_string().contains("002_"));
    }

    #[test]
    fn meldet_fehlende_erweiterungen_statt_den_ganzen_zugriff_zu_sperren() {
        let guard = SchemaVersionGuard;
        assert_eq!(
            guard.check(Some(3), "AiHarness", &[], Mssql).unwrap(),
            [
                "dispatch-runs",
                "note-structure",
                "dispatch-run-details",
                "target-branch"
            ]
        );
        assert_eq!(
            guard
                .check(Some(3), "AiHarness", &["dispatch-runs".into()], Mssql)
                .unwrap(),
            ["note-structure", "dispatch-run-details", "target-branch"]
        );
        assert_eq!(
            guard
                .check(
                    Some(3),
                    "AiHarness",
                    &["dispatch-runs".into(), "note-structure".into()],
                    Mssql
                )
                .unwrap(),
            ["dispatch-run-details", "target-branch"]
        );
        let mit_unbekannter = [features(), vec!["unbekannt".into()]].concat();
        assert_eq!(
            guard
                .check(Some(3), "AiHarness", &mit_unbekannter, Mssql)
                .unwrap(),
            Vec::<String>::new()
        );
    }

    #[test]
    fn nennt_im_meldungstext_eines_gesperrten_zugriffs_datenbank_und_skript() {
        assert!(
            SchemaVersionGuard::feature_message("note-structure", "AiHarness", Mssql)
                .contains("crates/nexus-db/sql/005_note_structure.sql")
        );
        assert!(
            SchemaVersionGuard::feature_message("note-structure", "AiHarness", Mssql)
                .contains("AiHarness")
        );
        assert!(
            SchemaVersionGuard::feature_message("dispatch-runs", "AiHarness", Mssql)
                .contains("004_dispatch_runs.sql")
        );
        assert!(
            SchemaVersionGuard::feature_message("dispatch-run-details", "AiHarness", Mssql)
                .contains("crates/nexus-db/sql/006_dispatch_run_details.sql")
        );
        assert_eq!(
            SchemaVersionGuard::script("note-structure"),
            "005_note_structure.sql"
        );
        assert!(
            SchemaVersionGuard::feature_message("target-branch", "AiHarness", Mssql)
                .contains("crates/nexus-db/sql/007_target_branch.sql")
        );
        assert_eq!(SchemaVersionGuard::script("gibt-es-nicht"), "");
    }

    #[test]
    fn verlangt_ein_update_des_harness_bei_neuerer_db() {
        let guard = SchemaVersionGuard;
        let fehler = guard
            .check(
                Some(EXPECTED_SCHEMA_VERSION + 1),
                "AiHarness",
                &features(),
                Mssql,
            )
            .unwrap_err();
        assert_eq!(fehler.code(), Some("board-schema-newer"));
        assert!(fehler.to_string().contains("nexus install"));
    }

    mod bei_sqlite_ohne_hinweis_auf_die_sql_skripte {
        use super::*;

        const FILE: &str = "C:/x/board.db";

        #[test]
        fn akzeptiert_die_erwartete_version_mit_allen_erweiterungen() {
            let guard = SchemaVersionGuard;
            assert_eq!(
                guard
                    .check(Some(EXPECTED_SCHEMA_VERSION), FILE, &features(), Sqlite)
                    .unwrap(),
                Vec::<String>::new()
            );
        }

        #[test]
        fn nennt_die_datei_wenn_das_schema_fehlt() {
            let guard = SchemaVersionGuard;
            let fehler = guard.check(None, FILE, &features(), Sqlite).unwrap_err();
            assert_eq!(fehler.code(), Some("board-schema-missing"));
            assert!(fehler.to_string().contains(FILE));
            assert!(!message(guard.check(None, FILE, &features(), Sqlite))
                .contains("crates/nexus-db/sql"));
        }

        #[test]
        fn verweist_bei_veralteter_datei_auf_den_neustart() {
            let guard = SchemaVersionGuard;
            let fehler = guard.check(Some(2), FILE, &features(), Sqlite).unwrap_err();
            assert_eq!(fehler.code(), Some("board-schema-outdated"));
            assert!(fehler.to_string().contains("neu starten"));
            assert!(!message(guard.check(Some(2), FILE, &features(), Sqlite))
                .contains("crates/nexus-db/sql"));
        }

        #[test]
        fn nennt_die_fehlende_erweiterung_ohne_skriptnamen() {
            let guard = SchemaVersionGuard;
            assert_eq!(
                guard
                    .check(Some(EXPECTED_SCHEMA_VERSION), FILE, &[], Sqlite)
                    .unwrap(),
                [
                    "dispatch-runs",
                    "note-structure",
                    "dispatch-run-details",
                    "target-branch"
                ]
            );
            assert!(
                SchemaVersionGuard::feature_message("dispatch-runs", FILE, Sqlite)
                    .contains("dispatch-runs")
            );
            assert!(
                !SchemaVersionGuard::feature_message("dispatch-runs", FILE, Sqlite)
                    .contains("004_")
            );
        }

        #[test]
        fn verlangt_ein_update_des_harness_bei_neuerer_datei() {
            let guard = SchemaVersionGuard;
            let fehler = guard
                .check(Some(EXPECTED_SCHEMA_VERSION + 1), FILE, &features(), Sqlite)
                .unwrap_err();
            assert_eq!(fehler.code(), Some("board-schema-newer"));
            assert!(fehler.to_string().contains("nexus install"));
        }
    }
}
