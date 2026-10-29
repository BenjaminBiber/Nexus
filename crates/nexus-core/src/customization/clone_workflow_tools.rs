//! Portiert `packages/core/src/customization/CloneWorkflowTools.ts`: prüft und lädt
//! `custom/workflow.ts` mit dem TypeScript und dem esbuild des Clones. Beides läuft als
//! Prozess, weil TypeScript 7 keine stabile JS-Schnittstelle hat und esbuild eine
//! Node-Bibliothek ist.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use regex::Regex;
use serde_json::Value;

use crate::error::{Error, Result};
use crate::process::hide_console;

use super::overview::Problem;
use super::tools::WorkflowTools;

/// Zeitgrenze der Typprüfung (zwei Minuten); nur die Tests setzen sie kürzer
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(120);

/// Das Skript, das `custom/workflow.ts` mit dem esbuild des Clones bündelt und lädt
const LOAD_SCRIPT: &str = include_str!("workflow_load.mjs");

/// Die Typen, die `custom/workflow.ts` aus `@harness/core` importiert. Sie stehen hier, weil
/// es das TypeScript-Paket nicht mehr gibt: `WORKFLOW_PHASES` kommt aus
/// [`crate::workflow::workflow_phases`], die Typen aus den Strukturen daneben.
const CORE_TYPES: &str = r#"export interface WorkflowOption {
  label: string;
  command: string;
  when: string;
}

export interface WorkflowStep {
  step: string;
  command?: string;
  place?: string;
  when: string;
  options?: WorkflowOption[];
}

export interface WorkflowPhase {
  id: string;
  title: string;
  hint: string;
  steps: WorkflowStep[];
}

export declare const WORKFLOW_PHASES: WorkflowPhase[];
"#;

/// Prüft und lädt `custom/workflow.ts` mit TypeScript und esbuild aus dem Clone.
#[derive(Debug, Clone)]
pub struct CloneWorkflowTools {
    clone_dir: PathBuf,
    timeout: Duration,
}

impl CloneWorkflowTools {
    /// Werkzeuge dieses Clones mit der üblichen Zeitgrenze
    pub fn new(clone_dir: impl Into<PathBuf>) -> Self {
        Self::with_timeout(clone_dir, DEFAULT_TIMEOUT)
    }

    /// Werkzeuge mit eigener Zeitgrenze (Tests)
    pub fn with_timeout(clone_dir: impl Into<PathBuf>, timeout: Duration) -> Self {
        Self {
            clone_dir: clone_dir.into(),
            timeout,
        }
    }

    /// Schreibt den Ersatz für `@harness/core`: die Typen als `.d.ts` und die Phasen als
    /// `.mjs`. Beides entsteht aus dem Fachkern, damit `custom/workflow.ts` weiterhin
    /// `import { WORKFLOW_PHASES, type WorkflowPhase } from '@harness/core'` schreiben kann.
    fn write_core_shim(&self, dir: &Path) -> Result<(PathBuf, PathBuf)> {
        let types = dir.join("harness-core.d.ts");
        std::fs::write(&types, CORE_TYPES).map_err(Error::unexpected)?;
        let phases = serde_json::to_string(&crate::workflow::workflow_phases())
            .map_err(Error::unexpected)?;
        let module = dir.join("harness-core.mjs");
        std::fs::write(
            &module,
            format!("export const WORKFLOW_PHASES = {phases};\n"),
        )
        .map_err(Error::unexpected)?;
        Ok((types, module))
    }

    /// Schreibt check.ts und tsconfig.json; @harness/core zeigt auf die Typen aus dem Fachkern
    fn write_check(&self, dir: &Path, file: &Path) -> Result<()> {
        // Absoluter Importpfad mit `/` (ein relativer scheitert unter Windows, wenn Temp und
        // Clone auf verschiedenen Laufwerken liegen)
        let target = absolute(file).to_string_lossy().replace('\\', "/");
        let import = serde_json::to_string(&target).map_err(Error::unexpected)?;
        let (types, _) = self.write_core_shim(dir)?;
        std::fs::write(
            dir.join("check.ts"),
            [
                "import type { WorkflowPhase } from '@harness/core';".to_string(),
                format!("import phases from {import};"),
                "export const checked: WorkflowPhase[] = phases;".to_string(),
                String::new(),
            ]
            .join("\n"),
        )
        .map_err(Error::unexpected)?;
        let config = serde_json::json!({
            "compilerOptions": {
                "strict": true,
                "noEmit": true,
                "target": "es2023",
                "module": "esnext",
                "moduleResolution": "bundler",
                "allowImportingTsExtensions": true,
                "skipLibCheck": true,
                "types": [],
                "paths": { "@harness/core": [types.to_string_lossy()] },
            },
            "files": ["check.ts"],
        });
        std::fs::write(
            dir.join("tsconfig.json"),
            serde_json::to_string_pretty(&config).map_err(Error::unexpected)?,
        )
        .map_err(Error::unexpected)
    }

    /// Zerlegt „datei(zeile,spalte): error TSnnnn: text“; Fehler in check.ts betreffen den
    /// Default-Export
    fn parse(&self, output: &str, file: &Path, status: Option<i32>) -> Vec<Problem> {
        let regex =
            Regex::new(r"(?m)^(.+?)\((\d+),\d+\): error (TS\d+): (.+)$").expect("festes Muster");
        let mut problems = Vec::new();
        for treffer in regex.captures_iter(output) {
            let (datei, zeile, code, text) = (
                treffer.get(1).map_or("", |m| m.as_str()),
                treffer.get(2).map_or("", |m| m.as_str()),
                treffer.get(3).map_or("", |m| m.as_str()),
                treffer.get(4).map_or("", |m| m.as_str()).trim_end(),
            );
            if Path::new(datei)
                .file_name()
                .is_some_and(|name| name == "check.ts")
            {
                problems.push(Problem::at(
                    "type-error",
                    format!("Der Default-Export passt nicht zu WorkflowPhase[]: {text}"),
                    file.to_string_lossy(),
                ));
                continue;
            }
            let mut problem = Problem::at(
                "type-error",
                format!("{code}: {text}"),
                self.clone_dir.join(datei).to_string_lossy(),
            );
            problem.line = zeile.parse().ok();
            problems.push(problem);
        }
        // Ohne erkennbare Fehlerzeile trotzdem melden, wenn tsc fehlschlug
        if problems.is_empty() && status != Some(0) {
            let erste = output.trim().lines().next().unwrap_or("").to_string();
            problems.push(Problem::at(
                "type-error",
                if erste.is_empty() {
                    "Typprüfung fehlgeschlagen.".to_string()
                } else {
                    erste
                },
                file.to_string_lossy(),
            ));
        }
        problems
    }
}

impl WorkflowTools for CloneWorkflowTools {
    /// Prüft eine erzeugte Datei, die den Default-Export als WorkflowPhase[] verwendet
    fn typecheck(&self, file: &Path) -> Result<Vec<Problem>> {
        let dir = tempfile::Builder::new()
            .prefix("harness-workflow-check-")
            .tempdir()
            .map_err(Error::unexpected)?;
        self.write_check(dir.path(), file)?;
        // tsc des Clones als Prozess starten, weil TypeScript 7 keine stabile
        // JS-Schnittstelle hat
        let tsc = self
            .clone_dir
            .join("node_modules")
            .join("typescript")
            .join("bin")
            .join("tsc");
        let mut command = Command::new("node");
        command
            .arg(&tsc)
            .arg("-p")
            .arg(dir.path().join("tsconfig.json"))
            .arg("--pretty")
            .arg("false")
            .current_dir(&self.clone_dir);
        Ok(match run(command, self.timeout) {
            Outcome::Timeout => vec![Problem::at(
                "type-error",
                "Die Typprüfung hat zu lange gedauert (über 2 Minuten).",
                file.to_string_lossy(),
            )],
            Outcome::NotStarted(message) => vec![Problem::at(
                "type-error",
                format!("TypeScript ließ sich nicht starten: {message}"),
                file.to_string_lossy(),
            )],
            Outcome::Done { output, status } => self.parse(&output, file, status),
        })
    }

    /// Bündelt die Datei mit esbuild, lädt sie und gibt den Default-Export als reine Daten
    /// (JSON) zurück
    fn load(&self, file: &Path) -> std::result::Result<Value, String> {
        let dir = tempfile::Builder::new()
            .prefix("harness-workflow-load-")
            .tempdir()
            .map_err(|error| error.to_string())?;
        let script = dir.path().join("load.mjs");
        std::fs::write(&script, LOAD_SCRIPT).map_err(|error| error.to_string())?;
        let (_, core) = self
            .write_core_shim(dir.path())
            .map_err(|error| error.to_string())?;
        let mut command = Command::new("node");
        command
            .arg(&script)
            .arg(&self.clone_dir)
            .arg(file)
            .arg(&core)
            .current_dir(&self.clone_dir);
        match run_with_stdout(command, self.timeout) {
            Outcome::Timeout => Err("Das Laden hat zu lange gedauert.".to_string()),
            Outcome::NotStarted(message) => Err(format!("Node ließ sich nicht starten: {message}")),
            Outcome::Done { output, status } => {
                if status != Some(0) {
                    let meldung = output.trim();
                    return Err(if meldung.is_empty() {
                        "Der Workflow ließ sich nicht bündeln.".to_string()
                    } else {
                        meldung.to_string()
                    });
                }
                serde_json::from_str(output.trim()).map_err(|error| error.to_string())
            }
        }
    }
}

/// Wie ein Aufruf ausgegangen ist
enum Outcome {
    /// Gestartet und beendet; `output` ist alles, was der Prozess geschrieben hat
    Done { output: String, status: Option<i32> },
    /// Zeitgrenze überschritten, der Prozess wurde beendet
    Timeout,
    /// Der Prozess ließ sich nicht starten (kein Node, keine Rechte …)
    NotStarted(String),
}

/// Startet den Befehl und liefert stdout und stderr zusammen (wie `${stdout}${stderr}` in
/// TypeScript); bricht nach `timeout` ab
fn run(command: Command, timeout: Duration) -> Outcome {
    run_streams(command, timeout, true)
}

/// Wie `run`, aber getrennt: bei Erfolg steht in `output` nur stdout, bei Fehlschlag stderr
fn run_with_stdout(command: Command, timeout: Duration) -> Outcome {
    run_streams(command, timeout, false)
}

/// Gemeinsamer Ablauf: Prozess starten, auf das Ende warten, Ausgaben einsammeln
fn run_streams(mut command: Command, timeout: Duration, zusammen: bool) -> Outcome {
    command.stdin(Stdio::null());
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());
    hide_console(&mut command);
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => return Outcome::NotStarted(error.to_string()),
    };
    // Die Ausgaben nebenläufig leeren, damit ein voller Puffer den Prozess nicht anhält
    let out = child.stdout.take().map(lesen);
    let err = child.stderr.take().map(lesen);
    let start = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Err(error) => return Outcome::NotStarted(error.to_string()),
            Ok(None) => {}
        }
        if start.elapsed() >= timeout {
            let _ = child.kill();
            let _ = child.wait();
            break None;
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let stdout = out.map(fertig).unwrap_or_default();
    let stderr = err.map(fertig).unwrap_or_default();
    let Some(status) = status else {
        return Outcome::Timeout;
    };
    let output = if zusammen {
        format!("{stdout}{stderr}")
    } else if status.success() {
        stdout
    } else {
        format!("{stderr}{stdout}")
    };
    Outcome::Done {
        output,
        status: status.code(),
    }
}

/// Liest einen Strom des Kindprozesses in einem eigenen Faden
fn lesen(mut stream: impl Read + Send + 'static) -> std::thread::JoinHandle<String> {
    std::thread::spawn(move || {
        let mut text = Vec::new();
        let _ = stream.read_to_end(&mut text);
        String::from_utf8_lossy(&text).into_owned()
    })
}

/// Wartet auf den Faden; ein abgestürzter Faden liefert keinen Text
fn fertig(handle: std::thread::JoinHandle<String>) -> String {
    handle.join().unwrap_or_default()
}

/// Absoluter Pfad ohne Zugriff auf das Dateisystem
fn absolute(path: &Path) -> PathBuf {
    if path.is_absolute() {
        return path.to_path_buf();
    }
    std::env::current_dir()
        .map(|dir| dir.join(path))
        .unwrap_or_else(|_| path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ohne_node_modules_meldet_einen_typfehler() {
        let dir = tempfile::tempdir().unwrap();
        let tools = CloneWorkflowTools::new(dir.path());
        let file = dir.path().join("workflow.ts");
        std::fs::write(&file, "export default [];").unwrap();
        let problems = tools.typecheck(&file).unwrap();
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert_eq!(problems[0].code, "type-error");
        assert!(!problems[0].message.is_empty());
    }

    #[test]
    fn ohne_esbuild_scheitert_das_laden_mit_meldung() {
        let dir = tempfile::tempdir().unwrap();
        let tools = CloneWorkflowTools::new(dir.path());
        let file = dir.path().join("workflow.ts");
        std::fs::write(&file, "export default [];").unwrap();
        let error = tools.load(&file).unwrap_err();
        assert!(!error.is_empty());
    }

    #[test]
    fn eine_fehlerzeile_wird_in_datei_zeile_und_text_zerlegt() {
        let tools = CloneWorkflowTools::new(Path::new("/clone"));
        let file = Path::new("/clone/custom/workflow.ts");
        let problems = tools.parse(
            "custom/workflow.ts(3,5): error TS2322: Type 'number' …\n",
            file,
            Some(2),
        );
        assert_eq!(problems.len(), 1);
        assert_eq!(problems[0].message, "TS2322: Type 'number' …");
        assert_eq!(problems[0].line, Some(3));
        assert_eq!(
            problems[0].file.as_deref(),
            Some(
                Path::new("/clone")
                    .join("custom/workflow.ts")
                    .to_string_lossy()
                    .as_ref()
            )
        );
    }

    #[test]
    fn ein_fehler_in_check_ts_betrifft_den_default_export() {
        let tools = CloneWorkflowTools::new(Path::new("/clone"));
        let file = Path::new("/clone/custom/workflow.ts");
        let problems = tools.parse(
            "/tmp/harness-workflow-check-1/check.ts(3,14): error TS2322: Type 'x' is not assignable\n",
            file,
            Some(2),
        );
        assert_eq!(problems.len(), 1);
        assert_eq!(
            problems[0].message,
            "Der Default-Export passt nicht zu WorkflowPhase[]: Type 'x' is not assignable"
        );
        assert_eq!(problems[0].line, None);
        assert_eq!(
            problems[0].file.as_deref(),
            Some(file.to_string_lossy().as_ref())
        );
    }

    #[test]
    fn ohne_erkennbare_zeile_meldet_ein_fehlschlag_die_erste_zeile() {
        let tools = CloneWorkflowTools::new(Path::new("/clone"));
        let file = Path::new("/clone/custom/workflow.ts");
        let problems = tools.parse("error TS5057: Datei fehlt\nzweite Zeile", file, Some(1));
        assert_eq!(problems.len(), 1);
        assert_eq!(problems[0].message, "error TS5057: Datei fehlt");
        // Ein erfolgreicher Lauf ohne Fehlerzeilen ergibt keine Probleme
        assert!(tools.parse("", file, Some(0)).is_empty());
    }
}
