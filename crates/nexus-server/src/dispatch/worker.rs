//! Der losgelöste Worker eines Laufs; portiert aus
//! `packages/server/src/dispatch/DispatchWorker.ts`.
//!
//! Er kennt keine Datenbank: er liest den Lauf aus `dispatch/runs`, richtet bei einer Umsetzung
//! den Worktree ein, startet Claude, schreibt das lesbare Lauf-Protokoll und am Ende immer eine
//! Ergebnisdatei. Ausgewertet wird im Server.

use std::collections::BTreeMap;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use base64::Engine as _;
use chrono::Local;
use nexus_core::clock::{iso, Clock};
use nexus_core::dispatch::output::{RunArtifact, RunArtifactScanner};
use nexus_core::dispatch::state::{RunFiles, RunSession};
use nexus_core::dispatch::{
    DispatchLog, RunEnd, RunKind, RunRecord, RunResult, RunStore, StreamEventFormatter,
    RUN_LOG_OUTPUT_MARKER, RUN_LOG_PROMPT_MARKER,
};
use nexus_core::process::{is_alive, orphan_sweeper::linked_worktree, OrphanSweeper};
use nexus_core::{HarnessError, HarnessHome, Result};

use super::claude_process::{ClaudeProcess, ClaudeRunInput, OutputSource};
use super::worktree_setup::{SetupInput, SetupOutcome, WorktreeSetupRunner};

/// So oft wird der Lauf gelesen, bevor er als nicht vorhanden gilt
const READ_ATTEMPTS: u32 = 5;
/// Pause zwischen zwei Leseversuchen
const READ_PAUSE: Duration = Duration::from_millis(200);
/// Zeitlimit der Einrichtung, wenn der Lauf keines nennt
const SETUP_TIMEOUT_MINUTES: i64 = 15;

/// Ist das eine gültige Lauf-ID (nichts, was einen Pfad verlassen könnte)?
fn gueltige_id(run_id: &str) -> bool {
    !run_id.is_empty()
        && run_id
            .chars()
            .all(|zeichen| zeichen.is_ascii_alphanumeric() || matches!(zeichen, '.' | '_' | '-'))
}

/// Das Lauf-Protokoll, geteilt mit den Rückrufen von Einrichtung und Claude.
/// Fehler beim Protokoll beenden den Lauf nicht: die Datei wird dann einfach vergessen.
#[derive(Debug, Clone, Default)]
struct RunLog(Arc<Mutex<Option<File>>>);

impl RunLog {
    /// Noch ohne Datei; bis zum Öffnen geht jeder Text ins Leere (wie `log === null` in TS)
    fn new() -> Self {
        Self::default()
    }

    /// Öffnet die Protokolldatei; scheitert das, bleibt das Protokoll leer
    fn open(&self, logs: &DispatchLog, file: &Path) {
        if let Ok(datei) = logs.open_run_log(file) {
            if let Ok(mut offen) = self.0.lock() {
                *offen = Some(datei);
            }
        }
    }

    /// Schreibt einen Text; ein Fehler schließt das Protokoll still
    fn write(&self, text: &str) {
        let Ok(mut offen) = self.0.lock() else { return };
        if let Some(datei) = offen.as_mut() {
            if datei.write_all(text.as_bytes()).is_err() {
                *offen = None;
            }
        }
    }

    /// Schließt das Protokoll und schreibt alles Gepufferte weg
    fn close(&self) {
        let Ok(mut offen) = self.0.lock() else { return };
        if let Some(datei) = offen.as_mut() {
            let _ = datei.flush();
        }
        *offen = None;
    }
}

/// Führt einen Lauf im eigenen Prozess aus: Protokoll, Einrichtung, Claude, Ergebnis.
#[derive(Debug, Clone)]
pub struct DispatchWorker {
    runs: RunStore,
    logs: DispatchLog,
    clock: Clock,
    /// Pause zwischen zwei Leseversuchen des Laufs (Tests kürzen sie)
    read_pause: Duration,
}

impl DispatchWorker {
    /// Worker im angegebenen Benutzerordner
    pub fn new(home: HarnessHome) -> Self {
        Self::with_clock(home, Clock::system())
    }

    /// Worker mit eigener Uhr (für Tests mit festen Zeitstempeln)
    pub fn with_clock(home: HarnessHome, clock: Clock) -> Self {
        Self {
            runs: RunStore::new(home.clone()),
            logs: DispatchLog::with_clock(home, clock.clone()),
            clock,
            read_pause: READ_PAUSE,
        }
    }

    /// Mit eigener Pause zwischen den Leseversuchen des Laufs (für Tests)
    pub fn with_read_pause(mut self, pause: Duration) -> Self {
        self.read_pause = pause;
        self
    }

    /// Führt den Lauf `run_id` aus. Bei gültiger Lauf-ID entsteht immer eine Ergebnisdatei;
    /// ein Fehler landet als Exit-Code 1 darin und wird danach weitergereicht.
    pub async fn run(&self, run_id: &str, env: BTreeMap<String, String>) -> Result<()> {
        if !gueltige_id(run_id) {
            return Err(HarnessError::new(
                "run-id-invalid",
                format!("„{run_id}“ ist keine gültige Lauf-ID."),
            )
            .into());
        }
        let log = RunLog::new();
        let (result, fehler) = match self.fuehre_aus(run_id, env, &log).await {
            Ok(result) => (result, None),
            Err(error) => {
                log.write(&format!(
                    "\n[dispatch] {} Fehler: {error}\n",
                    self.jetzt_iso()
                ));
                (
                    RunResult {
                        code: Some(1),
                        timed_out: false,
                        output: error.to_string(),
                        ..RunResult::default()
                    },
                    Some(error),
                )
            }
        };
        self.raeume_auf(run_id, &log).await;
        log.close();
        let geschrieben = self.runs.finish(
            run_id,
            &RunEnd {
                result,
                ended_at: self.jetzt_iso(),
            },
        );
        match fehler {
            Some(error) => Err(error),
            None => geschrieben,
        }
    }

    /// Beendet Prozesse, die der Lauf in seinem Worktree verwaist zurückgelassen hat (z. B. ein
    /// `nexus serve … &`). Nur in einem verknüpften Worktree, den kein anderer laufender Lauf
    /// nutzt; Fehler beenden den Lauf nicht
    async fn raeume_auf(&self, run_id: &str, log: &RunLog) {
        let Ok(Some(record)) = self.runs.read(run_id) else {
            return;
        };
        if !linked_worktree(Path::new(&record.cwd)) {
            return;
        }
        let Ok(laeufe) = self.runs.list() else {
            return;
        };
        for anderer in laeufe {
            if anderer.id == run_id
                || anderer.cwd != record.cwd
                || matches!(self.runs.result(&anderer.id), Ok(Some(_)))
            {
                continue;
            }
            let laeuft = anderer
                .pid
                .and_then(|pid| u32::try_from(pid).ok())
                .is_none_or(is_alive);
            if laeuft {
                return;
            }
        }
        let cwd = record.cwd.clone();
        let own = std::process::id();
        let Ok(pids) =
            tokio::task::spawn_blocking(move || OrphanSweeper::default().sweep(&cwd, own)).await
        else {
            return;
        };
        if !pids.is_empty() {
            let liste = pids
                .iter()
                .map(u32::to_string)
                .collect::<Vec<_>>()
                .join(", ");
            log.write(&format!(
                "[dispatch] {} Aufgeräumt: {} übrig gebliebene Prozesse beendet (PID {liste})\n",
                self.jetzt_iso(),
                pids.len()
            ));
        }
    }

    /// Der Lauf selbst: Kopf ins Protokoll, Einrichtung, Claude, Ende-Zeile
    async fn fuehre_aus(
        &self,
        run_id: &str,
        env: BTreeMap<String, String>,
        log: &RunLog,
    ) -> Result<RunResult> {
        let record = self.lies_lauf(run_id).await?.ok_or_else(|| {
            HarnessError::new("run-not-found", format!("Lauf {run_id} gibt es nicht."))
        })?;
        log.open(&self.logs, Path::new(&record.log_file));
        log.write(&self.kopf(&record));

        // Neu angelegter Worktree: erst die Einrichtung (z. B. npm ci); scheitert sie,
        // startet Claude nicht
        let mut setup_ok = None;
        if let Some(outcome) = self.richte_ein(&record, &env, log).await {
            match outcome {
                SetupOutcome::Ok => setup_ok = Some(true),
                SetupOutcome::Failed(gescheitert) => {
                    log.write(&format!(
                        "\n[dispatch] {} Einrichtung fehlgeschlagen – Claude wird nicht gestartet\n",
                        self.jetzt_iso()
                    ));
                    return Ok(RunResult {
                        code: gescheitert.code.map(i64::from),
                        timed_out: gescheitert.timed_out,
                        output: gescheitert.output.clone(),
                        setup_ok: None,
                        setup_failed: Some(nexus_core::dispatch::SetupFailure {
                            cwd: gescheitert.cwd,
                            run: gescheitert.run,
                            code: gescheitert.code.map(i64::from),
                            timed_out: gescheitert.timed_out,
                            output: gescheitert.output,
                        }),
                    });
                }
            }
        }

        // stream-json lesbar und mit Uhrzeit ins Protokoll; je Strom ein eigener Formatierer
        // (Teilzeilen bleiben getrennt). Ausgewertet wird die rohe Ausgabe im Ergebnis
        let stdout = Formatierer::new(&record.cwd, log, &self.clock);
        let stderr = Formatierer::new(&record.cwd, log, &self.clock);
        // Neben dem Protokoll bleiben Session und Screenshots des Laufs (Ordner `<Lauf-ID>.files`)
        let artefakte = Artefakte::new(Path::new(&record.log_file), &record.cwd, &self.clock);
        let input = ClaudeRunInput {
            command: record.command.clone(),
            args: record.args.clone(),
            prompt: record.prompt.clone(),
            cwd: PathBuf::from(&record.cwd),
            env,
            timeout: Duration::from_secs(minuten_in_sekunden(record.run.timeout_minutes)),
            cancel: None,
        };
        let result = ClaudeProcess::new()
            .run(input, {
                let log = log.clone();
                let stdout = stdout.clone();
                let stderr = stderr.clone();
                let artefakte = artefakte.clone();
                move |chunk, source| match source {
                    // Meldungen des Dispatchers stehen unverändert im Protokoll
                    OutputSource::Dispatch => log.write(chunk),
                    OutputSource::Stdout => {
                        stdout.push(chunk);
                        artefakte.push(chunk);
                    }
                    OutputSource::Stderr => stderr.push(chunk),
                }
            })
            .await;
        stdout.flush();
        stderr.flush();
        artefakte.flush();
        log.write(&format!(
            "\n[dispatch] {} Ende: Exit-Code {}{}\n",
            self.jetzt_iso(),
            code_text(result.code),
            if result.timed_out { ", Zeitlimit" } else { "" }
        ));
        Ok(RunResult {
            code: result.code.map(i64::from),
            timed_out: result.timed_out,
            output: result.output,
            setup_ok,
            setup_failed: None,
        })
    }

    /// Der Kopf des Protokolls; der Wortlaut ist Vertrag (`RunLogSplitter` liest ihn)
    fn kopf(&self, record: &RunRecord) -> String {
        let r = &record.run;
        let standard =
            |wert: &Option<String>| wert.clone().unwrap_or_else(|| "Standard".to_string());
        format!(
            "[dispatch] {} {} startet in {} ({})\n\
             [dispatch] Profil {}: Modell {}, Effort {}, {} Schritte, {} min ({})\n\
             [dispatch] {} {}\n\
             {RUN_LOG_PROMPT_MARKER}\n{}\n\n{RUN_LOG_OUTPUT_MARKER}\n",
            self.jetzt_iso(),
            record.agent,
            record.cwd,
            art(record.kind),
            r.profile,
            standard(&r.model),
            standard(&r.effort),
            r.max_turns,
            r.timeout_minutes,
            r.why,
            record.command,
            record.args.join(" "),
            record.prompt,
        )
    }

    /// Richtet den Worktree ein, wenn es eine Umsetzung mit Einrichtungsbefehlen ist;
    /// sonst `None` (dann gibt es im Ergebnis weder `setupOk` noch `setupFailed`)
    async fn richte_ein(
        &self,
        record: &RunRecord,
        env: &BTreeMap<String, String>,
        log: &RunLog,
    ) -> Option<SetupOutcome> {
        if record.kind != RunKind::Work {
            return None;
        }
        let steps = record.setup.as_ref().filter(|steps| !steps.is_empty())?;
        let input = SetupInput {
            steps: steps
                .iter()
                .map(|step| super::worktree_setup::WorktreeSetupStep {
                    cwd: step.cwd.clone(),
                    run: step.run.clone(),
                })
                .collect(),
            root: PathBuf::from(&record.cwd),
            env: env.clone(),
            timeout: Duration::from_secs(minuten_in_sekunden(
                record
                    .setup_timeout_minutes
                    .unwrap_or(SETUP_TIMEOUT_MINUTES),
            )),
        };
        let log = log.clone();
        Some(
            WorktreeSetupRunner::new()
                .run(input, move |text| log.write(text))
                .await,
        )
    }

    /// Liest den Lauf; der Server schreibt ihn kurz vor dem Start, daher mehrere Versuche
    /// mit Pause
    async fn lies_lauf(&self, run_id: &str) -> Result<Option<RunRecord>> {
        for versuch in 1..=READ_ATTEMPTS {
            if let Some(record) = self.runs.read(run_id)? {
                return Ok(Some(record));
            }
            if versuch < READ_ATTEMPTS {
                tokio::time::sleep(self.read_pause).await;
            }
        }
        Ok(None)
    }

    /// Jetzt als ISO-Zeit, wortgleich mit `new Date().toISOString()`
    fn jetzt_iso(&self) -> String {
        iso(self.clock.now())
    }
}

/// Ein Strom von Claude als lesbare Protokollzeilen mit Uhrzeit
#[derive(Clone)]
struct Formatierer {
    formatter: Arc<Mutex<StreamEventFormatter>>,
    log: RunLog,
    clock: Clock,
}

impl Formatierer {
    /// Formatierer für einen Strom; Pfade im Arbeitsordner erscheinen relativ
    fn new(root: &str, log: &RunLog, clock: &Clock) -> Self {
        Self {
            formatter: Arc::new(Mutex::new(StreamEventFormatter::with_root(Some(root)))),
            log: log.clone(),
            clock: clock.clone(),
        }
    }

    /// Nimmt ein Stück Ausgabe und schreibt die darin vollständigen Zeilen ins Protokoll
    fn push(&self, chunk: &str) {
        let Ok(mut formatter) = self.formatter.lock() else {
            return;
        };
        let zeilen = formatter.push(chunk.as_bytes());
        drop(formatter);
        self.schreibe(&zeilen);
    }

    /// Schreibt den Rest ohne Zeilenende (am Ende des Laufs)
    fn flush(&self) {
        let Ok(mut formatter) = self.formatter.lock() else {
            return;
        };
        let zeilen = formatter.flush();
        drop(formatter);
        self.schreibe(&zeilen);
    }

    fn schreibe(&self, zeilen: &[nexus_core::dispatch::RunLogLine]) {
        let jetzt = self.clock.now().with_timezone(&Local).naive_local();
        for zeile in zeilen {
            self.log.write(&StreamEventFormatter::stamped(zeile, jetzt));
        }
    }
}

/// Hält fest, was über das Protokoll hinaus vom Lauf bleibt: die Session von Claude Code und
/// die Bilder aus Tool-Ergebnissen. Fehler beim Ablegen beenden den Lauf nie.
#[derive(Clone)]
struct Artefakte {
    scanner: Arc<Mutex<RunArtifactScanner>>,
    files: RunFiles,
    cwd: String,
    clock: Clock,
}

impl Artefakte {
    /// Ablage neben dem Protokoll `log_file`; `cwd` ist der Arbeitsordner des Laufs
    fn new(log_file: &Path, cwd: &str, clock: &Clock) -> Self {
        Self {
            scanner: Arc::new(Mutex::new(RunArtifactScanner::new())),
            files: RunFiles::beside(log_file),
            cwd: cwd.to_string(),
            clock: clock.clone(),
        }
    }

    /// Nimmt ein Stück Ausgabe und legt ab, was in den vollständigen Zeilen steckt
    fn push(&self, chunk: &str) {
        let Ok(mut scanner) = self.scanner.lock() else {
            return;
        };
        let funde = scanner.push(chunk.as_bytes());
        drop(scanner);
        self.lege_ab(funde);
    }

    /// Wertet den Rest ohne Zeilenende aus (am Ende des Laufs)
    fn flush(&self) {
        let Ok(mut scanner) = self.scanner.lock() else {
            return;
        };
        let funde = scanner.flush();
        drop(scanner);
        self.lege_ab(funde);
    }

    /// Schreibt die Funde in die Ablage; was sich nicht schreiben oder dekodieren lässt, entfällt
    fn lege_ab(&self, funde: Vec<RunArtifact>) {
        for fund in funde {
            match fund {
                RunArtifact::Session { session_id } => {
                    let _ = self.files.write_session(&RunSession {
                        session_id,
                        cwd: self.cwd.clone(),
                    });
                }
                RunArtifact::Image {
                    tool_use_id,
                    media_type,
                    data,
                } => {
                    let Ok(bytes) =
                        base64::engine::general_purpose::STANDARD.decode(data.as_bytes())
                    else {
                        continue;
                    };
                    let _ =
                        self.files
                            .add_shot(&tool_use_id, &media_type, &bytes, self.clock.now());
                }
            }
        }
    }
}

/// Minuten als Sekunden; negative Werte zählen wie null
fn minuten_in_sekunden(minuten: i64) -> u64 {
    u64::try_from(minuten.max(0)).unwrap_or(0) * 60
}

/// Exit-Code im Protokoll; ein beendeter Prozess hat keinen (in TypeScript `null`)
fn code_text(code: Option<i32>) -> String {
    code.map(|code| code.to_string())
        .unwrap_or_else(|| "null".to_string())
}

/// Wie der Lauf im Protokollkopf heißt
fn art(kind: RunKind) -> &'static str {
    match kind {
        RunKind::Work => "Umsetzung",
        RunKind::Merge => "Merge-Session",
        RunKind::Research => "Recherche",
        RunKind::Plan => "Planung",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lauf_ids_mit_pfadanteilen_sind_ungueltig() {
        assert!(gueltige_id("ARC-0001-20261029-091500"));
        assert!(gueltige_id("a.b_c-1"));
        assert!(!gueltige_id("../böse"));
        assert!(!gueltige_id("a/b"));
        assert!(!gueltige_id(""));
    }

    #[test]
    fn exit_code_null_steht_als_null_im_protokoll() {
        assert_eq!(code_text(Some(3)), "3");
        assert_eq!(code_text(None), "null");
    }

    #[test]
    fn jede_lauf_art_hat_ihren_namen() {
        assert_eq!(art(RunKind::Work), "Umsetzung");
        assert_eq!(art(RunKind::Merge), "Merge-Session");
        assert_eq!(art(RunKind::Research), "Recherche");
        assert_eq!(art(RunKind::Plan), "Planung");
    }
}
