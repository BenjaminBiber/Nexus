//! Temporäres Git-Projekt mit eigenem `HARNESS_HOME` und einer SQLite-Board-Datenbank; ruft das
//! gebaute Binary `nexus` darin auf.
//! Portiert aus `packages/cli/tests/integration/helpers/TempProject.ts`.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use assert_cmd::cargo::CommandCargoExt;
use nexus_core::HarnessHome;
use nexus_db::{BoardRuntime, ConnectOptions};
use tempfile::TempDir;

/// GUID dieses Rechners in der Test-Konfig
pub const USER_ID: &str = "11111111-1111-1111-1111-111111111111";

/// Anzeigename dieses Rechners in der Test-Konfig
pub const USER_NAME: &str = "Test";

/// Ergebnis eines Aufrufs
#[derive(Debug)]
pub struct Lauf {
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
}

impl Lauf {
    /// Die Ausgabe von `--json` als Wert
    pub fn json(&self) -> serde_json::Value {
        serde_json::from_str(&self.stdout)
            .unwrap_or_else(|error| panic!("{error}: {}", self.stdout))
    }
}

/// Temporäres Git-Projekt samt eigenem Home und eigener Board-Datenbank.
pub struct TempProject {
    base: TempDir,
    /// Ordner des Projekts (mit Leerzeichen und Umlaut, wie in TypeScript)
    pub path: PathBuf,
}

impl TempProject {
    /// Legt Projekt, ersten Commit und ein Home mit SQLite-`board.yaml` an
    pub fn new() -> Self {
        let base = tempfile::Builder::new()
            .prefix("harness-board-")
            .tempdir()
            .expect("Temp-Ordner");
        let root = fs::canonicalize(base.path()).expect("echter Pfad");
        let path = root.join("Demo Projekt ä");
        fs::create_dir(&path).expect("Projektordner");
        fs::create_dir(root.join("home")).expect("Home-Ordner");
        let project = Self { base, path };
        project.git(&["init", "-q", "-b", "main"]);
        project.git(&["config", "user.email", "test@example.invalid"]);
        project.git(&["config", "user.name", "Test"]);
        project.git(&["commit", "--allow-empty", "-qm", "erster"]);
        project.write_config(None);
        project
    }

    /// Ordner, in dem Projekt und Home liegen
    pub fn base(&self) -> PathBuf {
        fs::canonicalize(self.base.path()).expect("echter Pfad")
    }

    /// Eigenes `HARNESS_HOME` dieses Testprojekts
    pub fn home(&self) -> PathBuf {
        self.base().join("home")
    }

    /// Schreibt `board.yaml` auf die SQLite-Datei des Homes, optional mit aktivem Board
    fn write_config(&self, active_board: Option<i32>) {
        let home = HarnessHome::at(self.home());
        let active = match active_board {
            Some(id) => format!("activeBoard: {id}\n"),
            None => String::new(),
        };
        fs::write(
            home.board_config_file(),
            format!(
                "provider: sqlite\nfile: {}\nuserId: {USER_ID}\nuserName: {USER_NAME}\n{active}",
                home.board_db_file().display()
            ),
        )
        .expect("board.yaml");
    }

    /// Legt Projekt `ARC` und Board `Sprint` an und macht es zum aktiven Board; liefert die
    /// Board-Nummer. Projekt- und Board-Befehle der CLI kommen erst in späteren Tickets,
    /// deshalb über die Dienste.
    pub fn seed(&self) -> i32 {
        let id = self.am_board(async |rt| {
            let actor = rt.actor(None);
            rt.services
                .projects
                .create("ARC", "Arches", &actor)
                .await
                .expect("Projekt");
            rt.services
                .boards
                .create("ARC", "Sprint", &actor)
                .await
                .expect("Board")
                .board
                .id
        });
        self.write_config(Some(id));
        id
    }

    /// Ein weiteres Board des Projekts `ARC`; das aktive Board bleibt, wie es ist
    pub fn create_board(&self, name: &str) -> i32 {
        self.am_board(async |rt| {
            rt.services
                .boards
                .create("ARC", name, &rt.actor(None))
                .await
                .expect("Board")
                .board
                .id
        })
    }

    /// Ordnet einem Board den Hauptordner eines Repos zu (`board boards repo` kommt später)
    pub fn assign_repo(&self, board_id: i32, repo: &Path) {
        self.am_board(async |rt| {
            rt.services
                .boards
                .set_repo_path(board_id, Some(&repo.display().to_string()))
                .await
                .expect("Repo-Pfad");
        });
    }

    /// Legt einen weiteren Worktree neben dem Projekt an und liefert seinen Pfad
    pub fn add_worktree(&self, name: &str) -> PathBuf {
        let path = self.base().join(name);
        self.git(&[
            "worktree",
            "add",
            "-q",
            "-b",
            &format!("branch-{name}"),
            &path.to_string_lossy(),
        ]);
        path
    }

    /// Öffnet das Board dieses Homes, führt die Arbeit aus und schließt es wieder
    pub fn am_board<T, W>(&self, work: W) -> T
    where
        W: AsyncFnOnce(&BoardRuntime) -> T,
    {
        let home = HarnessHome::at(self.home());
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("Laufzeit");
        runtime.block_on(async {
            let rt = BoardRuntime::open(&home, ConnectOptions::default())
                .await
                .expect("Board öffnet");
            let value = work(&rt).await;
            rt.close().await.expect("Board schließt");
            value
        })
    }

    /// Führt Git im Projekt aus und liefert die getrimmte Ausgabe
    pub fn git(&self, args: &[&str]) -> String {
        let output = Command::new("git")
            .args(args)
            .current_dir(&self.path)
            .output()
            .expect("git startet");
        assert!(
            output.status.success(),
            "git {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    }

    /// Ruft `nexus <args>` im Projekt auf
    pub fn run(&self, args: &[&str]) -> Lauf {
        self.run_in(&self.path, args)
    }

    /// Wie [`TempProject::run`], dazu `--json` vorn; der Aufruf muss klappen
    pub fn json(&self, args: &[&str]) -> serde_json::Value {
        let mit_json: Vec<&str> = std::iter::once("--json")
            .chain(args.iter().copied())
            .collect();
        let lauf = self.run(&mit_json);
        assert_eq!(lauf.code, 0, "{}", lauf.stderr);
        lauf.json()
    }

    /// Wie [`TempProject::run`], aber in einem anderen Arbeitsordner
    pub fn run_in(&self, cwd: &Path, args: &[&str]) -> Lauf {
        self.spawn(cwd, &self.home(), args, None)
    }

    /// Wie [`TempProject::run`], aber mit einem anderen `HARNESS_HOME`
    pub fn run_in_home(&self, home: &Path, args: &[&str]) -> Lauf {
        self.spawn(&self.path.clone(), home, args, None)
    }

    /// Wie [`TempProject::run`], dazu ein Text auf der Standardeingabe
    pub fn run_with_input(&self, args: &[&str], input: &str) -> Lauf {
        self.spawn(&self.path.clone(), &self.home(), args, Some(input))
    }

    /// Ruft `nexus <args>` mit eigenem Arbeitsordner, eigener Eingabe und zusätzlichen
    /// Umgebungsvariablen auf (die Hooks lesen Frist, Port und Env-Datei aus der Umgebung)
    pub fn run_with_env(
        &self,
        cwd: &Path,
        args: &[&str],
        env: &[(&str, &str)],
        input: Option<&str>,
    ) -> Lauf {
        self.spawn_with(cwd, &self.home(), args, input, env)
    }

    /// Wie [`TempProject::run`], aber innerhalb einer Session (`HARNESS_SESSION`)
    pub fn run_in_session(&self, session: &str, args: &[&str]) -> Lauf {
        self.spawn_with(
            &self.path.clone(),
            &self.home(),
            args,
            None,
            &[("HARNESS_SESSION", session)],
        )
    }

    /// Wie [`TempProject::run_in_session`], aber mit einem anderen `HARNESS_HOME`
    pub fn run_in_session_in_home(&self, home: &Path, session: &str, args: &[&str]) -> Lauf {
        self.spawn_with(
            &self.path.clone(),
            home,
            args,
            None,
            &[("HARNESS_SESSION", session)],
        )
    }

    /// Startet das Binary; `HARNESS_SESSION` und `HARNESS_TICKET` werden nicht geerbt, sonst
    /// gilt der Testlauf als innerhalb einer Session bzw. einem Ticket zugeordnet
    fn spawn(&self, cwd: &Path, home: &Path, args: &[&str], input: Option<&str>) -> Lauf {
        self.spawn_with(cwd, home, args, input, &[])
    }

    /// Wie [`TempProject::spawn`], dazu zusätzliche Umgebungsvariablen (auch `HARNESS_SESSION`)
    fn spawn_with(
        &self,
        cwd: &Path,
        home: &Path,
        args: &[&str],
        input: Option<&str>,
        extra: &[(&str, &str)],
    ) -> Lauf {
        let mut command = Command::cargo_bin("nexus").expect("Binary nexus");
        command
            .args(args)
            .current_dir(cwd)
            .env("HARNESS_HOME", home)
            .env_remove("HARNESS_SESSION")
            .env_remove("HARNESS_TICKET");
        for (name, value) in extra {
            command.env(name, value);
        }
        let output = match input {
            None => command.output().expect("nexus startet"),
            Some(text) => {
                use std::io::Write;
                let mut child = command
                    .stdin(std::process::Stdio::piped())
                    .stdout(std::process::Stdio::piped())
                    .stderr(std::process::Stdio::piped())
                    .spawn()
                    .expect("nexus startet");
                child
                    .stdin
                    .as_mut()
                    .expect("Standardeingabe")
                    .write_all(text.as_bytes())
                    .expect("Standardeingabe schreibt");
                child.wait_with_output().expect("nexus endet")
            }
        };
        Lauf {
            code: output.status.code().expect("Rückgabecode"),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        }
    }
}
