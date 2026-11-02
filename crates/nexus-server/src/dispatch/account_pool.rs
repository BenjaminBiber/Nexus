//! Claude-Accounts aus Sicht des Dispatchers: Wahl je Lauf, Ordner für `CLAUDE_CONFIG_DIR`,
//! Rücksetzzeit eines vollen Accounts. Portiert aus
//! `packages/server/src/dispatch/AccountPool.ts`.

use std::collections::BTreeMap;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use nexus_core::accounts::{AccountPaths, AccountStore, ClaudeAccount, STANDARD_ACCOUNT_ID};
use nexus_core::clock::Clock;
use nexus_core::dispatch::{
    AccountChoice, AccountChoiceInput, AccountChooser, DispatchConfig, DispatchStateStore,
    RunSettings,
};
use nexus_core::HarnessHome;

use crate::accounts::AccountUsageService;

/// Was der Lauf über seine Accounts mitbringt
#[derive(Debug, Clone, Default)]
pub struct RunAccounts {
    /// Erlaubte Accounts des Laufs; leer = alle
    pub allowed_accounts: Option<Vec<String>>,
    /// Herkunft der erlaubten Accounts („Profil schwer“)
    pub accounts_from: Option<String>,
}

impl From<&RunSettings> for RunAccounts {
    fn from(run: &RunSettings) -> Self {
        Self {
            allowed_accounts: run.allowed_accounts.clone(),
            accounts_from: run.accounts_from.clone(),
        }
    }
}

/// Die Accounts, unter denen der Dispatcher wählt.
#[async_trait]
pub trait AccountPool: Send + Sync {
    /// Wählt den Account eines Laufs
    async fn pick(
        &self,
        run: &RunAccounts,
        cfg: &DispatchConfig,
        now: DateTime<Utc>,
        pauses: &BTreeMap<String, String>,
    ) -> AccountChoice;

    /// Wert für `CLAUDE_CONFIG_DIR`; `None` = Standard (Umgebung des Servers)
    async fn config_dir(&self, id: &str) -> Option<String>;

    /// Aktive Accounts
    async fn enabled(&self) -> Vec<ClaudeAccount>;

    /// Mehr als ein aktiver Account? Nur dann verteilt der Dispatcher Läufe nach Limits
    async fn multi(&self) -> bool;

    /// Nach einem Nutzungslimit: ab wann der Account wieder frei ist (frisch bei Anthropic
    /// gefragt) oder `None`
    async fn resets_at(&self, id: &str, limit_percent: i64) -> Option<String>;
}

/// Accounts aus `~/.nexus/accounts.json`, Limits über den [`AccountUsageService`].
pub struct StoredAccountPool {
    store: AccountStore,
    usage: Arc<AccountUsageService>,
    paths: AccountPaths,
}

impl std::fmt::Debug for StoredAccountPool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StoredAccountPool").finish_non_exhaustive()
    }
}

impl StoredAccountPool {
    /// Pool über diesem Speicher und Limit-Dienst
    pub fn new(store: AccountStore, usage: Arc<AccountUsageService>, paths: AccountPaths) -> Self {
        Self {
            store,
            usage,
            paths,
        }
    }

    /// Pool über dem Benutzerordner mit den Pfaden des Rechners
    pub fn create(home: HarnessHome, usage: Arc<AccountUsageService>) -> Self {
        Self::new(
            AccountStore::new(home),
            usage,
            AccountPaths::from_process_env(),
        )
    }
}

#[async_trait]
impl AccountPool for StoredAccountPool {
    async fn pick(
        &self,
        run: &RunAccounts,
        cfg: &DispatchConfig,
        now: DateTime<Utc>,
        pauses: &BTreeMap<String, String>,
    ) -> AccountChoice {
        let accounts = self.store.list();
        // Limits fragt er ab, sobald es etwas zu verteilen gibt oder ein Account eine eigene
        // Obergrenze hat
        let enabled: Vec<&ClaudeAccount> =
            accounts.iter().filter(|account| account.enabled).collect();
        let multi = enabled.len() > 1
            || enabled
                .iter()
                .any(|account| account.limit_five_hour.is_some() || account.limit_week.is_some());
        let ids: Vec<String> = match run
            .allowed_accounts
            .as_deref()
            .filter(|ids| !ids.is_empty())
        {
            Some(ids) => ids.to_vec(),
            None => accounts.iter().map(|account| account.id.clone()).collect(),
        };
        let asked: Vec<ClaudeAccount> = accounts
            .iter()
            .filter(|account| ids.contains(&account.id))
            .cloned()
            .collect();
        let states = self.usage.states(&asked, pauses, multi).await;
        AccountChooser::new().choose(&AccountChoiceInput {
            allowed_accounts: run.allowed_accounts.clone(),
            accounts_from: run.accounts_from.clone(),
            accounts: &accounts,
            states,
            strategy: cfg.account_strategy,
            limit_percent: cfg.account_limit_percent,
            now,
        })
    }

    async fn config_dir(&self, id: &str) -> Option<String> {
        if id == STANDARD_ACCOUNT_ID {
            return None;
        }
        let account = self.store.find(id)?;
        let dir = account.config_dir?;
        Some(self.paths.expand(&dir).to_string_lossy().into_owned())
    }

    async fn enabled(&self) -> Vec<ClaudeAccount> {
        self.store
            .list()
            .into_iter()
            .filter(|account| account.enabled)
            .collect()
    }

    async fn multi(&self) -> bool {
        self.enabled().await.len() > 1
    }

    // Volle Fenster: auf oder über der Grenze des Accounts (eigene vor der Vorgabe); frei
    // erst, wenn jedes zurückgesetzt ist. Ist keins voll (das Limit kam aus einem Fenster, das
    // der Endpunkt nicht meldet) oder fehlt eine Rücksetzzeit, bleibt es unbekannt
    async fn resets_at(&self, id: &str, limit_percent: i64) -> Option<String> {
        let account = self.store.find(id)?;
        let usage = self.usage.usage(&account, true).await;
        let windows = [
            (
                usage.five_hour.as_ref(),
                account.limit_five_hour.unwrap_or(limit_percent),
            ),
            (
                usage.seven_day.as_ref(),
                account.limit_week.unwrap_or(limit_percent),
            ),
        ];
        let full: Vec<Option<String>> = windows
            .iter()
            .filter_map(|(window, limit)| {
                window
                    .filter(|window| window.percent >= *limit)
                    .map(|window| window.resets_at.clone())
            })
            .collect();
        if full.is_empty() || full.iter().any(Option::is_none) {
            return None;
        }
        let mut times: Vec<String> = full.into_iter().flatten().collect();
        times.sort();
        times.pop()
    }
}

/// Account für Claude-Aufrufe außerhalb des Dispatchers (Doku überarbeiten, Entwürfe,
/// Analyse): jeder aktive Account, meiste freie Kapazität; ist keiner frei, der Standard
/// (Umgebung des Servers). Liefert den Wert für `CLAUDE_CONFIG_DIR` oder `None`
pub async fn side_run_config_dir(
    home: &HarnessHome,
    cfg: &DispatchConfig,
    now: DateTime<Utc>,
) -> Option<String> {
    let usage = Arc::new(AccountUsageService::new(
        Arc::new(AccountStore::new(home.clone())),
        Clock::system(),
    ));
    let pool = StoredAccountPool::create(home.clone(), usage);
    side_run_config_dir_with(home, cfg, now, &pool).await
}

/// Wie [`side_run_config_dir`], mit einem vorhandenen Pool (der Server teilt seinen)
pub async fn side_run_config_dir_with(
    home: &HarnessHome,
    cfg: &DispatchConfig,
    now: DateTime<Utc>,
    pool: &dyn AccountPool,
) -> Option<String> {
    let pauses: BTreeMap<String, String> = DispatchStateStore::new(home.clone())
        .read()
        .ok()?
        .account_pauses
        .into_iter()
        .collect();
    let choice = pool.pick(&RunAccounts::default(), cfg, now, &pauses).await;
    match choice {
        AccountChoice::Picked { account, .. } => pool.config_dir(&account.id).await,
        AccountChoice::Waiting { .. } => None,
    }
}
