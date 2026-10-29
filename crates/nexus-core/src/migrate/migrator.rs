//! Kopiert die Daten des alten Ordners `~/.harness` nach `~/.nexus`. Erst wird in
//! `<ziel>.migrating` kopiert, danach umbenannt – bricht der Umzug ab, bleibt kein halber
//! Zielordner stehen. Die Quelle wird nur gelesen.

use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::board::config::BoardProvider;
use crate::board::BoardConfigStore;
use crate::clock::Clock;
use crate::error::{Error, HarnessError, Result};
use crate::locking::AtomicWriter;
use crate::registry::{HarnessHome, ServerInfoFile};

/// Ordner, die nicht mitkommen: die eingebaute App und die Startdateien legt `nexus install`
/// selbst wieder an
pub const SKIP_DIRS: [&str; 2] = ["app", "bin"];

/// Dateien, die nicht mitkommen: die Angaben des laufenden Servers gehören dem alten Prozess
pub const SKIP_FILES: [&str; 1] = ["server.json"];

/// Endungen, die nicht mitkommen: Sperren und halbe Dateien eines atomaren Schreibvorgangs
const SKIP_EXTENSIONS: [&str; 2] = ["lock", "tmp"];

/// Name der Datei, die den Umzug festhält
pub const MIGRATED_FROM_FILE: &str = "migrated-from.json";

/// Endung des Ordners, in den zuerst kopiert wird
const MIGRATING_SUFFIX: &str = ".migrating";

/// Was der Migrator von außen braucht: die Prüfung, ob zu einem Ordner noch ein Server läuft.
/// Maßgeblich ist der Health-Aufruf; `server.json` ist nur der Hinweis, wo er zu erreichen ist.
pub trait MigrateDeps {
    /// Antwortet unter diesem Port ein Nexus-Server (`GET /api/health` mit `name == "ai-harness"`)?
    fn is_ours(&self, port: u16) -> bool;
}

/// Wie der Umzug ausgegangen ist
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum MigrateStatus {
    /// Die Daten sind kopiert
    Migrated,
    /// Das Ziel hat schon Inhalt; es wird nichts angefasst
    AlreadyMigrated,
    /// Es gibt keine Quelle, aus der etwas zu holen wäre
    NothingToDo,
    /// Nur aufgelistet (`--dry-run`)
    DryRun,
}

/// Das Ergebnis des Umzugs (Text und `--json`)
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MigrateReport {
    pub status: MigrateStatus,
    pub from: String,
    pub to: String,
    /// Die kopierten (bzw. bei `--dry-run` die zu kopierenden) Pfade, relativ zur Quelle,
    /// mit `/` getrennt und alphabetisch sortiert
    pub files: Vec<String>,
    /// Wurde der Pfad der SQLite-Datei in `board.yaml` auf den neuen Ordner umgeschrieben?
    pub board_config_rewritten: bool,
}

/// Inhalt von `migrated-from.json`
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct MigratedFrom {
    from: String,
    at: String,
    version: String,
}

/// Kopiert den alten Ordner in den neuen.
pub struct Migrator<D: MigrateDeps> {
    deps: D,
    clock: Clock,
    version: String,
}

impl<D: MigrateDeps> Migrator<D> {
    /// Migrator mit eigener Server-Prüfung, Uhr und Fassung (steht in `migrated-from.json`)
    pub fn new(deps: D, clock: Clock, version: impl Into<String>) -> Self {
        Self {
            deps,
            clock,
            version: version.into(),
        }
    }

    /// Führt den Umzug aus. `dry_run` listet nur auf und schreibt nichts.
    pub fn run(&self, from: &Path, to: &Path, dry_run: bool) -> Result<MigrateReport> {
        let report = |status, files, rewritten| MigrateReport {
            status,
            from: from.display().to_string(),
            to: to.display().to_string(),
            files,
            board_config_rewritten: rewritten,
        };
        if has_content(to) {
            return Ok(report(MigrateStatus::AlreadyMigrated, Vec::new(), false));
        }
        if !from.is_dir() {
            return Ok(report(MigrateStatus::NothingToDo, Vec::new(), false));
        }
        self.require_no_server(from)?;
        self.require_no_server(to)?;
        let mut files = Vec::new();
        collect(from, Path::new(""), &mut files)?;
        files.sort();
        if dry_run {
            return Ok(report(MigrateStatus::DryRun, files, false));
        }
        let staging = staging_dir(to);
        if staging.exists() {
            fs::remove_dir_all(&staging).map_err(Error::unexpected)?;
        }
        let result = self.fill(from, to, &staging, &files);
        if result.is_err() {
            let _ = fs::remove_dir_all(&staging);
        }
        let rewritten = result?;
        if let Some(parent) = to.parent() {
            fs::create_dir_all(parent).map_err(Error::unexpected)?;
        }
        fs::rename(&staging, to).map_err(Error::unexpected)?;
        Ok(report(MigrateStatus::Migrated, files, rewritten))
    }

    /// Füllt den Zwischenordner: Dateien kopieren, `board.yaml` nachziehen, Umzug festhalten.
    /// Liefert, ob der Pfad der SQLite-Datei umgeschrieben wurde.
    fn fill(&self, from: &Path, to: &Path, staging: &Path, files: &[String]) -> Result<bool> {
        fs::create_dir_all(staging).map_err(Error::unexpected)?;
        for relative in files {
            let source = join_relative(from, relative);
            let target = join_relative(staging, relative);
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent).map_err(Error::unexpected)?;
            }
            fs::copy(&source, &target).map_err(Error::unexpected)?;
        }
        let rewritten = rewrite_board_config(from, to, staging)?;
        let json = serde_json::to_string_pretty(&MigratedFrom {
            from: from.display().to_string(),
            at: self.clock.iso(),
            version: self.version.clone(),
        })
        .map_err(Error::unexpected)?;
        AtomicWriter::new().write(&staging.join(MIGRATED_FROM_FILE), &format!("{json}\n"))?;
        Ok(rewritten)
    }

    /// Bricht mit `server-running` ab, wenn zu diesem Ordner noch ein Server läuft
    fn require_no_server(&self, home: &Path) -> Result<()> {
        let info = ServerInfoFile::new(HarnessHome::at(home));
        let Some(info) = info.read() else {
            return Ok(());
        };
        if !self.deps.is_ours(info.port) {
            return Ok(());
        }
        Err(HarnessError::new(
            "server-running",
            "Der Server läuft noch. Bitte zuerst `nexus stop`, dann `nexus migrate`.",
        )
        .into())
    }
}

/// Der Zwischenordner liegt neben dem Ziel, damit das Umbenennen auf derselben Platte bleibt
fn staging_dir(to: &Path) -> PathBuf {
    let mut name = to.as_os_str().to_os_string();
    name.push(MIGRATING_SUFFIX);
    PathBuf::from(name)
}

/// Hat der Ordner Inhalt? Ein leerer Ordner zählt wie keiner.
fn has_content(dir: &Path) -> bool {
    fs::read_dir(dir).is_ok_and(|mut entries| entries.next().is_some())
}

/// Sammelt alle zu kopierenden Dateien unter `dir` als Pfade relativ zur Quelle
fn collect(root: &Path, relative: &Path, files: &mut Vec<String>) -> Result<()> {
    let entries = fs::read_dir(root.join(relative)).map_err(Error::unexpected)?;
    for entry in entries {
        let entry = entry.map_err(Error::unexpected)?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let path = relative.join(&name);
        let kind = entry.file_type().map_err(Error::unexpected)?;
        if kind.is_dir() {
            if relative.as_os_str().is_empty() && SKIP_DIRS.contains(&name.as_str()) {
                continue;
            }
            collect(root, &path, files)?;
            continue;
        }
        if skip_file(&name) {
            continue;
        }
        files.push(path.to_string_lossy().replace('\\', "/"));
    }
    Ok(())
}

/// Bleibt die Datei zurück? Angaben des Servers, Sperren und halbe Dateien kommen nicht mit.
fn skip_file(name: &str) -> bool {
    if SKIP_FILES.contains(&name) {
        return true;
    }
    SKIP_EXTENSIONS
        .iter()
        .any(|extension| name.ends_with(&format!(".{extension}")))
}

/// Pfad aus einem mit `/` getrennten relativen Pfad
fn join_relative(root: &Path, relative: &str) -> PathBuf {
    relative
        .split('/')
        .fold(root.to_path_buf(), |path, part| path.join(part))
}

/// Schreibt den Pfad der SQLite-Datei in `board.yaml` auf den neuen Ordner um, wenn er in den
/// alten zeigt. Geschrieben wird im Zwischenordner, der Pfad zeigt aber schon auf das Ziel.
/// Andere Pfade und andere Anbieter bleiben unberührt.
fn rewrite_board_config(from: &Path, to: &Path, staging: &Path) -> Result<bool> {
    let store = BoardConfigStore::new(HarnessHome::at(staging));
    // Eine kaputte Konfig hält den Umzug nicht auf; sie wird mitkopiert wie sie ist
    let Ok(Some(mut config)) = store.read() else {
        return Ok(false);
    };
    if config.provider != BoardProvider::Sqlite {
        return Ok(false);
    }
    let Some(rest) = relative_to(Path::new(&config.file), from) else {
        return Ok(false);
    };
    config.file = to.join(rest).to_string_lossy().into_owned();
    store.write(&config)?;
    Ok(true)
}

/// Der Teil von `path` unterhalb von `base`; `None`, wenn `path` nicht darin liegt.
/// Unter Windows wird die Schreibweise wie im TypeScript-Code ohne Groß-/Kleinschreibung
/// verglichen.
fn relative_to(path: &Path, base: &Path) -> Option<PathBuf> {
    #[cfg(windows)]
    {
        let lower = |value: &Path| PathBuf::from(value.to_string_lossy().to_lowercase());
        if lower(path).strip_prefix(lower(base)).is_err() {
            return None;
        }
        // Die Teile kommen aus dem ursprünglichen Pfad, damit die Schreibweise erhalten bleibt
        let base_parts = base.components().count();
        return Some(path.components().skip(base_parts).collect());
    }
    #[cfg(not(windows))]
    path.strip_prefix(base).ok().map(Path::to_path_buf)
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;
    use crate::registry::{ServerInfo, ServerInfoFile};

    /// Server-Prüfung der Tests: diese Ports gelten als eigener Server
    struct TestDeps(HashSet<u16>);

    impl MigrateDeps for TestDeps {
        fn is_ours(&self, port: u16) -> bool {
            self.0.contains(&port)
        }
    }

    /// Migrator mit fester Zeit und fester Fassung
    fn migrator(laufende_ports: &[u16]) -> Migrator<TestDeps> {
        Migrator::new(
            TestDeps(laufende_ports.iter().copied().collect()),
            Clock::fixed("2026-10-29T12:00:00.000Z".parse().unwrap()),
            "1.2.3",
        )
    }

    /// Eine Datei mit Inhalt unter dem Ordner; fehlende Oberordner entstehen mit
    fn write(dir: &Path, relative: &str, content: &str) {
        let file = join_relative(dir, relative);
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(file, content).unwrap();
    }

    /// Quelle und Ziel in einem Temp-Ordner (`alt` und `neu` nebeneinander)
    fn ordner() -> (tempfile::TempDir, PathBuf, PathBuf) {
        let base = tempfile::tempdir().unwrap();
        let from = base.path().join("alt");
        let to = base.path().join("neu");
        fs::create_dir(&from).unwrap();
        (base, from, to)
    }

    #[test]
    fn kopiert_alle_daten_und_haelt_den_umzug_fest() {
        let (_base, from, to) = ordner();
        write(&from, "projects.json", "[]\n");
        write(&from, "dispatch/state.json", "{}\n");
        write(&from, "dispatch/runs/r1.json", "{}\n");

        let report = migrator(&[]).run(&from, &to, false).unwrap();

        assert_eq!(report.status, MigrateStatus::Migrated);
        assert_eq!(
            report.files,
            vec![
                "dispatch/runs/r1.json".to_string(),
                "dispatch/state.json".to_string(),
                "projects.json".to_string()
            ]
        );
        assert_eq!(
            fs::read_to_string(to.join("dispatch").join("state.json")).unwrap(),
            "{}\n"
        );
        let json: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(to.join(MIGRATED_FROM_FILE)).unwrap())
                .unwrap();
        assert_eq!(json["from"], from.display().to_string());
        assert_eq!(json["at"], "2026-10-29T12:00:00.000Z");
        assert_eq!(json["version"], "1.2.3");
    }

    #[test]
    fn nimmt_die_board_datenbank_mit_wal_und_shm_mit() {
        let (_base, from, to) = ordner();
        write(&from, "board.db", "db");
        write(&from, "board.db-wal", "wal");
        write(&from, "board.db-shm", "shm");

        let report = migrator(&[]).run(&from, &to, false).unwrap();

        assert_eq!(
            report.files,
            vec![
                "board.db".to_string(),
                "board.db-shm".to_string(),
                "board.db-wal".to_string()
            ]
        );
        assert_eq!(fs::read_to_string(to.join("board.db-wal")).unwrap(), "wal");
        assert_eq!(fs::read_to_string(to.join("board.db-shm")).unwrap(), "shm");
    }

    #[test]
    fn laesst_app_bin_server_json_sperren_und_halbe_dateien_zurueck() {
        let (_base, from, to) = ordner();
        write(&from, "app/nexus.app", "app");
        write(&from, "bin/nexus", "start");
        write(&from, "server.json", "{}");
        write(&from, "projects.lock", "");
        write(&from, "projects.json.1234.ab.tmp", "halb");
        write(&from, "projects.json", "[]\n");

        let report = migrator(&[]).run(&from, &to, false).unwrap();

        assert_eq!(report.files, vec!["projects.json".to_string()]);
        assert!(!to.join("app").exists());
        assert!(!to.join("bin").exists());
        assert!(!to.join("server.json").exists());
    }

    #[test]
    fn laesst_die_quelle_unveraendert() {
        let (_base, from, to) = ordner();
        write(&from, "projects.json", "[]\n");
        write(&from, "board.yaml", "provider: sqlite\n");
        let vorher = pruefsumme(&from);

        migrator(&[]).run(&from, &to, false).unwrap();

        assert_eq!(pruefsumme(&from), vorher);
    }

    #[test]
    fn ein_ziel_mit_inhalt_wird_nicht_angefasst() {
        let (_base, from, to) = ordner();
        write(&from, "projects.json", "[]\n");
        write(&to, "projects.json", "schon da\n");

        let report = migrator(&[]).run(&from, &to, false).unwrap();

        assert_eq!(report.status, MigrateStatus::AlreadyMigrated);
        assert!(report.files.is_empty());
        assert_eq!(
            fs::read_to_string(to.join("projects.json")).unwrap(),
            "schon da\n"
        );
    }

    #[test]
    fn ohne_quelle_gibt_es_nichts_zu_tun() {
        let (_base, from, to) = ordner();
        fs::remove_dir(&from).unwrap();

        let report = migrator(&[]).run(&from, &to, false).unwrap();

        assert_eq!(report.status, MigrateStatus::NothingToDo);
        assert!(!to.exists());
    }

    #[test]
    fn ein_laufender_server_bricht_mit_server_running_ab() {
        let (_base, from, to) = ordner();
        write(&from, "projects.json", "[]\n");
        ServerInfoFile::new(HarnessHome::at(&from))
            .write(&ServerInfo {
                pid: 4242,
                port: 4710,
                started_at: "2026-10-29T11:00:00.000Z".to_string(),
            })
            .unwrap();

        let error = migrator(&[4710]).run(&from, &to, false).unwrap_err();

        assert_eq!(error.code(), Some("server-running"));
        assert_eq!(
            error.to_string(),
            "Der Server läuft noch. Bitte zuerst `nexus stop`, dann `nexus migrate`."
        );
        assert!(!to.exists());
    }

    #[test]
    fn eine_veraltete_server_json_haelt_den_umzug_nicht_auf() {
        let (_base, from, to) = ordner();
        write(&from, "projects.json", "[]\n");
        ServerInfoFile::new(HarnessHome::at(&from))
            .write(&ServerInfo {
                pid: 4242,
                port: 4710,
                started_at: "2026-10-29T11:00:00.000Z".to_string(),
            })
            .unwrap();

        // Auf dem Port antwortet niemand: die Angaben sind alt
        let report = migrator(&[]).run(&from, &to, false).unwrap();

        assert_eq!(report.status, MigrateStatus::Migrated);
    }

    #[test]
    fn schreibt_den_pfad_der_sqlite_datei_auf_den_neuen_ordner_um() {
        let (_base, from, to) = ordner();
        let config = format!(
            "provider: sqlite\nfile: {}\nuserId: 11111111-1111-1111-1111-111111111111\nuserName: Benjamin\n",
            from.join("board.db").display()
        );
        write(&from, "board.yaml", &config);
        write(&from, "board.db", "db");

        let report = migrator(&[]).run(&from, &to, false).unwrap();

        assert!(report.board_config_rewritten);
        let geschrieben = BoardConfigStore::new(HarnessHome::at(&to))
            .read()
            .unwrap()
            .unwrap();
        assert_eq!(
            geschrieben.file,
            to.join("board.db").to_string_lossy().into_owned()
        );
    }

    #[test]
    fn ein_pfad_ausserhalb_des_alten_ordners_bleibt_stehen() {
        let (_base, from, to) = ordner();
        let config = "provider: sqlite\nfile: /woanders/board.db\nuserId: 11111111-1111-1111-1111-111111111111\nuserName: Benjamin\n";
        write(&from, "board.yaml", config);

        let report = migrator(&[]).run(&from, &to, false).unwrap();

        assert!(!report.board_config_rewritten);
        assert_eq!(
            BoardConfigStore::new(HarnessHome::at(&to))
                .read()
                .unwrap()
                .unwrap()
                .file,
            "/woanders/board.db"
        );
    }

    #[test]
    fn dry_run_listet_nur_auf_und_schreibt_nichts() {
        let (_base, from, to) = ordner();
        write(&from, "projects.json", "[]\n");

        let report = migrator(&[]).run(&from, &to, true).unwrap();

        assert_eq!(report.status, MigrateStatus::DryRun);
        assert_eq!(report.files, vec!["projects.json".to_string()]);
        assert!(!to.exists());
        assert!(!staging_dir(&to).exists());
    }

    #[test]
    fn nach_dem_umzug_bleibt_kein_zwischenordner_stehen() {
        let (_base, from, to) = ordner();
        write(&from, "projects.json", "[]\n");

        migrator(&[]).run(&from, &to, false).unwrap();

        assert!(!staging_dir(&to).exists());
    }

    /// Alle Dateien eines Ordners mit ihrem Inhalt, sortiert – die Prüfsumme der Tests
    fn pruefsumme(dir: &Path) -> Vec<(String, String)> {
        let mut files = Vec::new();
        collect(dir, Path::new(""), &mut files).unwrap();
        files.sort();
        files
            .into_iter()
            .map(|relative| {
                let content = fs::read_to_string(join_relative(dir, &relative)).unwrap();
                (relative, content)
            })
            .collect()
    }
}
