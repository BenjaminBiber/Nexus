//! Anmeldung eines Claude-Accounts über `claude auth login`; portiert aus
//! `packages/server/tests/unit/AccountLogin.test.ts` des Referenzbranches.
//!
//! Statt eines echten Prozesses steuert der Test Ausgabe, Eingabe und Ende über Kanäle.

mod common;

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use nexus_core::accounts::{AccountPaths, ClaudeAccount};
use nexus_core::clock::Clock;
use nexus_server::accounts::login::AfterLogin;
use nexus_server::accounts::{AccountLoginService, LoginProcess, LoginSpawner};
use tokio::sync::{mpsc, oneshot};

use common::{TestServer, TestServerOptions};

/// Nachgebautes `claude auth login`: der Test schreibt die Ausgabe und bestimmt das Ende.
/// Alle Teile sind geteilt, damit der Test nichts über einen `await` hinweg sperrt
#[derive(Clone)]
struct FakeLogin {
    args: Vec<String>,
    env: BTreeMap<String, String>,
    print: mpsc::Sender<String>,
    exit: Arc<Mutex<Option<oneshot::Sender<Option<i32>>>>>,
    input: Arc<Mutex<Vec<String>>>,
    killed: Arc<Mutex<bool>>,
}

impl FakeLogin {
    /// Schreibt Text auf die Ausgabe des Prozesses
    async fn print(&self, text: &str) {
        self.print.send(text.to_string()).await.expect("Ausgabe");
        tick().await;
    }

    /// Beendet den Prozess mit diesem Code
    async fn exit(&self, code: i32) {
        if let Some(exit) = self.exit.lock().unwrap().take() {
            let _ = exit.send(Some(code));
        }
        tick().await;
    }

    /// Wurde der Prozess beendet?
    fn killed(&self) -> bool {
        *self.killed.lock().unwrap()
    }

    /// Was der Dienst auf die Eingabe geschrieben hat
    fn input(&self) -> Vec<String> {
        self.input.lock().unwrap().clone()
    }
}

/// Die gestarteten Anmeldungen eines Starters
type Started = Arc<Mutex<Vec<FakeLogin>>>;

/// Ein Starter, der jede Anmeldung als [`FakeLogin`] merkt
fn spawner() -> (LoginSpawner, Started) {
    let all: Started = Arc::new(Mutex::new(Vec::new()));
    let seen = all.clone();
    let spawner: LoginSpawner = Arc::new(
        move |_command: &str, args: &[String], env: &BTreeMap<String, String>| {
            let (out_sender, out_receiver) = mpsc::channel::<String>(16);
            let (in_sender, mut in_receiver) = mpsc::channel::<String>(8);
            let (exit_sender, exit_receiver) = oneshot::channel::<Option<i32>>();
            let input: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
            let written = input.clone();
            tokio::spawn(async move {
                while let Some(line) = in_receiver.recv().await {
                    written.lock().unwrap().push(line);
                }
            });
            let killed = Arc::new(Mutex::new(false));
            let flag = killed.clone();
            seen.lock().unwrap().push(FakeLogin {
                args: args.to_vec(),
                env: env.clone(),
                print: out_sender,
                exit: Arc::new(Mutex::new(Some(exit_sender))),
                input,
                killed,
            });
            Ok(LoginProcess {
                output: out_receiver,
                exit: exit_receiver,
                stdin: in_sender,
                kill: Arc::new(move || *flag.lock().unwrap() = true),
            })
        },
    );
    (spawner, all)
}

/// Die zuletzt gestartete Anmeldung
fn last(all: &Started) -> FakeLogin {
    all.lock().unwrap().last().cloned().expect("Prozess")
}

/// Lässt die begleitende Aufgabe der Anmeldung zum Zuge kommen
async fn tick() {
    for _ in 0..5 {
        tokio::task::yield_now().await;
    }
    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
}

/// Der Account, den die Tests anmelden
fn account() -> ClaudeAccount {
    ClaudeAccount {
        id: "max".into(),
        name: "Max".into(),
        config_dir: Some("/acc/max".into()),
        enabled: true,
        fallback: false,
        limit_five_hour: None,
        limit_week: None,
    }
}

/// Ein `after_login`, das diesen Text meldet
fn after(text: Option<&'static str>) -> (AfterLogin, Arc<Mutex<Vec<String>>>) {
    let calls: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let seen = calls.clone();
    let after: AfterLogin = Arc::new(move |account: ClaudeAccount| {
        seen.lock().unwrap().push(account.id);
        Box::pin(async move { text.map(str::to_string) })
    });
    (after, calls)
}

/// Dienst mit diesem Starter
fn service(after: AfterLogin, spawner: LoginSpawner) -> AccountLoginService {
    AccountLoginService::with_spawner(
        after,
        spawner,
        AccountPaths::new(BTreeMap::new().into_iter().collect(), "/home/ich"),
        Clock::system(),
    )
}

#[tokio::test]
async fn startet_claude_auth_login_im_ordner_des_accounts_mit_vorbelegter_email() {
    let (spawner, all) = spawner();
    let service = service(after(None).0, spawner);

    let dto = service.start(&account(), "claude", Some("max@example.com"));

    assert_eq!(dto.state, "running");
    let last = last(&all);
    assert_eq!(
        last.args,
        vec!["auth", "login", "--claudeai", "--email", "max@example.com"]
    );
    assert_eq!(
        last.env.get("CLAUDE_CONFIG_DIR").map(String::as_str),
        Some("/acc/max")
    );
    assert!(!last.env.contains_key("ANTHROPIC_API_KEY"));
    assert!(!last.env.contains_key("CLAUDECODE"));
}

#[tokio::test]
async fn merkt_sich_die_anmeldeseite_reicht_einen_code_weiter_und_richtet_nach_erfolg_ein() {
    let (spawner, all) = spawner();
    let (after, calls) = after(Some("Nexus-Plugin eingerichtet"));
    let service = service(after, spawner);
    service.start(&account(), "claude", None);
    let process = last(&all);

    process
        .print(
            "Opening browser… If it did not open, visit: \
https://claude.ai/oauth/authorize?code=true&x=1.\nPaste code here if prompted > ",
        )
        .await;

    let status = service.status("max").expect("Stand");
    assert_eq!(status.state, "running");
    assert_eq!(
        status.url.as_deref(),
        Some("https://claude.ai/oauth/authorize?code=true&x=1")
    );

    service.code("max", "  abc#123 ").expect("Code");
    tick().await;
    assert_eq!(process.input(), vec!["abc#123\n".to_string()]);

    process.exit(0).await;

    assert_eq!(*calls.lock().unwrap(), vec!["max".to_string()]);
    let status = service.status("max").expect("Stand");
    assert_eq!(status.state, "done");
    assert_eq!(status.plugin.as_deref(), Some("Nexus-Plugin eingerichtet"));
}

#[tokio::test]
async fn ein_code_ungleich_null_ist_gescheitert_und_abbrechen_beendet_den_prozess() {
    let (spawner, all) = spawner();
    let service = service(after(None).0, spawner);
    service.start(&account(), "claude", None);
    let process = last(&all);
    process.print("Login failed: denied").await;
    process.exit(1).await;

    let status = service.status("max").expect("Stand");
    assert_eq!(status.state, "failed");
    assert_eq!(status.output.trim(), "Login failed: denied");

    service.start(&account(), "claude", None);
    let second = last(&all);
    service.cancel("max", true);
    assert!(second.killed());
    assert_eq!(service.status("max").expect("Stand").state, "cancelled");
    let error = service.code("max", "x").expect_err("keine Anmeldung");
    assert!(error.to_string().contains("keine Anmeldung"), "{error}");
}

#[tokio::test]
async fn meldet_einen_nicht_startbaren_befehl_als_gescheitert() {
    let failing: LoginSpawner = Arc::new(|_, _, _| Err("ENOENT".to_string()));
    let service = service(after(None).0, failing);

    let dto = service.start(&account(), "claude", None);

    assert_eq!(dto.state, "failed");
    assert_eq!(
        dto.output,
        "Claude Code konnte nicht gestartet werden: ENOENT"
    );
}

#[tokio::test]
async fn code_und_abbrechen_ohne_laufende_anmeldung() {
    let (spawner, _all) = spawner();
    let server = TestServer::with(TestServerOptions {
        account_login_spawner: Some(spawner),
        ..TestServerOptions::default()
    });

    let code = server
        .post("/api/accounts/standard/login/code", r#"{"code":"x"}"#)
        .await;
    assert_eq!(code.status, 422);
    assert_eq!(
        server.delete("/api/accounts/standard/login").await.status,
        200
    );
}

#[tokio::test]
async fn ohne_startbares_claude_zeigt_die_liste_die_gescheiterte_anmeldung() {
    let failing: LoginSpawner = Arc::new(|_, _, _| Err("ENOENT".to_string()));
    let server = TestServer::with(TestServerOptions {
        account_login_spawner: Some(failing),
        ..TestServerOptions::default()
    });

    let reply = server.post("/api/accounts/standard/login", "{}").await;

    assert_eq!(reply.status, 200);
    let body: serde_json::Value = serde_json::from_str(&reply.text).expect("JSON");
    assert_eq!(body["accounts"][0]["login"]["state"], "failed");
}

#[tokio::test]
async fn legt_einen_account_an_meldet_ihn_an_und_benennt_ihn_nach_der_email() {
    let (spawner, all) = spawner();
    let server = TestServer::with(TestServerOptions {
        account_login_spawner: Some(spawner),
        ..TestServerOptions::default()
    });

    let started = server
        .post("/api/accounts/login", r#"{"name":"","email":""}"#)
        .await;

    assert_eq!(started.status, 200);
    let list: serde_json::Value = serde_json::from_str(&started.text).expect("JSON");
    let account = &list["accounts"][1];
    assert_eq!(account["id"], "neuer-account");
    assert_eq!(account["name"], "Neuer Account");
    assert_eq!(account["login"]["state"], "running");
    let process = last(&all);
    assert_eq!(
        process.env.get("CLAUDE_CONFIG_DIR").map(String::as_str),
        Some(
            server
                .home
                .accounts_dir()
                .join("neuer-account")
                .to_string_lossy()
                .as_ref()
        )
    );

    // Claude Code schreibt die Anmeldung in den Ordner des Accounts
    let dir = std::path::PathBuf::from(account["effectiveDir"].as_str().expect("wirksamer Ordner"));
    std::fs::create_dir_all(&dir).expect("Ordner");
    std::fs::write(
        dir.join(".claude.json"),
        r#"{"oauthAccount":{"emailAddress":"max@example.com"}}"#,
    )
    .expect("Identität");
    process.exit(0).await;

    let done: serde_json::Value =
        serde_json::from_str(&server.get("/api/accounts").await.text).expect("JSON");
    assert_eq!(done["accounts"][1]["name"], "max@example.com");
    assert_eq!(done["accounts"][1]["loggedIn"], true);
    assert_eq!(done["accounts"][1]["login"]["state"], "done");
}
