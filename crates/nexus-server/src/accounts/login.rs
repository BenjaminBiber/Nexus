//! Meldet einen Claude-Account mit `claude auth login` in seinem Ordner an; portiert aus
//! `packages/server/src/usage/AccountLoginService.ts`.
//!
//! Claude Code öffnet die Anmeldeseite im Browser und speichert die Anmeldung selbst
//! (Schlüsselbund bzw. `.credentials.json`). Fragt es nach einem Code, reicht [`code`] ihn
//! weiter. Nach Erfolg läuft `after_login` (Plugin einrichten, Name aus der E-Mail). Je
//! Account höchstens eine Anmeldung gleichzeitig.
//!
//! [`code`]: AccountLoginService::code

use std::collections::{BTreeMap, HashMap};
use std::future::Future;
use std::pin::Pin;
use std::process::Stdio;
use std::sync::{Arc, LazyLock, Mutex};
use std::time::Duration;

use nexus_core::accounts::{AccountPaths, ClaudeAccount};
use nexus_core::clock::Clock;
use nexus_core::{HarnessError, Result};
use regex::Regex;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::mpsc;

use crate::accounts::plugin::account_env;

/// Zeitlimit einer Anmeldung
const TIMEOUT: Duration = Duration::from_secs(10 * 60);
/// So viele Zeichen der Ausgabe bleiben erhalten (das Ende)
const MAX_OUTPUT_CHARS: usize = 4_000;

/// Erste Adresse in der Ausgabe
static URL_PATTERN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"https://\S+").unwrap());

/// Stand einer Anmeldung über `claude auth login`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountLoginDto {
    /// `running`, `done`, `failed` oder `cancelled`
    pub state: String,
    pub started_at: String,
    /// Anmeldeseite aus der Ausgabe – falls sich kein Browser geöffnet hat
    pub url: Option<String>,
    /// Letzte Ausgabe (gekürzt), bei Fehlern der Grund
    pub output: String,
    /// Ergebnis der Plugin-Einrichtung nach der Anmeldung; null = noch nicht bzw. nicht nötig
    pub plugin: Option<String>,
}

/// Was vom gestarteten Prozess gebraucht wird (Tests reichen eine Attrappe)
pub struct LoginProcess {
    /// Ausgabe des Prozesses (Standardausgabe und Standardfehler gemischt)
    pub output: mpsc::Receiver<String>,
    /// Endet mit dem Prozess; der Wert ist sein Code (`None` = durch Signal beendet)
    pub exit: tokio::sync::oneshot::Receiver<Option<i32>>,
    /// Eingabe des Prozesses (der Code aus dem Browser)
    pub stdin: mpsc::Sender<String>,
    /// Beendet den Prozess
    pub kill: Arc<dyn Fn() + Send + Sync>,
}

/// Startet `claude auth login`; Tests setzen eine Attrappe ein
pub type LoginSpawner = Arc<
    dyn Fn(&str, &[String], &BTreeMap<String, String>) -> std::result::Result<LoginProcess, String>
        + Send
        + Sync,
>;

/// Was nach einer erfolgreichen Anmeldung läuft (Name aus der E-Mail, Plugin einrichten);
/// der Rückgabewert erscheint als `plugin` im Stand
pub type AfterLogin = Arc<
    dyn Fn(ClaudeAccount) -> Pin<Box<dyn Future<Output = Option<String>> + Send>> + Send + Sync,
>;

/// Eine bekannte Anmeldung
struct Login {
    dto: Arc<Mutex<AccountLoginDto>>,
    stdin: Option<mpsc::Sender<String>>,
    kill: Option<Arc<dyn Fn() + Send + Sync>>,
    task: Option<tokio::task::JoinHandle<()>>,
}

impl Login {
    /// Der Stand als Kopie
    fn dto(&self) -> AccountLoginDto {
        self.dto.lock().expect("Sperre der Anmeldung").clone()
    }

    /// Läuft die Anmeldung noch?
    fn running(&self) -> bool {
        self.dto.lock().expect("Sperre der Anmeldung").state == "running"
    }
}

/// Meldet Claude-Accounts an und kennt den Stand jeder Anmeldung.
pub struct AccountLoginService {
    after_login: AfterLogin,
    spawner: LoginSpawner,
    paths: AccountPaths,
    clock: Clock,
    logins: Mutex<HashMap<String, Login>>,
}

impl std::fmt::Debug for AccountLoginService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AccountLoginService")
            .finish_non_exhaustive()
    }
}

impl AccountLoginService {
    /// Dienst mit echtem Prozessstart
    pub fn new(after_login: AfterLogin, paths: AccountPaths, clock: Clock) -> Self {
        Self::with_spawner(after_login, system_spawner(), paths, clock)
    }

    /// Dienst mit eigenem Prozessstart (Tests)
    pub fn with_spawner(
        after_login: AfterLogin,
        spawner: LoginSpawner,
        paths: AccountPaths,
        clock: Clock,
    ) -> Self {
        Self {
            after_login,
            spawner,
            paths,
            clock,
            logins: Mutex::new(HashMap::new()),
        }
    }

    /// Startet die Anmeldung; eine laufende des Accounts wird vorher beendet
    pub fn start(
        &self,
        account: &ClaudeAccount,
        command: &str,
        email: Option<&str>,
    ) -> AccountLoginDto {
        self.cancel(&account.id, false);
        let files = self.paths.files(account.config_dir.as_deref());
        let env = account_env(&std::env::vars().collect(), files.override_dir.as_deref());
        let email = email.map(str::trim).filter(|email| !email.is_empty());
        let mut args = vec![
            "auth".to_string(),
            "login".to_string(),
            "--claudeai".to_string(),
        ];
        if let Some(email) = email {
            args.push("--email".to_string());
            args.push(email.to_string());
        }
        let dto = Arc::new(Mutex::new(AccountLoginDto {
            state: "running".to_string(),
            started_at: self.clock.iso(),
            url: None,
            output: String::new(),
            plugin: None,
        }));
        let process = match (self.spawner)(command, &args, &env) {
            Ok(process) => process,
            Err(error) => {
                let mut stand = dto.lock().expect("Sperre der Anmeldung");
                stand.state = "failed".to_string();
                stand.output = format!("Claude Code konnte nicht gestartet werden: {error}");
                let out = stand.clone();
                drop(stand);
                self.logins.lock().expect("Sperre der Anmeldungen").insert(
                    account.id.clone(),
                    Login {
                        dto,
                        stdin: None,
                        kill: None,
                        task: None,
                    },
                );
                return out;
            }
        };
        let kill = process.kill.clone();
        let stdin = process.stdin.clone();
        let task = tokio::spawn(watch(
            process,
            dto.clone(),
            account.clone(),
            self.after_login.clone(),
        ));
        let out = dto.lock().expect("Sperre der Anmeldung").clone();
        self.logins.lock().expect("Sperre der Anmeldungen").insert(
            account.id.clone(),
            Login {
                dto,
                stdin: Some(stdin),
                kill: Some(kill),
                task: Some(task),
            },
        );
        out
    }

    /// Stand der Anmeldung oder `None`
    pub fn status(&self, account_id: &str) -> Option<AccountLoginDto> {
        self.logins
            .lock()
            .expect("Sperre der Anmeldungen")
            .get(account_id)
            .map(Login::dto)
    }

    /// Alle bekannten Anmeldungen (für die Liste)
    pub fn all(&self) -> BTreeMap<String, AccountLoginDto> {
        self.logins
            .lock()
            .expect("Sperre der Anmeldungen")
            .iter()
            .map(|(id, login)| (id.clone(), login.dto()))
            .collect()
    }

    /// Code aus dem Browser an die laufende Anmeldung weitergeben
    pub fn code(&self, account_id: &str, code: &str) -> Result<()> {
        let sender = {
            let logins = self.logins.lock().expect("Sperre der Anmeldungen");
            let login = logins.get(account_id).filter(|login| login.running());
            match login.and_then(|login| login.stdin.clone()) {
                Some(sender) => sender,
                None => {
                    return Err(HarnessError::new(
                        "account-login-not-running",
                        "Für diesen Account läuft keine Anmeldung.",
                    )
                    .into())
                }
            }
        };
        let text = code.trim();
        if text.is_empty() {
            return Err(HarnessError::new("input-invalid", "Der Code ist leer.").into());
        }
        let _ = sender.try_send(format!("{text}\n"));
        Ok(())
    }

    /// Bricht eine laufende Anmeldung ab; `keep` false vergisst auch den Stand
    pub fn cancel(&self, account_id: &str, keep: bool) {
        let mut logins = self.logins.lock().expect("Sperre der Anmeldungen");
        if let Some(login) = logins.get_mut(account_id) {
            if login.running() {
                login.dto.lock().expect("Sperre der Anmeldung").state = "cancelled".to_string();
                if let Some(kill) = &login.kill {
                    kill();
                }
            }
            if let Some(task) = login.task.take() {
                task.abort();
            }
        }
        if !keep {
            logins.remove(account_id);
        }
    }

    /// Beendet alle laufenden Anmeldungen (der Server fährt herunter)
    pub fn close(&self) {
        let ids: Vec<String> = self
            .logins
            .lock()
            .expect("Sperre der Anmeldungen")
            .keys()
            .cloned()
            .collect();
        for id in ids {
            self.cancel(&id, true);
        }
    }
}

/// Begleitet eine laufende Anmeldung: Ausgabe sammeln, Zeitlimit wachen, nach Erfolg
/// einrichten
async fn watch(
    mut process: LoginProcess,
    dto: Arc<Mutex<AccountLoginDto>>,
    account: ClaudeAccount,
    after_login: AfterLogin,
) {
    let collect = {
        let dto = dto.clone();
        async move {
            while let Some(chunk) = process.output.recv().await {
                let mut stand = dto.lock().expect("Sperre der Anmeldung");
                stand.output = tail(&format!("{}{chunk}", stand.output));
                if stand.url.is_none() {
                    stand.url = first_url(&stand.output);
                }
            }
        }
    };
    let code = tokio::select! {
        _ = collect => process.exit.await.ok().flatten(),
        exit = &mut process.exit => exit.ok().flatten(),
        _ = tokio::time::sleep(TIMEOUT) => {
            let mut stand = dto.lock().expect("Sperre der Anmeldung");
            stand.output = tail(&format!(
                "{}\nZeitlimit von 10 Minuten erreicht – Anmeldung abgebrochen.",
                stand.output
            ));
            stand.state = "failed".to_string();
            drop(stand);
            (process.kill)();
            return;
        }
    };
    {
        let stand = dto.lock().expect("Sperre der Anmeldung");
        if stand.state != "running" {
            return;
        }
    }
    if code != Some(0) {
        dto.lock().expect("Sperre der Anmeldung").state = "failed".to_string();
        (process.kill)();
        return;
    }
    // Erst nach dem Einrichten gilt die Anmeldung als fertig, damit die Oberfläche den Stand
    // mit Plugin zeigt
    let plugin = after_login(account).await;
    let mut stand = dto.lock().expect("Sperre der Anmeldung");
    stand.plugin = plugin;
    stand.state = "done".to_string();
}

/// Die letzten [`MAX_OUTPUT_CHARS`] Zeichen
fn tail(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= MAX_OUTPUT_CHARS {
        return text.to_string();
    }
    chars[chars.len() - MAX_OUTPUT_CHARS..].iter().collect()
}

/// Die erste `https`-Adresse der Ausgabe, ohne Satzzeichen am Ende
fn first_url(output: &str) -> Option<String> {
    let found = URL_PATTERN.find(output)?.as_str();
    Some(
        found
            .trim_end_matches([')', ']', '.', ',', '\'', '"'])
            .to_string(),
    )
}

/// Startet `claude auth login` wirklich: Ausgabe und Eingabe über Kanäle
pub fn system_spawner() -> LoginSpawner {
    Arc::new(
        |command: &str, args: &[String], env: &BTreeMap<String, String>| {
            let mut child = tokio::process::Command::new(command)
                .args(args)
                .env_clear()
                .envs(env)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .kill_on_drop(true)
                .spawn()
                .map_err(|error| error.to_string())?;
            let (out_sender, out_receiver) = mpsc::channel::<String>(64);
            let (in_sender, mut in_receiver) = mpsc::channel::<String>(8);
            let (exit_sender, exit_receiver) = tokio::sync::oneshot::channel();
            for pipe in [
                child.stdout.take().map(OutPipe::Out),
                child.stderr.take().map(OutPipe::Err),
            ]
            .into_iter()
            .flatten()
            {
                let sender = out_sender.clone();
                tokio::spawn(async move {
                    match pipe {
                        OutPipe::Out(pipe) => pump(pipe, sender).await,
                        OutPipe::Err(pipe) => pump(pipe, sender).await,
                    }
                });
            }
            drop(out_sender);
            if let Some(mut stdin) = child.stdin.take() {
                tokio::spawn(async move {
                    while let Some(line) = in_receiver.recv().await {
                        if stdin.write_all(line.as_bytes()).await.is_err() {
                            return;
                        }
                        let _ = stdin.flush().await;
                    }
                });
            }
            let pid = child.id();
            tokio::spawn(async move {
                let code = child.wait().await.ok().and_then(|status| status.code());
                let _ = exit_sender.send(code);
            });
            Ok(LoginProcess {
                output: out_receiver,
                exit: exit_receiver,
                stdin: in_sender,
                kill: Arc::new(move || {
                    if let Some(pid) = pid {
                        nexus_core::process::terminate(pid);
                    }
                }),
            })
        },
    )
}

/// Standardausgabe oder Standardfehler eines Prozesses
enum OutPipe {
    Out(tokio::process::ChildStdout),
    Err(tokio::process::ChildStderr),
}

/// Schiebt jede Zeile einer Ausgabe in den Kanal
async fn pump<R: tokio::io::AsyncRead + Unpin>(pipe: R, sender: mpsc::Sender<String>) {
    let mut lines = BufReader::new(pipe).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        if sender.send(format!("{line}\n")).await.is_err() {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kuerzt_die_ausgabe_auf_das_ende() {
        let long = "x".repeat(MAX_OUTPUT_CHARS + 10);
        assert_eq!(tail(&long).chars().count(), MAX_OUTPUT_CHARS);
        assert_eq!(tail("kurz"), "kurz");
    }

    #[test]
    fn findet_die_erste_adresse_ohne_satzzeichen() {
        assert_eq!(
            first_url("Bitte öffnen: https://claude.ai/oauth?code=1).").as_deref(),
            Some("https://claude.ai/oauth?code=1")
        );
        assert_eq!(first_url("ohne Adresse"), None);
    }
}
