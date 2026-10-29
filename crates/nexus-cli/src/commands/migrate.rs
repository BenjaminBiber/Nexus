//! `nexus migrate`: holt die Daten aus dem alten Ordner `~/.harness` in den neuen Ordner
//! `~/.nexus`. Der alte Ordner bleibt unverändert liegen, damit die alte Fassung weiterläuft.

use std::path::Path;

use nexus_core::clock::Clock;
use nexus_core::migrate::{MigrateDeps, MigrateReport, MigrateStatus, Migrator};
use nexus_core::registry::{harness_dir, nexus_dir, process_lookup};
use nexus_core::Result;

use crate::context::CliContext;
use crate::output::Printer;
use crate::server::{PortState, ServerLauncher};

/// Beschreibung von `migrate` in der Hilfe
pub const MIGRATE_ABOUT: &str = "Holt die Daten aus dem alten Ordner ~/.harness nach ~/.nexus. Der alte Ordner bleibt unverändert liegen.";

/// Beispiele unter der Hilfe von `migrate`
pub const MIGRATE_EXAMPLES: &str = "\nBeispiele:\n  nexus migrate\n  nexus migrate --dry-run\n  nexus migrate --from /pfad/zu/.harness\n\nLäuft noch ein Server, zuerst `nexus stop`.";

/// Hilfe der Option `--from`
pub const FROM_HELP: &str = "Alter Ordner (Standard: HARNESS_HOME, sonst ~/.harness)";

/// Hilfe der Option `--dry-run`
pub const DRY_RUN_HELP: &str = "Nur auflisten, was kopiert würde";

/// Prüft über den Health-Aufruf, ob unter einem Port noch ein Server von Nexus antwortet
struct LauncherDeps(ServerLauncher);

impl MigrateDeps for LauncherDeps {
    fn is_ours(&self, port: u16) -> bool {
        self.0.probe(port) == PortState::Ours
    }
}

/// Führt `migrate` aus: kopiert (oder listet mit `--dry-run` nur auf) und meldet das Ergebnis
pub fn run(context: &CliContext, from: Option<&Path>, dry_run: bool) -> Result<u8> {
    let from = from
        .map(Path::to_path_buf)
        .unwrap_or_else(|| harness_dir(process_lookup));
    let to = nexus_dir(process_lookup);
    let report = migrator().run(&from, &to, dry_run)?;
    context.printer().result(&report, &lines(&report));
    Ok(0)
}

/// Kopiert den Ordner, wenn die Auflösung noch auf `~/.harness` zurückgefallen ist. Läuft ein
/// Server, bleibt es beim Rückfall und es erscheint ein Hinweis auf stderr. Fehler bremsen den
/// aufgerufenen Befehl nicht: er arbeitet dann weiter auf dem alten Ordner.
pub fn migrate_on_first_run(printer: &Printer) {
    let from = harness_dir(process_lookup);
    let to = nexus_dir(process_lookup);
    match migrator().run(&from, &to, false) {
        Ok(report) if report.status == MigrateStatus::Migrated => {
            eprintln!(
                "Die Daten sind von {} nach {} umgezogen. Der alte Ordner bleibt liegen.",
                report.from, report.to
            );
        }
        Ok(_) => {}
        Err(error) if error.code() == Some("server-running") => {
            printer.warn(
                "Der Server läuft noch, deshalb ist der Umzug nach ~/.nexus verschoben. Bitte `nexus stop`, dann `nexus migrate`.",
            );
        }
        Err(error) => printer.warn(&format!(
            "Der Umzug nach ~/.nexus ist nicht gelaufen: {error}"
        )),
    }
}

/// Migrator mit der Server-Prüfung dieses Prozesses, der Uhr des Rechners und der Fassung
fn migrator() -> Migrator<LauncherDeps> {
    Migrator::new(
        LauncherDeps(ServerLauncher::from_process_env()),
        Clock::system(),
        env!("CARGO_PKG_VERSION"),
    )
}

/// Die Textzeilen zum Ergebnis
fn lines(report: &MigrateReport) -> Vec<String> {
    match report.status {
        MigrateStatus::AlreadyMigrated => vec![format!("Schon migriert: {}.", report.to)],
        MigrateStatus::NothingToDo => vec![format!(
            "Es gibt nichts zu migrieren: {} fehlt.",
            report.from
        )],
        MigrateStatus::DryRun => {
            let mut lines = vec![format!(
                "{} würde nach {} kopiert ({}):",
                report.from,
                report.to,
                anzahl(report.files.len())
            )];
            lines.extend(report.files.iter().map(|file| format!("  {file}")));
            lines
        }
        MigrateStatus::Migrated => {
            let mut lines = vec![format!(
                "{} nach {} kopiert ({}). Der alte Ordner bleibt unverändert liegen.",
                report.from,
                report.to,
                anzahl(report.files.len())
            )];
            if report.board_config_rewritten {
                lines.push(
                    "Der Pfad der Board-Datenbank in board.yaml zeigt jetzt auf den neuen Ordner."
                        .to_string(),
                );
            }
            lines
        }
    }
}

/// „1 Datei“ bzw. „n Dateien“
fn anzahl(count: usize) -> String {
    if count == 1 {
        "1 Datei".to_string()
    } else {
        format!("{count} Dateien")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ein Ergebnis mit diesem Zustand
    fn report(status: MigrateStatus, files: &[&str], rewritten: bool) -> MigrateReport {
        MigrateReport {
            status,
            from: "/h/.harness".to_string(),
            to: "/h/.nexus".to_string(),
            files: files.iter().map(ToString::to_string).collect(),
            board_config_rewritten: rewritten,
        }
    }

    #[test]
    fn meldet_den_umzug_mit_der_zahl_der_dateien() {
        assert_eq!(
            lines(&report(MigrateStatus::Migrated, &["board.db"], false)),
            vec!["/h/.harness nach /h/.nexus kopiert (1 Datei). Der alte Ordner bleibt unverändert liegen."]
        );
    }

    #[test]
    fn nennt_die_umgeschriebene_board_konfiguration() {
        let lines = lines(&report(MigrateStatus::Migrated, &["board.db"], true));
        assert_eq!(
            lines.last().unwrap(),
            "Der Pfad der Board-Datenbank in board.yaml zeigt jetzt auf den neuen Ordner."
        );
    }

    #[test]
    fn ein_schon_migrierter_ordner_wird_gemeldet() {
        assert_eq!(
            lines(&report(MigrateStatus::AlreadyMigrated, &[], false)),
            vec!["Schon migriert: /h/.nexus."]
        );
    }

    #[test]
    fn ohne_quelle_gibt_es_nichts_zu_migrieren() {
        assert_eq!(
            lines(&report(MigrateStatus::NothingToDo, &[], false)),
            vec!["Es gibt nichts zu migrieren: /h/.harness fehlt."]
        );
    }

    #[test]
    fn dry_run_listet_die_dateien_auf() {
        assert_eq!(
            lines(&report(
                MigrateStatus::DryRun,
                &["board.db", "projects.json"],
                false
            )),
            vec![
                "/h/.harness würde nach /h/.nexus kopiert (2 Dateien):".to_string(),
                "  board.db".to_string(),
                "  projects.json".to_string()
            ]
        );
    }
}
