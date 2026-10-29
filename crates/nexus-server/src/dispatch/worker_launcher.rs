//! Startet den Worker eines Laufs als eigenen, losgelösten Prozess; portiert aus
//! `packages/server/src/dispatch/WorkerLauncher.ts`.
//!
//! Der Worker soll einen Neustart des Servers und das Schließen des Terminals überleben,
//! deshalb bekommt er keine Ein- und Ausgabe und eine eigene Prozessgruppe:
//!
//! - **Unix:** `process_group(0)`. Der Worker steht damit nicht mehr in der Vordergrundgruppe
//!   des Terminals und bekommt beim Schließen kein `SIGHUP`. (Der Plan nennt `setsid` per
//!   `pre_exec`; das wäre `unsafe`, was der Workspace verbietet.)
//! - **Windows:** `DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW`, dazu
//!   `CREATE_BREAKAWAY_FROM_JOB`, damit der Worker nicht mit dem Job-Objekt des Servers endet.
//!   Lehnt das System den Ausbruch ab, folgt ein zweiter Versuch ohne Breakaway.
//!
//! In Phase 5 ruft die App ihre eigene Programmdatei auf; der Pfad ist deshalb übergebbar.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use nexus_core::{Error, Result};

/// Unterbefehl, mit dem die Programmdatei zum Worker wird
const SUBCOMMAND: &str = "dispatch-worker";

/// Startet den Worker eines Laufs; Tests ersetzen das durch eine Attrappe.
pub trait WorkerLauncher: std::fmt::Debug + Send + Sync {
    /// Startet losgelöst und liefert die Prozessnummer (`None`, wenn der Start scheiterte)
    fn launch(&self, run_id: &str, env: &BTreeMap<String, String>) -> Option<u32>;
}

/// Startet dieselbe Programmdatei wie der Server mit `dispatch-worker <lauf>`.
#[derive(Debug, Clone)]
pub struct ProcessWorkerLauncher {
    /// Programmdatei, die als Worker startet (Standard: die eigene)
    exe: PathBuf,
}

impl ProcessWorkerLauncher {
    /// Starter für die eigene Programmdatei
    pub fn new() -> Result<Self> {
        let exe = std::env::current_exe().map_err(Error::unexpected)?;
        Ok(Self::with_exe(exe))
    }

    /// Starter für eine andere Programmdatei (Desktop-App, Tests)
    pub fn with_exe(exe: impl Into<PathBuf>) -> Self {
        Self { exe: exe.into() }
    }

    /// Baut den Befehl: ohne Ein- und Ausgabe, mit vollständig vorgegebener Umgebung
    fn befehl(&self, run_id: &str, env: &BTreeMap<String, String>) -> Command {
        let mut command = Command::new(&self.exe);
        command.arg(SUBCOMMAND).arg(run_id);
        command.env_clear();
        command.envs(env);
        command.stdin(Stdio::null());
        command.stdout(Stdio::null());
        command.stderr(Stdio::null());
        command
    }

    /// Startet den Worker; `breakaway` gilt nur unter Windows
    fn starte(
        &self,
        run_id: &str,
        env: &BTreeMap<String, String>,
        breakaway: bool,
    ) -> std::io::Result<std::process::Child> {
        let mut command = self.befehl(run_id, env);
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;

            let _ = breakaway;
            command.process_group(0);
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;

            /// Ohne eigene Konsole starten
            const DETACHED_PROCESS: u32 = 0x0000_0008;
            /// Eigene Prozessgruppe: Strg+C des Terminals erreicht den Worker nicht
            const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
            /// Aus dem Job-Objekt des Servers ausbrechen
            const CREATE_BREAKAWAY_FROM_JOB: u32 = 0x0100_0000;

            let mut flags =
                DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP | nexus_core::process::CREATE_NO_WINDOW;
            if breakaway {
                flags |= CREATE_BREAKAWAY_FROM_JOB;
            }
            command.creation_flags(flags);
        }
        #[cfg(not(any(unix, windows)))]
        {
            let _ = breakaway;
        }
        command.spawn()
    }
}

impl WorkerLauncher for ProcessWorkerLauncher {
    fn launch(&self, run_id: &str, env: &BTreeMap<String, String>) -> Option<u32> {
        let gestartet = match self.starte(run_id, env, true) {
            Ok(kind) => Ok(kind),
            // „Zugriff verweigert“: das Job-Objekt lässt keinen Ausbruch zu – ohne erneut
            // versuchen (unter Unix ist der erste Versuch schon der ohne Breakaway)
            Err(_) if cfg!(windows) => self.starte(run_id, env, false),
            Err(error) => Err(error),
        };
        let kind = gestartet.ok()?;
        let pid = kind.id();
        // Der Worker wird nicht abgewartet; ein Hintergrund-Thread holt nur sein Ende ab,
        // damit unter Unix kein Zombie bleibt, solange der Server läuft
        std::thread::spawn(move || {
            let mut kind = kind;
            let _ = kind.wait();
        });
        Some(pid)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn der_befehl_nennt_den_versteckten_unterbefehl_und_den_lauf() {
        let launcher = ProcessWorkerLauncher::with_exe("/pfad/nexus");
        let command = launcher.befehl("ARC-0001-x", &BTreeMap::new());
        let args: Vec<_> = command
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();
        assert_eq!(command.get_program().to_string_lossy(), "/pfad/nexus");
        assert_eq!(args, vec!["dispatch-worker", "ARC-0001-x"]);
    }

    #[test]
    fn die_umgebung_wird_vollstaendig_vorgegeben() {
        let launcher = ProcessWorkerLauncher::with_exe("/pfad/nexus");
        let env = BTreeMap::from([("HARNESS_TICKET".to_string(), "ARC-0001".to_string())]);
        let command = launcher.befehl("ARC-0001-x", &env);
        let gesetzt: Vec<_> = command
            .get_envs()
            .map(|(name, wert)| {
                (
                    name.to_string_lossy().into_owned(),
                    wert.map(|wert| wert.to_string_lossy().into_owned()),
                )
            })
            .collect();
        assert_eq!(
            gesetzt,
            vec![("HARNESS_TICKET".to_string(), Some("ARC-0001".to_string()))]
        );
    }

    #[test]
    fn ein_fehlendes_programm_zeigt_sich_als_fehlende_prozessnummer() {
        let launcher = ProcessWorkerLauncher::with_exe("/gibt/es/nicht/nexus");
        assert_eq!(launcher.launch("ARC-0001-x", &BTreeMap::new()), None);
    }
}
