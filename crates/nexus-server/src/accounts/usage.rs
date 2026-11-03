//! Anmeldung, Plugin und Abo-Limits je Claude-Account; portiert aus
//! `packages/server/src/usage/AccountUsageService.ts`.
//!
//! Das Token kommt unter macOS aus dem Schlüsselbund (Dienstname, wie Claude Code ihn je
//! Ordner bildet), sonst – und als Rückfall – aus `.credentials.json`. Die Limits liest je
//! Account ein eigener [`ClaudeUsageService`] (Zwischenspeicher je Account).

use std::collections::{BTreeMap, HashMap};
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use nexus_core::accounts::{
    has_plugin, AccountPaths, AccountStore, AccountTokenStore, ClaudeAccount,
};
use nexus_core::clock::Clock;
use nexus_core::dispatch::AccountState;
use serde::{Deserialize, Serialize};

use crate::oauth_refresh::{file_refresh, TokenPost};
use crate::usage::{
    ClaudeUsageDto, ClaudeUsageService, ReadCredentials, UsageFetch, DEFAULT_TTL_MS,
};

/// Programm, das Einträge des macOS-Schlüsselbunds liest
const SECURITY: &str = "/usr/bin/security";
/// Zeitlimit eines Schlüsselbund-Aufrufs
const SECURITY_TIMEOUT: Duration = Duration::from_secs(5);

/// Ein Account samt Anmeldung, Plugin und Limits für die Oberfläche
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountInfoDto {
    pub id: String,
    pub name: String,
    pub config_dir: Option<String>,
    /// Wirksamer Ordner (aufgelöst)
    pub effective_dir: String,
    pub enabled: bool,
    /// Nur als Fallback: der Dispatcher nimmt ihn erst, wenn kein anderer erlaubter Account
    /// frei ist
    pub fallback: bool,
    /// Eigene Obergrenzen in Prozent; null = Vorgabe des Dispatchers
    pub limit_five_hour: Option<i64>,
    pub limit_week: Option<i64>,
    /// Anmeldung gefunden (`oauthAccount` in `.claude.json` oder ein Token)
    pub logged_in: bool,
    pub email: Option<String>,
    /// Nexus-Plugin in der Plugin-Liste von Claude Code
    pub plugin: bool,
    /// Läufe nutzen ein Langzeit-Token (`claude setup-token`); das Token selbst geht nie
    /// hinaus
    pub run_token: bool,
    pub usage: ClaudeUsageDto,
    /// Pause des Dispatchers für diesen Account (ISO)
    pub paused_until: Option<String>,
}

/// Limits eines aktiven Accounts für die Ringe in der Seitenleiste
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountUsageEntryDto {
    pub id: String,
    pub name: String,
    pub fallback: bool,
    /// Eigene Obergrenzen in Prozent; null = Vorgabe des Dispatchers
    pub limit_five_hour: Option<i64>,
    pub limit_week: Option<i64>,
    pub usage: ClaudeUsageDto,
}

/// Die Liste der Accounts; Tests reichen eine Attrappe statt des Speichers
pub trait AccountList: Send + Sync {
    /// Alle Accounts, der Standard zuerst
    fn list(&self) -> Vec<ClaudeAccount>;
}

impl AccountList for AccountStore {
    fn list(&self) -> Vec<ClaudeAccount> {
        AccountStore::list(self)
    }
}

impl AccountList for Vec<ClaudeAccount> {
    fn list(&self) -> Vec<ClaudeAccount> {
        self.clone()
    }
}

/// Limits und Zustand der Accounts, wie der Dispatcher sie braucht; Tests reichen eine
/// Attrappe statt des echten Dienstes
#[async_trait::async_trait]
pub trait AccountLimits: Send + Sync {
    /// Zustand für die Account-Wahl; `with_usage` false fragt Anthropic nicht
    async fn states(
        &self,
        accounts: &[ClaudeAccount],
        pauses: &BTreeMap<String, String>,
        with_usage: bool,
    ) -> BTreeMap<String, AccountState>;

    /// Limits eines Accounts; `fresh` umgeht den Zwischenspeicher
    async fn usage(&self, account: &ClaudeAccount, fresh: bool) -> ClaudeUsageDto;
}

#[async_trait::async_trait]
impl AccountLimits for AccountUsageService {
    async fn states(
        &self,
        accounts: &[ClaudeAccount],
        pauses: &BTreeMap<String, String>,
        with_usage: bool,
    ) -> BTreeMap<String, AccountState> {
        AccountUsageService::states(self, accounts, pauses, with_usage).await
    }

    async fn usage(&self, account: &ClaudeAccount, fresh: bool) -> ClaudeUsageDto {
        AccountUsageService::usage(self, account, fresh).await
    }
}

/// Liest ein Token aus dem macOS-Schlüsselbund (Dienstname → Inhalt oder `None`)
pub type Keychain =
    Arc<dyn Fn(String) -> Pin<Box<dyn Future<Output = Option<String>> + Send>> + Send + Sync>;

/// Was den Dienst von der Umgebung des Rechners löst (Tests setzen hier ein)
pub struct AccountUsageOptions {
    pub paths: AccountPaths,
    /// Name der Plattform wie in Node (`win32`, `darwin`, `linux`)
    pub platform: String,
    pub keychain: Keychain,
    /// Abruf des OAuth-Endpunkts; `None` = der echte über HTTPS
    pub fetch: Option<UsageFetch>,
    pub clock: Clock,
    /// Langzeit-Tokens der Läufe. Hat ein Account eins, erneuert der Dienst ein abgelaufenes
    /// Token in seinem Ordner selbst (sonst täte es niemand mehr); `None` = nie
    pub tokens: Option<AccountTokenStore>,
    /// Aufruf des Token-Endpunkts beim Erneuern; `None` = der echte über HTTPS
    pub token_post: Option<TokenPost>,
}

impl Default for AccountUsageOptions {
    fn default() -> Self {
        Self {
            paths: AccountPaths::from_process_env(),
            platform: crate::dispatch::context::node_platform().to_string(),
            keychain: system_keychain(),
            fetch: None,
            clock: Clock::system(),
            tokens: None,
            token_post: None,
        }
    }
}

/// Ein Dienst je Account, erkennbar an seinem Ordner
struct PerAccount {
    dir: PathBuf,
    service: Arc<ClaudeUsageService>,
}

/// Anmeldung, Plugin und Abo-Limits je Claude-Account.
pub struct AccountUsageService {
    store: Arc<dyn AccountList>,
    options: AccountUsageOptions,
    services: Mutex<HashMap<String, PerAccount>>,
}

impl std::fmt::Debug for AccountUsageService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AccountUsageService")
            .field("platform", &self.options.platform)
            .finish_non_exhaustive()
    }
}

impl AccountUsageService {
    /// Dienst über diesem Speicher mit der Umgebung des Rechners
    pub fn new(store: Arc<dyn AccountList>, clock: Clock) -> Self {
        Self::with_options(
            store,
            AccountUsageOptions {
                clock,
                ..AccountUsageOptions::default()
            },
        )
    }

    /// Dienst mit eigenen Pfaden, Schlüsselbund und Abruf (Tests)
    pub fn with_options(store: Arc<dyn AccountList>, options: AccountUsageOptions) -> Self {
        Self {
            store,
            options,
            services: Mutex::new(HashMap::new()),
        }
    }

    /// Limits des Standard-Accounts (Ring in der Seitenleiste)
    pub async fn current(&self) -> ClaudeUsageDto {
        match self.store.list().first() {
            Some(standard) => self.usage(standard, false).await,
            None => ClaudeUsageDto::unavailable("no-login", None),
        }
    }

    /// Limits aller aktiven Accounts (Ringe in der Seitenleiste); ohne Anmeldedatei,
    /// Identität und Plugin
    pub async fn overview(&self) -> Vec<AccountUsageEntryDto> {
        let accounts: Vec<ClaudeAccount> = self
            .store
            .list()
            .into_iter()
            .filter(|account| account.enabled)
            .collect();
        let mut out = Vec::with_capacity(accounts.len());
        for account in accounts {
            let usage = self.usage(&account, false).await;
            out.push(AccountUsageEntryDto {
                id: account.id,
                name: account.name,
                fallback: account.fallback,
                limit_five_hour: account.limit_five_hour,
                limit_week: account.limit_week,
                usage,
            });
        }
        out
    }

    /// Limits eines Accounts; `fresh` umgeht den Zwischenspeicher
    pub async fn usage(&self, account: &ClaudeAccount, fresh: bool) -> ClaudeUsageDto {
        self.service(account).current(fresh).await
    }

    /// Alle Accounts mit Anmeldung, Plugin und Limits; `with_usage` false fragt Anthropic nicht
    pub async fn infos(
        &self,
        pauses: &BTreeMap<String, String>,
        with_usage: bool,
    ) -> Vec<AccountInfoDto> {
        let mut out = Vec::new();
        for account in self.store.list() {
            let paused = pauses.get(&account.id).cloned();
            out.push(self.info(&account, paused, with_usage).await);
        }
        out
    }

    /// Ein Account mit Anmeldung, Plugin und (optional) Limits
    pub async fn info(
        &self,
        account: &ClaudeAccount,
        paused_until: Option<String>,
        with_usage: bool,
    ) -> AccountInfoDto {
        let files = self.options.paths.files(account.config_dir.as_deref());
        let identity = identity(&files.identity_file);
        let plugin = std::fs::read_to_string(&files.plugins_file)
            .map(|text| has_plugin(&text))
            .unwrap_or(false);
        let usage = match with_usage {
            true => Some(self.usage(account, false).await),
            false => None,
        };
        let logged_in = identity.is_some()
            || usage
                .as_ref()
                .is_some_and(|usage| usage.reason.as_deref() != Some("no-login"));
        AccountInfoDto {
            id: account.id.clone(),
            name: account.name.clone(),
            config_dir: account.config_dir.clone(),
            effective_dir: files.config_dir.to_string_lossy().into_owned(),
            enabled: account.enabled,
            fallback: account.fallback,
            limit_five_hour: account.limit_five_hour,
            limit_week: account.limit_week,
            logged_in,
            email: identity,
            plugin,
            run_token: self
                .options
                .tokens
                .as_ref()
                .is_some_and(|tokens| tokens.has(&account.id)),
            usage: usage.unwrap_or_else(|| ClaudeUsageDto::unavailable("disabled", None)),
            paused_until,
        }
    }

    /// Zustand für die Account-Wahl des Dispatchers; ohne `with_usage` (nur ein Account im
    /// Spiel) ohne Anfrage bei Anthropic
    pub async fn states(
        &self,
        accounts: &[ClaudeAccount],
        pauses: &BTreeMap<String, String>,
        with_usage: bool,
    ) -> BTreeMap<String, AccountState> {
        let mut out = BTreeMap::new();
        for account in accounts {
            let paused = pauses.get(&account.id).cloned();
            let info = self
                .info(account, paused, with_usage && account.enabled)
                .await;
            out.insert(
                account.id.clone(),
                AccountState {
                    // Ohne Usage-Abfrage gilt der Standard-Account als angemeldet: so startet
                    // der Dispatcher mit einem Account wie bisher
                    logged_in: Some(
                        info.logged_in || (!with_usage && account.config_dir.is_none()),
                    ),
                    plugin: match account.config_dir {
                        None => None,
                        Some(_) => Some(info.plugin),
                    },
                    five_hour: info.usage.five_hour.as_ref().map(|window| window.percent),
                    seven_day: info.usage.seven_day.as_ref().map(|window| window.percent),
                    five_hour_resets_at: info
                        .usage
                        .five_hour
                        .as_ref()
                        .and_then(|window| window.resets_at.clone()),
                    seven_day_resets_at: info
                        .usage
                        .seven_day
                        .as_ref()
                        .and_then(|window| window.resets_at.clone()),
                    paused_until: info.paused_until,
                },
            );
        }
        out
    }

    /// Dienst je Account; ändert sich der Ordner (Account neu angelegt), entsteht ein neuer
    fn service(&self, account: &ClaudeAccount) -> Arc<ClaudeUsageService> {
        let files = self.options.paths.files(account.config_dir.as_deref());
        let mut services = self.services.lock().expect("Sperre der Account-Limits");
        if let Some(known) = services.get(&account.id) {
            if known.dir == files.config_dir {
                return known.service.clone();
            }
        }
        let read = self.credentials(
            files.keychain_service.clone(),
            files.credentials_file.clone(),
        );
        let fetch = self
            .options
            .fetch
            .clone()
            .unwrap_or_else(crate::usage::http_fetch);
        let mut service = ClaudeUsageService::with_credentials(
            read,
            fetch,
            self.options.clock.clone(),
            DEFAULT_TTL_MS,
        );
        // Erneuert nur die Datei; unter macOS liegt die Anmeldung im Schlüsselbund
        if let (Some(tokens), false) = (&self.options.tokens, self.options.platform == "darwin") {
            let (tokens, id) = (tokens.clone(), account.id.clone());
            service = service.with_refresh(file_refresh(
                files.credentials_file.clone(),
                self.options
                    .token_post
                    .clone()
                    .unwrap_or_else(crate::oauth_refresh::http_post),
                self.options.clock.clone(),
                Arc::new(move || tokens.has(&id)),
            ));
        }
        let service = Arc::new(service);
        services.insert(
            account.id.clone(),
            PerAccount {
                dir: files.config_dir,
                service: service.clone(),
            },
        );
        service
    }

    /// Inhalt der Anmeldung: unter macOS zuerst der Schlüsselbund, sonst bzw. danach die Datei
    fn credentials(&self, keychain_service: String, file: PathBuf) -> ReadCredentials {
        let darwin = self.options.platform == "darwin";
        let keychain = self.options.keychain.clone();
        Arc::new(move || {
            let keychain = keychain.clone();
            let keychain_service = keychain_service.clone();
            let file = file.clone();
            Box::pin(async move {
                if darwin {
                    if let Some(token) = keychain(keychain_service).await {
                        return Some(token);
                    }
                }
                tokio::task::spawn_blocking(move || std::fs::read_to_string(file).ok())
                    .await
                    .ok()
                    .flatten()
            })
        })
    }
}

/// E-Mail aus `oauthAccount` in `.claude.json` oder `None`
fn identity(file: &std::path::Path) -> Option<String> {
    let text = std::fs::read_to_string(file).ok()?;
    let json: serde_json::Value = serde_json::from_str(&text).ok()?;
    json.get("oauthAccount")?
        .get("emailAddress")?
        .as_str()
        .filter(|email| !email.is_empty())
        .map(str::to_string)
}

/// `security find-generic-password -s <dienst> -w`; nicht gefunden oder Fehler → `None`
pub fn system_keychain() -> Keychain {
    Arc::new(|service: String| {
        Box::pin(async move {
            let call = tokio::process::Command::new(SECURITY)
                .args(["find-generic-password", "-s", &service, "-w"])
                .output();
            let output = tokio::time::timeout(SECURITY_TIMEOUT, call)
                .await
                .ok()?
                .ok()?;
            if !output.status.success() {
                return None;
            }
            let token = String::from_utf8_lossy(&output.stdout).trim().to_string();
            (!token.is_empty()).then_some(token)
        })
    })
}

/// Ein Schlüsselbund, der nie etwas findet (andere Plattformen, Tests)
pub fn no_keychain() -> Keychain {
    Arc::new(|_service: String| Box::pin(async { None }))
}
