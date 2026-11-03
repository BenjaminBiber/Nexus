//! Claude-Accounts aus Sicht des Dispatchers: Wahl je Lauf, Ordner für `CLAUDE_CONFIG_DIR`,
//! Rücksetzzeit eines vollen Accounts. Portiert aus
//! `packages/server/src/dispatch/AccountPool.ts`.

use std::collections::BTreeMap;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use nexus_core::accounts::{
    AccountPaths, AccountStore, AccountTokenStore, ClaudeAccount, STANDARD_ACCOUNT_ID,
};
use nexus_core::clock::Clock;
use nexus_core::dispatch::{
    estimate_reserve, AccountChoice, AccountChoiceInput, AccountChooser, AccountReserve,
    AccountState, DispatchConfig, DispatchStateStore, ReserveFor, RunKind, RunRecord, RunSettings,
    UsageHistory, UsageSample, UsageSnapshot,
};
use nexus_core::HarnessHome;

use crate::accounts::{AccountLimits, AccountList, AccountUsageService};

/// Was der Lauf über seine Accounts mitbringt
#[derive(Debug, Clone, Default)]
pub struct RunAccounts {
    /// Erlaubte Accounts des Laufs; leer = alle
    pub allowed_accounts: Option<Vec<String>>,
    /// Herkunft der erlaubten Accounts („Profil schwer“)
    pub accounts_from: Option<String>,
    /// Profil des Laufs; leer (Seitenläufe) = keine Reserve
    pub profile: String,
    /// Modell des Laufs, für die Schätzung der Reserve
    pub model: Option<String>,
    /// Effort des Laufs, für die Schätzung der Reserve
    pub effort: Option<String>,
}

impl From<&RunSettings> for RunAccounts {
    fn from(run: &RunSettings) -> Self {
        Self {
            allowed_accounts: run.allowed_accounts.clone(),
            accounts_from: run.accounts_from.clone(),
            profile: run.profile.clone(),
            model: run.model.clone(),
            effort: run.effort.clone(),
        }
    }
}

/// Die Accounts, unter denen der Dispatcher wählt.
#[async_trait]
pub trait AccountPool: Send + Sync {
    /// Wählt den Account eines Laufs; `running` sind die laufenden Läufe, deren ausstehende
    /// Reserve auf ihrem Account mitzählt
    async fn pick(
        &self,
        run: &RunAccounts,
        cfg: &DispatchConfig,
        now: DateTime<Utc>,
        pauses: &BTreeMap<String, String>,
        running: &[RunRecord],
    ) -> AccountChoice;

    /// Wert für `CLAUDE_CONFIG_DIR`; `None` = Standard (Umgebung des Servers)
    async fn config_dir(&self, id: &str) -> Option<String>;

    /// Langzeit-Token des Accounts für `CLAUDE_CODE_OAUTH_TOKEN` oder `None`
    async fn oauth_token(&self, _id: &str) -> Option<String> {
        None
    }

    /// Aktive Accounts
    async fn enabled(&self) -> Vec<ClaudeAccount>;

    /// Mehr als ein aktiver Account? Nur dann verteilt der Dispatcher Läufe nach Limits
    async fn multi(&self) -> bool;

    /// Nach einem Nutzungslimit: ab wann der Account wieder frei ist (frisch bei Anthropic
    /// gefragt) oder `None`
    async fn resets_at(&self, id: &str, limit_percent: i64) -> Option<String>;

    /// Aktuelle Auslastung eines Accounts (frisch bei Anthropic gefragt); `None` = unbekannt
    async fn usage_snapshot(&self, _id: &str) -> Option<UsageSnapshot> {
        None
    }
}

/// Accounts aus `~/.nexus/accounts.json`, Limits über den [`AccountUsageService`].
pub struct StoredAccountPool {
    store: Arc<dyn AccountList>,
    usage: Arc<dyn AccountLimits>,
    paths: AccountPaths,
    /// Langzeit-Tokens der Läufe; `None` = Läufe nutzen die Anmeldung im Ordner
    tokens: Option<AccountTokenStore>,
    /// Verbrauchsproben vergangener Läufe; `None` = nur der feste Puffer
    history: Option<UsageHistory>,
}

impl std::fmt::Debug for StoredAccountPool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StoredAccountPool").finish_non_exhaustive()
    }
}

impl StoredAccountPool {
    /// Pool über diesem Speicher und Limit-Dienst
    pub fn new(
        store: Arc<dyn AccountList>,
        usage: Arc<dyn AccountLimits>,
        paths: AccountPaths,
    ) -> Self {
        Self {
            store,
            usage,
            paths,
            tokens: None,
            history: None,
        }
    }

    /// Läufe bekommen das Langzeit-Token ihres Accounts aus diesem Speicher
    pub fn with_tokens(mut self, tokens: AccountTokenStore) -> Self {
        self.tokens = Some(tokens);
        self
    }

    /// Die erwartete Reserve eines Laufs wird aus diesen Proben geschätzt
    pub fn with_history(mut self, history: UsageHistory) -> Self {
        self.history = Some(history);
        self
    }

    /// Pool über dem Benutzerordner mit den Pfaden des Rechners
    pub fn create(home: HarnessHome, usage: Arc<AccountUsageService>) -> Self {
        let paths = AccountPaths::from_process_env();
        Self::new(
            Arc::new(AccountStore::with_paths(home.clone(), paths.clone())),
            usage,
            paths,
        )
        .with_tokens(AccountTokenStore::new(home.clone()))
        .with_history(UsageHistory::new(home))
    }

    /// Die jüngsten Proben; ein Lesefehler ergibt keine (dann gilt der Puffer)
    fn samples(&self) -> Vec<UsageSample> {
        self.history
            .as_ref()
            .and_then(|history| history.recent(SAMPLE_LIMIT).ok())
            .unwrap_or_default()
    }
}

/// So viele Proben liest die Schätzung höchstens
const SAMPLE_LIMIT: usize = 500;

/// Noch ausstehender Verbrauch der laufenden Umsetzungsläufe je Account: ihre geschätzte
/// Reserve minus das, was ihr Account seit dem Start des Laufs schon mehr ausgelastet ist
/// (je Fenster nur, wenn beide Werte bekannt sind, mindestens 0)
fn pending_reserve(
    running: &[RunRecord],
    samples: &[UsageSample],
    fallback: AccountReserve,
    states: &BTreeMap<String, AccountState>,
) -> BTreeMap<String, AccountReserve> {
    let mut pending: BTreeMap<String, AccountReserve> = BTreeMap::new();
    for record in running {
        if record.kind != RunKind::Work {
            continue;
        }
        let (Some(id), Some(start)) = (&record.run.account, &record.run.usage_at_start) else {
            continue;
        };
        let estimate = estimate_reserve(
            samples,
            &ReserveFor {
                profile: &record.run.profile,
                model: record.run.model.as_deref(),
                effort: record.run.effort.as_deref(),
            },
            fallback,
        );
        let now = states.get(id);
        let used = |now: Option<i64>, start: Option<i64>| match (now, start) {
            (Some(now), Some(start)) => (now - start).max(0),
            _ => 0,
        };
        let rest = AccountReserve {
            five_hour: (estimate.reserve.five_hour
                - used(now.and_then(|s| s.five_hour), start.five_hour))
            .max(0),
            week: (estimate.reserve.week - used(now.and_then(|s| s.seven_day), start.seven_day))
                .max(0),
        };
        let total = pending.entry(id.clone()).or_default();
        *total = total.plus(rest);
    }
    pending
}

#[async_trait]
impl AccountPool for StoredAccountPool {
    async fn pick(
        &self,
        run: &RunAccounts,
        cfg: &DispatchConfig,
        now: DateTime<Utc>,
        pauses: &BTreeMap<String, String>,
        running: &[RunRecord],
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
        // Seitenläufe (ohne Profil) rechnen keine Reserve und keine laufenden Läufe ein
        let (reserve, source, pending) = if run.profile.is_empty() {
            (AccountReserve::default(), None, BTreeMap::new())
        } else {
            let samples = self.samples();
            let fallback = AccountReserve {
                five_hour: cfg.account_reserve_five_hour,
                week: cfg.account_reserve_week,
            };
            let own = estimate_reserve(
                &samples,
                &ReserveFor {
                    profile: &run.profile,
                    model: run.model.as_deref(),
                    effort: run.effort.as_deref(),
                },
                fallback,
            );
            (
                own.reserve,
                Some(own.source),
                pending_reserve(running, &samples, fallback, &states),
            )
        };
        let choice = AccountChooser::new().choose(&AccountChoiceInput {
            allowed_accounts: run.allowed_accounts.clone(),
            accounts_from: run.accounts_from.clone(),
            accounts: &accounts,
            states,
            strategy: cfg.account_strategy,
            limit_percent: cfg.account_limit_percent,
            now,
            reserve,
            pending,
            reset_grace_minutes: cfg.account_reset_grace_minutes,
        });
        match (choice, source) {
            (
                AccountChoice::Picked {
                    account,
                    why,
                    snapshot,
                },
                Some(source),
            ) => AccountChoice::Picked {
                account,
                why: format!(
                    "{why}; Reserve 5 h {} %, Woche {} % ({source})",
                    reserve.five_hour, reserve.week
                ),
                snapshot,
            },
            (choice, _) => choice,
        }
    }

    async fn config_dir(&self, id: &str) -> Option<String> {
        if id == STANDARD_ACCOUNT_ID {
            return None;
        }
        let account = self
            .store
            .list()
            .into_iter()
            .find(|account| account.id == id)?;
        let dir = account.config_dir?;
        Some(self.paths.expand(&dir).to_string_lossy().into_owned())
    }

    async fn oauth_token(&self, id: &str) -> Option<String> {
        self.tokens.as_ref()?.get(id)
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
        let account = self
            .store
            .list()
            .into_iter()
            .find(|account| account.id == id)?;
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

    async fn usage_snapshot(&self, id: &str) -> Option<UsageSnapshot> {
        let account = self
            .store
            .list()
            .into_iter()
            .find(|account| account.id == id)?;
        let usage = self.usage.usage(&account, true).await;
        if !usage.available {
            return None;
        }
        Some(UsageSnapshot {
            five_hour: usage.five_hour.as_ref().map(|window| window.percent),
            seven_day: usage.seven_day.as_ref().map(|window| window.percent),
            five_hour_resets_at: usage
                .five_hour
                .as_ref()
                .and_then(|window| window.resets_at.clone()),
            seven_day_resets_at: usage
                .seven_day
                .as_ref()
                .and_then(|window| window.resets_at.clone()),
        })
    }
}

/// Anmeldung eines Claude-Aufrufs außerhalb des Dispatchers; leer = Standard (Umgebung des
/// Servers)
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SideRunAccount {
    /// Wert für `CLAUDE_CONFIG_DIR`
    pub config_dir: Option<String>,
    /// Langzeit-Token für `CLAUDE_CODE_OAUTH_TOKEN`
    pub oauth_token: Option<String>,
}

/// Account für Claude-Aufrufe außerhalb des Dispatchers (Doku überarbeiten, Entwürfe,
/// Analyse): jeder aktive Account, meiste freie Kapazität; ist keiner frei, der Standard
/// (Umgebung des Servers)
pub async fn side_run_account(
    home: &HarnessHome,
    cfg: &DispatchConfig,
    now: DateTime<Utc>,
) -> SideRunAccount {
    let usage = Arc::new(AccountUsageService::new(
        Arc::new(AccountStore::new(home.clone())),
        Clock::system(),
    ));
    let pool = StoredAccountPool::create(home.clone(), usage);
    side_run_account_with(home, cfg, now, &pool).await
}

/// Wie [`side_run_account`], mit einem vorhandenen Pool (der Server teilt seinen)
pub async fn side_run_account_with(
    home: &HarnessHome,
    cfg: &DispatchConfig,
    now: DateTime<Utc>,
    pool: &dyn AccountPool,
) -> SideRunAccount {
    let Ok(state) = DispatchStateStore::new(home.clone()).read() else {
        return SideRunAccount::default();
    };
    let pauses: BTreeMap<String, String> = state.account_pauses.into_iter().collect();
    let choice = pool
        .pick(&RunAccounts::default(), cfg, now, &pauses, &[])
        .await;
    match choice {
        AccountChoice::Picked { account, .. } => SideRunAccount {
            config_dir: pool.config_dir(&account.id).await,
            oauth_token: pool.oauth_token(&account.id).await,
        },
        AccountChoice::Waiting { .. } => SideRunAccount::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::usage::ClaudeUsageDto;
    use nexus_core::dispatch::DispatchConfigNormalizer;
    use serde_json::json;

    const NOW: &str = "2026-11-03T10:00:00Z";

    /// Feste Zustände je Account statt einer Abfrage bei Anthropic
    struct FixedLimits(BTreeMap<String, AccountState>);

    #[async_trait]
    impl AccountLimits for FixedLimits {
        async fn states(
            &self,
            accounts: &[ClaudeAccount],
            _pauses: &BTreeMap<String, String>,
            _with_usage: bool,
        ) -> BTreeMap<String, AccountState> {
            accounts
                .iter()
                .filter_map(|a| self.0.get(&a.id).map(|s| (a.id.clone(), s.clone())))
                .collect()
        }

        async fn usage(&self, _account: &ClaudeAccount, _fresh: bool) -> ClaudeUsageDto {
            ClaudeUsageDto::unavailable("disabled", None)
        }
    }

    fn now() -> DateTime<Utc> {
        NOW.parse().unwrap()
    }

    /// `m` Minuten nach `NOW` als ISO-Text
    fn in_minutes(m: i64) -> String {
        nexus_core::clock::iso(now() + chrono::Duration::minutes(m))
    }

    fn account(id: &str) -> ClaudeAccount {
        ClaudeAccount {
            id: id.to_string(),
            name: id.to_string(),
            config_dir: Some(format!("/a/{id}")),
            enabled: true,
            fallback: false,
            limit_five_hour: None,
            limit_week: None,
        }
    }

    /// Pool über diesen Accounts; `a` hat diese 5-Stunden-Auslastung und Rücksetzung
    fn pool_of(
        accounts: Vec<ClaudeAccount>,
        five_hour: i64,
        resets_at: String,
    ) -> StoredAccountPool {
        let state = AccountState {
            logged_in: Some(true),
            plugin: Some(true),
            five_hour: Some(five_hour),
            five_hour_resets_at: Some(resets_at),
            ..AccountState::default()
        };
        StoredAccountPool::new(
            Arc::new(accounts),
            Arc::new(FixedLimits(BTreeMap::from([("a".to_string(), state)]))),
            AccountPaths::new(Default::default(), "/home"),
        )
    }

    /// Pool über dem Account `a`
    fn pool(five_hour: i64, resets_at: String) -> StoredAccountPool {
        pool_of(vec![account("a")], five_hour, resets_at)
    }

    /// Einstellung: Grenze 90, Puffer 10 und 2, Kulanz 30 Minuten
    fn cfg() -> DispatchConfig {
        DispatchConfigNormalizer::new()
            .normalize(&json!({
                "accountLimitPercent": 90,
                "accountReserveFiveHour": 10,
                "accountReserveWeek": 2,
                "accountResetGraceMinutes": 30,
            }))
            .unwrap()
    }

    fn run(profile: &str) -> RunAccounts {
        RunAccounts {
            profile: profile.to_string(),
            model: Some("sonnet".to_string()),
            effort: Some("high".to_string()),
            ..RunAccounts::default()
        }
    }

    /// Laufender Umsetzungslauf auf `a`, der bei `start` Prozent im 5-Stunden-Fenster begann
    fn running_on_a(start: i64) -> RunRecord {
        let settings = RunSettings {
            profile: "klein".to_string(),
            why: "x".to_string(),
            model: Some("sonnet".to_string()),
            effort: Some("high".to_string()),
            max_turns: 10,
            timeout_minutes: 10,
            reason: None,
            config_hash: None,
            rule_index: None,
            escalated_from: None,
            allowed_accounts: None,
            accounts_from: None,
            account: Some("a".to_string()),
            account_why: None,
            usage_at_start: Some(UsageSnapshot {
                five_hour: Some(start),
                ..UsageSnapshot::default()
            }),
        };
        RunRecord {
            id: "r1".to_string(),
            kind: RunKind::Work,
            ticket_id: "T-1".to_string(),
            board_id: 1,
            title: "T".to_string(),
            agent: "junior-T-1".to_string(),
            cwd: "/wt".to_string(),
            branch: "ticket/x".to_string(),
            target: None,
            run: settings,
            attempt: None,
            command: "claude".to_string(),
            args: Vec::new(),
            prompt: "p".to_string(),
            log_file: "/log".to_string(),
            started_at: NOW.to_string(),
            pid: None,
            setup: None,
            setup_timeout_minutes: None,
            question_id: None,
            research_before: None,
            planned_before: None,
            repo_root: None,
        }
    }

    async fn pick(
        pool: &StoredAccountPool,
        run: &RunAccounts,
        running: &[RunRecord],
    ) -> AccountChoice {
        pool.pick(run, &cfg(), now(), &BTreeMap::new(), running)
            .await
    }

    #[tokio::test]
    async fn knapp_wartet_mit_grund_wenn_die_ruecksetzung_noch_fern_ist() {
        let choice = pick(&pool(88, in_minutes(120)), &run("klein"), &[]).await;
        let AccountChoice::Waiting { reason, .. } = choice else {
            panic!("es sollte warten: {choice:?}");
        };
        assert!(reason.contains("knapp"), "{reason}");
    }

    #[tokio::test]
    async fn knapp_startet_kurz_vor_der_ruecksetzung_des_5_stunden_fensters() {
        let choice = pick(&pool(88, in_minutes(10)), &run("klein"), &[]).await;
        assert!(matches!(choice, AccountChoice::Picked { .. }), "{choice:?}");
    }

    #[tokio::test]
    async fn ausstehende_reserve_laufender_laeufe_zaehlt_mit() {
        // Auslastung 70, der laufende Lauf begann bei 68 (Puffer 10, davon 2 verbraucht → 8
        // offen), der neue braucht 10: 70 + 10 + 8 = 88 > 85
        let capped = ClaudeAccount {
            limit_five_hour: Some(85),
            ..account("a")
        };
        let pool = pool_of(vec![capped], 70, in_minutes(120));
        let choice = pick(&pool, &run("klein"), &[running_on_a(68)]).await;
        let AccountChoice::Waiting { reason, .. } = choice else {
            panic!("es sollte warten: {choice:?}");
        };
        assert!(reason.contains("70 % + 18 % erwartet"), "{reason}");

        // Ohne den laufenden Lauf passt es: 70 + 10 <= 85
        let choice = pick(&pool, &run("klein"), &[]).await;
        assert!(matches!(choice, AccountChoice::Picked { .. }), "{choice:?}");
    }

    #[tokio::test]
    async fn ohne_proben_nennt_die_begruendung_den_puffer() {
        let choice = pick(&pool(20, in_minutes(120)), &run("klein"), &[]).await;
        let AccountChoice::Picked { why, .. } = choice else {
            panic!("es sollte starten: {choice:?}");
        };
        assert!(
            why.contains("Reserve 5 h 10 %, Woche 2 % (Puffer)"),
            "{why}"
        );
    }

    #[tokio::test]
    async fn proben_ersetzen_den_puffer_und_stehen_in_der_begruendung() {
        let dir = tempfile::tempdir().unwrap();
        let history = UsageHistory::new(HarnessHome::at(dir.path()));
        for _ in 0..5 {
            history
                .append(&UsageSample {
                    account: "a".to_string(),
                    profile: "klein".to_string(),
                    model: Some("sonnet".to_string()),
                    effort: Some("high".to_string()),
                    started_at: "2026-11-03T08:00:00.000Z".to_string(),
                    ended_at: "2026-11-03T08:30:00.000Z".to_string(),
                    before: UsageSnapshot {
                        five_hour: Some(10),
                        seven_day: Some(10),
                        five_hour_resets_at: Some("2026-11-03T13:00:00.000Z".to_string()),
                        seven_day_resets_at: Some("2026-11-09T13:00:00.000Z".to_string()),
                    },
                    after: UsageSnapshot {
                        five_hour: Some(13),
                        seven_day: Some(11),
                        ..UsageSnapshot::default()
                    },
                })
                .unwrap();
        }
        let pool = pool(20, in_minutes(120)).with_history(history);
        let choice = pick(&pool, &run("klein"), &[]).await;
        let AccountChoice::Picked { why, .. } = choice else {
            panic!("es sollte starten: {choice:?}");
        };
        assert!(
            why.contains("Reserve 5 h 3 %, Woche 1 % (aus 5 Läufen profil:klein/sonnet/high)"),
            "{why}"
        );
    }

    #[tokio::test]
    async fn seitenlaeufe_ohne_profil_rechnen_keine_reserve() {
        // 88 % + Puffer 10 wäre knapp; ein Seitenlauf (leeres Profil) startet wie bisher
        let choice = pick(&pool(88, in_minutes(120)), &RunAccounts::default(), &[]).await;
        let AccountChoice::Picked { why, .. } = choice else {
            panic!("es sollte starten: {choice:?}");
        };
        assert!(!why.contains("Reserve"), "{why}");
    }
}
