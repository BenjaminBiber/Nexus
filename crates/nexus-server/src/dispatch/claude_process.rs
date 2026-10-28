//! Startet Claude Code im Headless-Modus, beendet es nach dem Zeitlimit samt Kindprozessen
//! und sammelt die Ausgabe; portiert aus `packages/server/src/dispatch/ClaudeProcess.ts`.
//!
//! Der Baustein hängt nicht vom Dispatch-Zustand ab: auch die Docs-Dienste (Überarbeiten,
//! Ticket-Entwürfe, Doc-Fragen) starten Claude darüber.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use nexus_core::process::ProcessGroup;
use nexus_core::HarnessError;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};
use tokio_util::sync::CancellationToken;

/// So viele Zeichen der Ausgabe werden behalten (nur das Ende zählt)
const MAX_OUTPUT_CHARS: usize = 200_000;

/// Wie lange nach dem Prozessende höchstens auf das Schließen der Ausgaben gewartet wird
const OUTPUT_DRAIN: Duration = Duration::from_secs(10);

/// Herkunft einer Ausgabe: Claude (stdout, stderr) oder eine Meldung des Dispatchers selbst
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputSource {
    Stdout,
    Stderr,
    Dispatch,
}

/// Ergebnis eines Laufs; entspricht `RunResult` aus TypeScript (ohne die Felder der Einrichtung)
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RunResult {
    /// Exit-Code von Claude; `None`, wenn der Prozess beendet wurde
    pub code: Option<i32>,
    pub timed_out: bool,
    /// stdout (Ereignisse von `--output-format stream-json`), notfalls samt stderr
    pub output: String,
}

/// Alles, was ein Lauf braucht
#[derive(Debug, Clone)]
pub struct ClaudeRunInput {
    pub command: String,
    pub args: Vec<String>,
    /// Arbeitsanweisung; geht über stdin an Claude
    pub prompt: String,
    pub cwd: PathBuf,
    /// Umgebung des Prozesses; sie wird vollständig vorgegeben (nichts wird geerbt)
    pub env: BTreeMap<String, String>,
    pub timeout: Duration,
    /// Bricht den Lauf ab: Claude wird samt Kindprozessen beendet, das Ergebnis kommt wie sonst
    pub cancel: Option<CancellationToken>,
}

/// Wie ein Lauf endet: von selbst, im Zeitlimit oder durch Abbruch von außen
enum Ausgang {
    Fertig(Option<i32>),
    Zeitlimit,
    Abbruch,
}

/// Gesammelte Ausgabe eines Laufs (nur das Ende, siehe `keep`)
#[derive(Debug, Default)]
struct Collected {
    stdout: String,
    stderr: String,
}

/// Startet Claude, liest die Ausgabe zeilenweise mit und beendet bei Zeitlimit oder Abbruch
/// den ganzen Prozessbaum.
#[derive(Debug, Clone, Copy)]
pub struct ClaudeProcess {
    /// Wartezeit nach dem Beenden im Zeitlimit, bis hart beendet und das Ergebnis geliefert wird
    hard_kill: Duration,
}

impl Default for ClaudeProcess {
    fn default() -> Self {
        Self::new()
    }
}

impl ClaudeProcess {
    /// Mit der üblichen Wartezeit von 15 s bis zum harten Beenden
    pub fn new() -> Self {
        Self {
            hard_kill: Duration::from_secs(15),
        }
    }

    /// Mit eigener Wartezeit bis zum harten Beenden (für Tests)
    pub fn with_hard_kill(hard_kill: Duration) -> Self {
        Self { hard_kill }
    }

    /// Führt den Lauf aus; endet nie mit einem Fehler, alles landet im Ergebnis
    /// (Befehl nicht gefunden: Exit-Code 127). `on_output` bekommt jedes Stück Ausgabe
    /// mit seiner Quelle, auch die Meldungen des Dispatchers.
    pub async fn run(
        &self,
        input: ClaudeRunInput,
        on_output: impl FnMut(&str, OutputSource) + Send + 'static,
    ) -> RunResult {
        let mut command = match self.build(&input) {
            Ok(command) => command,
            Err(error) => {
                return RunResult {
                    code: Some(127),
                    timed_out: false,
                    output: error.message,
                }
            }
        };
        let mut group = match ProcessGroup::spawn(&mut command) {
            Ok(group) => group,
            Err(error) => {
                return RunResult {
                    code: Some(127),
                    timed_out: false,
                    output: error.to_string(),
                }
            }
        };

        // Ein Sammler nimmt alle Stücke entgegen; so ruft nur eine Stelle `on_output` auf
        let (sender, mut receiver) = unbounded_channel::<(String, OutputSource)>();
        let collected = Arc::new(Mutex::new(Collected::default()));
        let sammler = tokio::spawn({
            let collected = Arc::clone(&collected);
            let mut on_output = on_output;
            async move {
                while let Some((chunk, source)) = receiver.recv().await {
                    on_output(&chunk, source);
                    let mut collected = collected.lock().expect("Ausgabe-Sammler");
                    match source {
                        OutputSource::Stdout => {
                            let text = std::mem::take(&mut collected.stdout) + &chunk;
                            collected.stdout = keep(text);
                        }
                        OutputSource::Stderr => {
                            let text = std::mem::take(&mut collected.stderr) + &chunk;
                            collected.stderr = keep(text);
                        }
                        // Meldungen des Dispatchers stehen nur im Protokoll, nicht im Ergebnis
                        OutputSource::Dispatch => {}
                    }
                }
            }
        });

        // Anweisung schreiben und stdin schließen; Fehler dabei sind egal (Claude kann schon weg sein)
        if let Some(mut stdin) = group.take_stdin() {
            let prompt = input.prompt.clone();
            tokio::spawn(async move {
                let _ = stdin.write_all(prompt.as_bytes()).await;
                let _ = stdin.shutdown().await;
            });
        }
        if let Some(stdout) = group.take_stdout() {
            tokio::spawn(lies_zeilenweise(
                stdout,
                OutputSource::Stdout,
                sender.clone(),
            ));
        }
        if let Some(stderr) = group.take_stderr() {
            tokio::spawn(lies_zeilenweise(
                stderr,
                OutputSource::Stderr,
                sender.clone(),
            ));
        }

        let cancel = input.cancel.clone().unwrap_or_default();
        // Erst entscheiden, wie der Lauf endet; der Prozess wird danach angefasst, damit
        // das Warten nicht mehr geliehen ist.
        let ausgang = tokio::select! {
            ende = group.wait() => Ausgang::Fertig(ende.ok().and_then(|status| status.code())),
            _ = tokio::time::sleep(input.timeout) => Ausgang::Zeitlimit,
            _ = cancel.cancelled() => Ausgang::Abbruch,
        };
        let mut timed_out = false;
        let code = match ausgang {
            Ausgang::Fertig(code) => code,
            Ausgang::Zeitlimit => {
                timed_out = true;
                let _ = sender.send((
                    "\n[dispatch] Zeitlimit erreicht – Claude wird beendet\n".to_string(),
                    OutputSource::Dispatch,
                ));
                self.beende(&mut group).await
            }
            Ausgang::Abbruch => {
                let _ = sender.send((
                    "\n[dispatch] Abgebrochen – Claude wird beendet\n".to_string(),
                    OutputSource::Dispatch,
                ));
                self.beende(&mut group).await
            }
        };

        // Hält ein übrig gebliebener Kindprozess die Ausgabe offen, nicht ewig darauf warten
        drop(sender);
        if tokio::time::timeout(OUTPUT_DRAIN, sammler).await.is_err() {
            // Der Sammler hängt an einer offenen Ausgabe; das bisher Gesammelte reicht
        }

        let collected = collected.lock().expect("Ausgabe-Sammler");
        let output = [collected.stdout.as_str(), collected.stderr.as_str()]
            .into_iter()
            .filter(|teil| !teil.trim().is_empty())
            .collect::<Vec<_>>()
            .join("\n");
        RunResult {
            code,
            timed_out,
            output,
        }
    }

    /// Beendet den Prozessbaum und wartet auf das Ende; greift das nicht, wird nach
    /// `hard_kill` der Prozess selbst hart beendet und nicht länger gewartet.
    async fn beende(&self, group: &mut ProcessGroup) -> Option<i32> {
        group.kill_tree();
        match tokio::time::timeout(self.hard_kill, group.wait()).await {
            Ok(ende) => ende.ok().and_then(|status| status.code()),
            Err(_) => {
                group.kill().await;
                None
            }
        }
    }

    /// Baut den Befehl: Auflösung wie in Node, Arbeitsordner, vollständige Umgebung, Rohre
    fn build(&self, input: &ClaudeRunInput) -> Result<Command, HarnessError> {
        let resolved = resolve_command(&input.command, &input.env)?;
        let mut command = if resolved.shell {
            // .cmd und .bat laufen nur über cmd.exe; die Argumente stammen aus der Einstellung
            let mut line = quote(&resolved.file)?;
            for arg in &input.args {
                line.push(' ');
                line.push_str(&quote(arg)?);
            }
            let mut command = Command::new("cmd");
            #[cfg(windows)]
            {
                // raw_arg: cmd.exe braucht die Zeile genau so, wie sie hier steht;
                // tokio::process::Command bringt raw_arg unter Windows selbst mit
                command.raw_arg("/d").raw_arg("/s").raw_arg("/c");
                command.raw_arg(format!("\"{line}\""));
            }
            #[cfg(not(windows))]
            {
                command.args(["/d", "/s", "/c", &line]);
            }
            command
        } else {
            let mut command = Command::new(&resolved.file);
            command.args(&input.args);
            command
        };
        command.current_dir(&input.cwd);
        command.env_clear();
        command.envs(&input.env);
        command.stdin(Stdio::piped());
        command.stdout(Stdio::piped());
        command.stderr(Stdio::piped());
        Ok(command)
    }
}

/// Aufgelöster Befehl: die zu startende Datei und ob cmd.exe sie starten muss
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedCommand {
    pub file: String,
    pub shell: bool,
}

/// Unter Windows startet nur cmd.exe eine `.cmd`, die `.exe` läuft direkt; ein Pfad wird
/// genommen, wie er ist, und muss existieren.
pub fn resolve_command(
    command: &str,
    env: &BTreeMap<String, String>,
) -> Result<ResolvedCommand, HarnessError> {
    let hat_pfad = command.contains('\\') || command.contains('/');
    let hat_endung = Path::new(command).extension().is_some();
    if !cfg!(windows) || hat_endung || hat_pfad {
        if hat_pfad && !Path::new(command).exists() {
            return Err(HarnessError::new(
                "claude-not-found",
                format!("Claude Code nicht gefunden: {command}"),
            ));
        }
        return Ok(ResolvedCommand {
            file: command.to_string(),
            shell: ist_stapeldatei(command),
        });
    }
    let pfad = env
        .get("PATH")
        .or_else(|| env.get("Path"))
        .cloned()
        .unwrap_or_default();
    // Wie Windows selbst: zuerst die Reihenfolge der PATH-Ordner, je Ordner .exe vor .cmd vor .bat
    for dir in pfad.split(TRENNER).filter(|teil| !teil.is_empty()) {
        for ext in [".exe", ".cmd", ".bat"] {
            let candidate = Path::new(dir).join(format!("{command}{ext}"));
            if candidate.exists() {
                return Ok(ResolvedCommand {
                    file: candidate.to_string_lossy().into_owned(),
                    shell: ext != ".exe",
                });
            }
        }
    }
    Ok(ResolvedCommand {
        file: command.to_string(),
        shell: false,
    })
}

/// Trennzeichen der Suchpfade (Windows `;`, sonst `:`)
const TRENNER: char = if cfg!(windows) { ';' } else { ':' };

/// Endet der Befehl auf `.cmd` oder `.bat` (Groß-/Kleinschreibung egal)?
fn ist_stapeldatei(command: &str) -> bool {
    let klein = command.to_lowercase();
    klein.ends_with(".cmd") || klein.ends_with(".bat")
}

/// Für cmd.exe quoten; die Argumente stammen aus der Einstellung, nicht aus Tickets
pub fn quote(arg: &str) -> Result<String, HarnessError> {
    if arg.contains(['"', '%', '!', '^']) {
        return Err(HarnessError::new(
            "claude-arg-unsafe",
            format!("Argument für claude enthält ein Zeichen, das cmd.exe nicht sicher weitergibt: {arg}"),
        ));
    }
    let harmlos = !arg.is_empty()
        && arg.chars().all(|zeichen| {
            zeichen.is_alphanumeric() && zeichen.is_ascii()
                || zeichen == '_'
                || matches!(zeichen, '-' | '.' | ':' | '/' | '\\' | '=')
        });
    Ok(if harmlos {
        arg.to_string()
    } else {
        format!("\"{arg}\"")
    })
}

/// Liest eine Ausgabe zeilenweise und schickt jedes Stück an den Sammler. Zeilenweise,
/// damit über Blockgrenzen geteilte UTF-8-Zeichen heil bleiben.
async fn lies_zeilenweise<R>(
    quelle: R,
    source: OutputSource,
    sender: UnboundedSender<(String, OutputSource)>,
) where
    R: tokio::io::AsyncRead + Unpin,
{
    let mut leser = BufReader::new(quelle);
    let mut puffer = Vec::new();
    loop {
        puffer.clear();
        match leser.read_until(b'\n', &mut puffer).await {
            Ok(0) | Err(_) => return,
            Ok(_) => {
                let text = String::from_utf8_lossy(&puffer).into_owned();
                if sender.send((text, source)).is_err() {
                    return;
                }
            }
        }
    }
}

/// Behält nur das Ende der Ausgabe; die angeschnittene erste Zeile fällt weg, damit kein
/// JSON-Bruchstück als Fehlertext zählt.
fn keep(text: String) -> String {
    if text.len() <= MAX_OUTPUT_CHARS {
        return text;
    }
    let zeichen = text.chars().count();
    if zeichen <= MAX_OUTPUT_CHARS {
        return text;
    }
    let start = text
        .char_indices()
        .nth(zeichen - MAX_OUTPUT_CHARS)
        .map(|(index, _)| index)
        .unwrap_or(0);
    let ende = &text[start..];
    match ende.find('\n') {
        Some(index) => ende[index + 1..].to_string(),
        None => ende.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn behaelt_nur_das_ende_und_wirft_die_angeschnittene_zeile_weg() {
        let text = format!("{}\nzweite Zeile\n", "a".repeat(MAX_OUTPUT_CHARS));
        let behalten = keep(text);
        assert_eq!(behalten, "zweite Zeile\n");
    }

    #[test]
    fn kurze_ausgaben_bleiben_unveraendert() {
        assert_eq!(keep("kurz\n".to_string()), "kurz\n");
    }

    #[test]
    fn quoten_lehnt_gefaehrliche_zeichen_ab() {
        let error = quote("a\"b").expect_err("Fehler");
        assert_eq!(error.code, "claude-arg-unsafe");
        assert!(quote("%PATH%").is_err());
        assert!(quote("a!b").is_err());
        assert!(quote("a^b").is_err());
    }

    #[test]
    fn quoten_laesst_harmlose_argumente_in_ruhe_und_klammert_den_rest() {
        assert_eq!(quote("-p").expect("ok"), "-p");
        assert_eq!(
            quote("C:\\pfad\\claude.cmd").expect("ok"),
            "C:\\pfad\\claude.cmd"
        );
        assert_eq!(quote("Bash(harness:*)").expect("ok"), "\"Bash(harness:*)\"");
    }

    #[test]
    fn ein_pfad_ohne_datei_meldet_claude_not_found() {
        let error = resolve_command("/gibt/es/nicht/claude", &BTreeMap::new()).expect_err("Fehler");
        assert_eq!(error.code, "claude-not-found");
    }
}
