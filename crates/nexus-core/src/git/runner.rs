//! Führt Git-Befehle mit einer Argumentliste aus (keine Shell, daher sicher bei Leerzeichen).

use std::path::Path;
use std::process::{Command, Output};

use crate::error::{HarnessError, Result};

/// Ergebnis eines Git-Aufrufs, der scheitern darf
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitAttempt {
    pub ok: bool,
    pub out: String,
    pub err: String,
}

/// Die drei Arten, Git aufzurufen. Als Trait, damit Tests einzelne Aufrufe scheitern
/// lassen können (wie `FakeGit` in den TypeScript-Tests).
pub trait GitCommands {
    /// Führt `git <args>` im Ordner `cwd` aus und liefert die getrimmte Ausgabe
    fn run(&self, cwd: &Path, args: &[&str]) -> Result<String> {
        Ok(self.raw(cwd, args)?.trim().to_string())
    }

    /// Wie `run`, aber ungetrimmt (für maschinenlesbare Ausgaben, deren erste Spalte
    /// ein Leerzeichen sein kann)
    fn raw(&self, cwd: &Path, args: &[&str]) -> Result<String>;

    /// Führt aus, ohne zu werfen; liefert Erfolg, Ausgabe und Fehlerausgabe (getrimmt)
    fn try_run(&self, cwd: &Path, args: &[&str]) -> GitAttempt;
}

/// Führt Git-Befehle aus.
#[derive(Debug, Clone, Copy, Default)]
pub struct GitRunner;

impl GitRunner {
    /// Neuer Aufrufer (hält keinen Zustand)
    pub fn new() -> Self {
        Self
    }

    /// Startet Git und wartet auf das Ergebnis; unter Windows ohne Konsolenfenster
    fn output(&self, cwd: &Path, args: &[&str]) -> Result<Output> {
        let mut command = Command::new("git");
        command.args(args).current_dir(cwd);
        #[cfg(windows)]
        {
            // CREATE_NO_WINDOW: sonst blitzt bei jedem Git-Aufruf ein Konsolenfenster auf
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x0800_0000);
        }
        command.output().map_err(|error| {
            HarnessError::new(
                "git-failed",
                format!(
                    "Git-Befehl fehlgeschlagen (git {}): {error}",
                    args.join(" ")
                ),
            )
            .into()
        })
    }
}

impl GitCommands for GitRunner {
    fn raw(&self, cwd: &Path, args: &[&str]) -> Result<String> {
        let output = self.output(cwd, args)?;
        if output.status.success() {
            return Ok(String::from_utf8_lossy(&output.stdout).into_owned());
        }
        Err(HarnessError::new("git-failed", failure_message(args, &output)).into())
    }

    fn try_run(&self, cwd: &Path, args: &[&str]) -> GitAttempt {
        match self.output(cwd, args) {
            Ok(output) if output.status.success() => GitAttempt {
                ok: true,
                out: String::from_utf8_lossy(&output.stdout).trim().to_string(),
                err: String::new(),
            },
            Ok(output) => GitAttempt {
                ok: false,
                out: String::from_utf8_lossy(&output.stdout).trim().to_string(),
                err: failure_detail(args, &output),
            },
            Err(error) => GitAttempt {
                ok: false,
                out: String::new(),
                err: error.to_string(),
            },
        }
    }
}

/// Meldung eines gescheiterten Git-Aufrufs, wortgleich zu TypeScript
fn failure_message(args: &[&str], output: &Output) -> String {
    format!(
        "Git-Befehl fehlgeschlagen (git {}): {}",
        args.join(" "),
        failure_detail(args, output)
    )
}

/// Grund des Scheiterns: die Fehlerausgabe von Git, sonst der Rückgabecode.
/// Git meldet manche Fälle (z. B. `merge-base --is-ancestor`) nur über den Code,
/// ohne etwas auf die Fehlerausgabe zu schreiben – leer bleiben darf der Grund nie.
fn failure_detail(args: &[&str], output: &Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if !stderr.is_empty() {
        return stderr;
    }
    match output.status.code() {
        Some(code) => format!("git {} endete mit Code {code}", args.join(" ")),
        None => format!("git {} wurde abgebrochen", args.join(" ")),
    }
}
