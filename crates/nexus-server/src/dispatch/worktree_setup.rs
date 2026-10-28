//! Führt im Worker die Einrichtungsbefehle eines neu angelegten Worktrees nacheinander aus
//! (vor Claude); portiert aus `packages/server/src/dispatch/WorktreeSetupRunner.ts`.
//!
//! Shell wie für Claude: unter Windows die Git Bash aus `CLAUDE_CODE_GIT_BASH_PATH`, sonst
//! `cmd`; sonst `/bin/sh`. Beim ersten Fehlschlag (Exit ≠ 0, Zeitlimit) ist Schluss; das
//! Ergebnis nennt den Befehl.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use nexus_core::process::ProcessGroup;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};

/// So viele Zeichen der Ausgabe eines Befehls werden behalten
const MAX_OUTPUT_CHARS: usize = 20_000;

/// Wie lange nach dem Beenden im Zeitlimit höchstens auf das Ende gewartet wird
const NACHLAUF: Duration = Duration::from_secs(5);

/// Ein Einrichtungsbefehl: Ordner (relativ zum Worktree) und Befehlszeile
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorktreeSetupStep {
    pub cwd: String,
    pub run: String,
}

/// Gescheiterter Einrichtungsbefehl samt Ergebnis
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetupFailure {
    pub cwd: String,
    pub run: String,
    pub code: Option<i32>,
    pub timed_out: bool,
    pub output: String,
}

/// Ergebnis der Einrichtung: alle Befehle gelaufen oder einer gescheitert
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SetupOutcome {
    Ok,
    Failed(SetupFailure),
}

/// Alles, was die Einrichtung braucht
#[derive(Debug, Clone)]
pub struct SetupInput {
    pub steps: Vec<WorktreeSetupStep>,
    /// Worktree des Laufs; der Ordner der Schritte ist relativ dazu
    pub root: PathBuf,
    pub env: BTreeMap<String, String>,
    pub timeout: Duration,
}

/// Führt die Einrichtungsbefehle eines Worktrees nacheinander aus.
#[derive(Debug, Clone, Copy, Default)]
pub struct WorktreeSetupRunner;

impl WorktreeSetupRunner {
    /// Neuer Läufer (hält keinen Zustand)
    pub fn new() -> Self {
        Self
    }

    /// Liefert das Ergebnis der Einrichtung; endet nie mit einem Fehler.
    /// `on_output` bekommt die Kopfzeilen und die Ausgabe der Befehle fürs Lauf-Protokoll.
    pub async fn run(
        &self,
        input: SetupInput,
        on_output: impl FnMut(&str) + Send + 'static,
    ) -> SetupOutcome {
        let melden = Arc::new(Mutex::new(on_output));
        for step in &input.steps {
            melde(
                &melden,
                &format!("[dispatch] Einrichtung: {}$ {}\n", step.cwd, step.run),
            );
            let cwd = input.root.join(&step.cwd);
            if !im_worktree(&input.root, &cwd) || !cwd.is_dir() {
                let output = format!("Ordner {} gibt es im Worktree nicht.", step.cwd);
                melde(&melden, &format!("[dispatch] {output}\n"));
                return SetupOutcome::Failed(SetupFailure {
                    cwd: step.cwd.clone(),
                    run: step.run.clone(),
                    code: None,
                    timed_out: false,
                    output,
                });
            }
            let ende = self.exec(&step.run, &cwd, &input, &melden).await;
            melde(
                &melden,
                &format!(
                    "[dispatch] Einrichtung beendet: Exit-Code {}{}\n",
                    ende.code
                        .map(|code| code.to_string())
                        .unwrap_or_else(|| "null".to_string()),
                    if ende.timed_out { ", Zeitlimit" } else { "" }
                ),
            );
            if ende.code != Some(0) || ende.timed_out {
                return SetupOutcome::Failed(SetupFailure {
                    cwd: step.cwd.clone(),
                    run: step.run.clone(),
                    code: ende.code,
                    timed_out: ende.timed_out,
                    output: ende.output,
                });
            }
        }
        SetupOutcome::Ok
    }

    /// Ein Befehl in der Shell; sammelt das Ende der Ausgabe
    async fn exec(
        &self,
        run: &str,
        cwd: &Path,
        input: &SetupInput,
        melden: &Arc<Mutex<impl FnMut(&str) + Send + 'static>>,
    ) -> Ende {
        let mut command = self.shell(run, &input.env);
        command.current_dir(cwd);
        command.env_clear();
        command.envs(&input.env);
        command.stdin(Stdio::null());
        command.stdout(Stdio::piped());
        command.stderr(Stdio::piped());
        let mut group = match ProcessGroup::spawn(&mut command) {
            Ok(group) => group,
            Err(error) => {
                let text = format!("\n{error}\n");
                melde(melden, &text);
                return Ende {
                    code: Some(127),
                    timed_out: false,
                    output: text,
                };
            }
        };

        let (sender, mut receiver) = unbounded_channel::<String>();
        let gesammelt = Arc::new(Mutex::new(String::new()));
        let sammler = tokio::spawn({
            let gesammelt = Arc::clone(&gesammelt);
            let melden = Arc::clone(melden);
            async move {
                while let Some(chunk) = receiver.recv().await {
                    melde(&melden, &chunk);
                    let mut gesammelt = gesammelt.lock().expect("Ausgabe-Sammler");
                    let text = std::mem::take(&mut *gesammelt) + &chunk;
                    *gesammelt = ende_behalten(text);
                }
            }
        });
        if let Some(stdout) = group.take_stdout() {
            tokio::spawn(lies_zeilenweise(stdout, sender.clone()));
        }
        if let Some(stderr) = group.take_stderr() {
            tokio::spawn(lies_zeilenweise(stderr, sender.clone()));
        }

        let mut timed_out = false;
        let fertig = tokio::select! {
            ende = group.wait() => Some(ende.ok().and_then(|status| status.code())),
            _ = tokio::time::sleep(input.timeout) => None,
        };
        let code = match fertig {
            Some(code) => code,
            None => {
                timed_out = true;
                let _ = sender.send(
                    "\n[dispatch] Zeitlimit der Einrichtung erreicht – Befehl wird beendet\n"
                        .to_string(),
                );
                group.kill_tree();
                match tokio::time::timeout(NACHLAUF, group.wait()).await {
                    Ok(ende) => ende.ok().and_then(|status| status.code()),
                    Err(_) => None,
                }
            }
        };
        drop(sender);
        if tokio::time::timeout(NACHLAUF, sammler).await.is_err() {
            // Ein übrig gebliebener Kindprozess hält die Ausgabe offen; das Gesammelte reicht
        }
        let output = gesammelt.lock().expect("Ausgabe-Sammler").clone();
        Ende {
            code,
            timed_out,
            output,
        }
    }

    /// Die Shell des Befehls: unter Windows die Git Bash aus der Einstellung, sonst `cmd`;
    /// auf den anderen Systemen `/bin/sh`.
    fn shell(&self, run: &str, env: &BTreeMap<String, String>) -> Command {
        if cfg!(windows) {
            let bash = env.get("CLAUDE_CODE_GIT_BASH_PATH");
            if let Some(bash) = bash.filter(|pfad| Path::new(pfad).exists()) {
                let mut command = Command::new(bash);
                command.arg("-c").arg(run);
                return command;
            }
            let mut command = Command::new("cmd");
            #[cfg(windows)]
            {
                // raw_arg: cmd.exe bekommt die Zeile genau so, wie sie im Ticket steht;
                // tokio::process::Command bringt raw_arg unter Windows selbst mit
                command.raw_arg("/d").raw_arg("/s").raw_arg("/c");
                command.raw_arg(run);
            }
            #[cfg(not(windows))]
            {
                command.args(["/d", "/s", "/c", run]);
            }
            return command;
        }
        let mut command = Command::new("/bin/sh");
        command.arg("-c").arg(run);
        command
    }
}

/// Ergebnis eines einzelnen Befehls
#[derive(Debug, Clone, PartialEq, Eq)]
struct Ende {
    code: Option<i32>,
    timed_out: bool,
    output: String,
}

/// Ruft die Protokollfunktion auf
fn melde(melden: &Arc<Mutex<impl FnMut(&str) + Send + 'static>>, text: &str) {
    (melden.lock().expect("Protokoll"))(text);
}

/// Liegt der Ordner im Worktree (und nicht darüber oder daneben)?
fn im_worktree(root: &Path, cwd: &Path) -> bool {
    let root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let Ok(cwd) = cwd.canonicalize() else {
        return false;
    };
    cwd.starts_with(&root)
}

/// Liest eine Ausgabe zeilenweise, damit geteilte UTF-8-Zeichen heil bleiben
async fn lies_zeilenweise<R>(quelle: R, sender: UnboundedSender<String>)
where
    R: tokio::io::AsyncRead + Unpin,
{
    let mut leser = BufReader::new(quelle);
    let mut puffer = Vec::new();
    loop {
        puffer.clear();
        match leser.read_until(b'\n', &mut puffer).await {
            Ok(0) | Err(_) => return,
            Ok(_) => {
                if sender
                    .send(String::from_utf8_lossy(&puffer).into_owned())
                    .is_err()
                {
                    return;
                }
            }
        }
    }
}

/// Behält nur das Ende der Ausgabe
fn ende_behalten(text: String) -> String {
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
    text[start..].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn behaelt_nur_das_ende_der_ausgabe() {
        let text = format!("{}z", "a".repeat(MAX_OUTPUT_CHARS));
        let behalten = ende_behalten(text);
        assert_eq!(behalten.chars().count(), MAX_OUTPUT_CHARS);
        assert!(behalten.ends_with('z'));
    }
}
