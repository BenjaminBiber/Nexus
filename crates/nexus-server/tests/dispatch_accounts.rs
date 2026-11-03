//! Der Dispatcher mit mehreren Claude-Accounts; portiert aus
//! `packages/server/tests/unit/dispatch/DispatchAccounts.test.ts` des Referenzbranches.
//!
//! Der Pool bekommt eine Attrappe der Limits: so steht die Auslastung je Account fest, ohne
//! Anthropic zu fragen.

mod common;
mod dispatch_support;

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use nexus_core::accounts::{
    AccountPatch, AccountPaths, AccountStore, ClaudeAccount, NewAccount, STANDARD_ACCOUNT_ID,
};
use nexus_core::board::basics::TicketStatus;
use nexus_core::board::ticket_service::TicketPatch;
use nexus_core::dispatch::{hhmm, AccountState};
use nexus_core::HarnessHome;
use nexus_server::accounts::AccountLimits;
use nexus_server::dispatch::account_pool::{AccountPool, StoredAccountPool};
use nexus_server::usage::{ClaudeUsageDto, UsageWindowDto};
use serde_json::{json, Value};

use dispatch_support::DispatchFixture;

/// Ausgabe eines Laufs, den das Nutzungslimit beendet hat
const LIMIT: &str = "Claude usage limit reached";

/// Limits je Account, wie der Test sie setzt; die Rücksetzzeit gilt für ein volles Fenster
#[derive(Debug, Default)]
struct FakeUsage {
    load: Mutex<BTreeMap<String, i64>>,
    resets_at: Mutex<Option<String>>,
    /// Mit welchem `with_usage` die Wahl gefragt hat
    with_usage: Mutex<Vec<bool>>,
    /// Mit welchem `fresh` die Rücksetzzeit gefragt hat
    fresh: Mutex<Vec<bool>>,
    /// Feste Antwort von [`AccountLimits::usage`]; ohne sie zählt `load`
    reply: Mutex<Option<ClaudeUsageDto>>,
}

impl FakeUsage {
    fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    fn set_load(&self, id: &str, percent: i64) {
        self.load.lock().unwrap().insert(id.to_string(), percent);
    }

    fn set_resets_at(&self, at: Option<&str>) {
        *self.resets_at.lock().unwrap() = at.map(str::to_string);
    }
}

#[async_trait]
impl AccountLimits for FakeUsage {
    async fn states(
        &self,
        accounts: &[ClaudeAccount],
        pauses: &BTreeMap<String, String>,
        with_usage: bool,
    ) -> BTreeMap<String, AccountState> {
        self.with_usage.lock().unwrap().push(with_usage);
        let load = self.load.lock().unwrap().clone();
        let resets_at = self.resets_at.lock().unwrap().clone();
        accounts
            .iter()
            .map(|account| {
                (
                    account.id.clone(),
                    AccountState {
                        logged_in: Some(true),
                        plugin: Some(true),
                        five_hour: with_usage.then(|| load.get(&account.id).copied().unwrap_or(0)),
                        seven_day: with_usage.then_some(0),
                        five_hour_resets_at: resets_at.clone(),
                        seven_day_resets_at: None,
                        paused_until: pauses.get(&account.id).cloned(),
                    },
                )
            })
            .collect()
    }

    async fn usage(&self, account: &ClaudeAccount, fresh: bool) -> ClaudeUsageDto {
        self.fresh.lock().unwrap().push(fresh);
        if let Some(reply) = self.reply.lock().unwrap().clone() {
            return reply;
        }
        let percent = self
            .load
            .lock()
            .unwrap()
            .get(&account.id)
            .copied()
            .unwrap_or(0);
        ClaudeUsageDto {
            available: true,
            reason: None,
            plan: Some("max".into()),
            five_hour: Some(UsageWindowDto {
                percent,
                resets_at: self.resets_at.lock().unwrap().clone(),
            }),
            seven_day: None,
            fetched_at: None,
        }
    }
}

/// Fixture mit den Accounts `pro-1`, `pro-2` und `max` (Profil schwer nur `max`); der
/// Standard-Account ist abgeschaltet. Liefert dazu die Ordner der Accounts
async fn setup(raw: Value) -> (DispatchFixture, Arc<FakeUsage>, BTreeMap<String, String>) {
    let usage = FakeUsage::new();
    let limits = usage.clone();
    let profiles = json!({
        "klein": { "model": "sonnet", "accounts": ["pro-1", "pro-2"] },
        "standard": { "model": "opus", "accounts": ["pro-1", "pro-2"] },
        "schwer": { "model": "opus", "effort": "max", "accounts": ["max"] },
    });
    let mut patch = json!({ "profiles": profiles });
    if let (Some(patch), Some(raw)) = (patch.as_object_mut(), raw.as_object()) {
        for (key, value) in raw {
            patch.insert(key.clone(), value.clone());
        }
    }
    let f = DispatchFixture::create_with_accounts(patch, |home: &HarnessHome| {
        Some(Arc::new(StoredAccountPool::new(
            Arc::new(AccountStore::new(home.clone())),
            limits,
            AccountPaths::new(BTreeMap::new().into_iter().collect(), home.dir()),
        )) as Arc<dyn AccountPool>)
    })
    .await;
    let store = AccountStore::new(f.home.clone());
    store
        .update(
            STANDARD_ACCOUNT_ID,
            &AccountPatch {
                enabled: Some(false),
                ..AccountPatch::default()
            },
        )
        .expect("Standard abschalten");
    let mut dirs = BTreeMap::new();
    for name in ["pro-1", "pro-2", "max"] {
        let dir = f.dir().join("profiles").join(name);
        std::fs::create_dir_all(&dir).expect("Ordner des Accounts");
        store
            .create(&NewAccount {
                name: name.to_string(),
                config_dir: Some(dir.to_string_lossy().into_owned()),
            })
            .expect("Account anlegen");
        dirs.insert(name.to_string(), dir.to_string_lossy().into_owned());
    }
    (f, usage, dirs)
}

/// `CLAUDE_CONFIG_DIR` des zuletzt gestarteten Laufs
fn config_dir_of(f: &DispatchFixture) -> Option<String> {
    f.launcher.last_env().get("CLAUDE_CONFIG_DIR").cloned()
}

#[tokio::test]
async fn startet_mit_dem_account_des_profils_und_nennt_ihn_im_start_kommentar() {
    let (f, _usage, dirs) = setup(json!({})).await;
    let id = f
        .ready(
            "Schwer",
            TicketPatch {
                labels: Some(vec!["auto".into(), "profil:schwer".into()]),
                ..TicketPatch::default()
            },
        )
        .await;

    f.tick().await;

    assert_eq!(config_dir_of(&f).as_deref(), Some(dirs["max"].as_str()));
    let run = f.run(&id);
    assert_eq!(run.run.account.as_deref(), Some("max"));
    assert_eq!(run.run.allowed_accounts, Some(vec!["max".to_string()]));
    let start = f
        .comments(&id)
        .await
        .into_iter()
        .find(|comment| comment.starts_with("[dispatch] Start"))
        .expect("Start-Kommentar");
    assert!(
        start.contains(
            "\nAccount: max (Profil schwer → max; Reserve 5 h 10 %, Woche 2 % (Puffer))."
        ),
        "{start}"
    );
}

/// Ticket auf dem Profil `schwer` (nur Account `max`)
async fn ready_schwer(f: &DispatchFixture, title: &str) -> String {
    f.ready(
        title,
        TicketPatch {
            labels: Some(vec!["auto".into(), "profil:schwer".into()]),
            ..TicketPatch::default()
        },
    )
    .await
}

/// Warum ein Ticket wartet, wie die Zeile im Zustand es nennt
async fn waiting_reason(f: &DispatchFixture, id: &str) -> String {
    let status = f.service.status(Some(f.board.id)).await.expect("Zustand");
    status
        .tickets
        .iter()
        .find(|row| row.id == id)
        .expect("Zeile des Tickets")
        .reason
        .clone()
}

#[tokio::test]
async fn ein_knapper_account_laesst_das_ticket_warten_bis_die_ruecksetzung_nah_ist() {
    let (f, usage, _dirs) = setup(json!({})).await;
    usage.set_load("max", 88);
    usage.set_resets_at(Some(&f.now_iso_in(120)));
    let id = ready_schwer(&f, "Knapp").await;

    let report = f.tick().await;

    // 88 % + Puffer 10 liegt über der Grenze 90, die Rücksetzung ist noch zwei Stunden fern
    assert!(report.started.is_empty());
    let reason = waiting_reason(&f, &id).await;
    assert!(
        reason.contains("knapp (5 h 88 % + 10 % erwartet, Grenze 90 %)"),
        "{reason}"
    );

    // Setzt sich das Fenster in 10 Minuten zurück, zählt die Reserve nicht
    usage.set_resets_at(Some(&f.now_iso_in(10)));
    let report = f.tick().await;

    assert_eq!(report.started, vec![id.clone()]);
    let start = f
        .comments(&id)
        .await
        .into_iter()
        .find(|comment| comment.starts_with("[dispatch] Start"))
        .expect("Start-Kommentar");
    assert!(
        start.contains("Reserve 5 h 10 %, Woche 2 % (Puffer)"),
        "{start}"
    );
}

#[tokio::test]
async fn die_ausstehende_reserve_eines_laufenden_laufs_zaehlt_fuer_das_naechste_ticket() {
    let (f, usage, _dirs) = setup(json!({ "wip": 2 })).await;
    usage.set_load("max", 75);
    usage.set_resets_at(Some(&f.now_iso_in(120)));
    let first = ready_schwer(&f, "Erstes").await;
    let second = ready_schwer(&f, "Zweites").await;

    let report = f.tick().await;

    // 75 % + 10 % passen; das zweite Ticket muss die 10 % des laufenden mitrechnen: 75 + 20 > 90
    assert_eq!(report.started.len(), 1, "{:?}", report.started);
    let waiting = if report.started[0] == first {
        &second
    } else {
        &first
    };
    let reason = waiting_reason(&f, waiting).await;
    assert!(reason.contains("5 h 75 % + 20 % erwartet"), "{reason}");
}

#[tokio::test]
async fn waehlt_unter_mehreren_accounts_den_mit_der_meisten_freien_kapazitaet() {
    let (f, usage, dirs) = setup(json!({})).await;
    usage.set_load("pro-1", 70);
    usage.set_load("pro-2", 20);
    let id = f.ready("Verteilt", TicketPatch::default()).await;

    f.tick().await;

    assert_eq!(config_dir_of(&f).as_deref(), Some(dirs["pro-2"].as_str()));
    assert_eq!(
        f.run(&id).run.account_why.as_deref(),
        Some(
            "Profil standard → pro-2, meiste freie Kapazität: pro-1 (70 %), pro-2 (20 %); \
Reserve 5 h 10 %, Woche 2 % (Puffer)"
        )
    );
}

#[tokio::test]
async fn ein_nutzungslimit_pausiert_nur_den_account_und_das_ticket_startet_auf_dem_anderen() {
    let (f, usage, dirs) = setup(json!({})).await;
    let resets_at = f.now_iso_in(90);
    usage.set_resets_at(Some(&resets_at));
    usage.set_load("pro-2", 50);
    let id = f.ready("Rotation", TicketPatch::default()).await;

    f.tick().await;
    assert_eq!(config_dir_of(&f).as_deref(), Some(dirs["pro-1"].as_str()));

    usage.set_load("pro-1", 100);
    f.end(&id, LIMIT, Some(1));
    let report = f.tick().await;

    assert_eq!(
        report
            .evaluated
            .iter()
            .map(|run| (run.ticket_id.clone(), run.outcome.clone()))
            .collect::<Vec<_>>(),
        vec![(id.clone(), "limit".to_string())]
    );
    // Nicht global pausiert: derselbe Durchlauf startet das Ticket gleich auf pro-2
    assert_eq!(report.idle, None);
    assert_eq!(report.started, vec![id.clone()]);
    assert_eq!(config_dir_of(&f).as_deref(), Some(dirs["pro-2"].as_str()));
    let state = f.service.stores.state.read().expect("Zustand");
    assert_eq!(state.paused_until, None);
    assert_eq!(
        state.account_pauses.get("pro-1").map(String::as_str),
        Some(resets_at.as_str())
    );
    let pause = f
        .comments(&id)
        .await
        .into_iter()
        .find(|comment| comment.starts_with("[dispatch] Pause"))
        .expect("Pause-Kommentar");
    assert_eq!(
        pause,
        format!(
            "[dispatch] Pause: Nutzungslimit von Account pro-1 erreicht, Ticket zurück nach \
Geplant. Account pro-1 pausiert bis {}. Das Ticket startet beim nächsten Durchlauf auf einem \
anderen freien Account. Zählt nicht als Versuch.",
            hhmm(&resets_at)
        )
    );
    assert!(!f
        .comments(&id)
        .await
        .iter()
        .any(|comment| comment.starts_with("[dispatch] Abbruch")));
    let status = f.service.status(None).await.expect("Zustand");
    assert_eq!(
        status.account_pauses,
        BTreeMap::from([("pro-1".to_string(), resets_at)])
    );
}

#[tokio::test]
async fn ist_kein_erlaubter_account_frei_wartet_das_ticket_ungeclaimt() {
    let (f, usage, _dirs) = setup(json!({})).await;
    usage.set_load("max", 95);
    let id = f
        .ready(
            "Wartet",
            TicketPatch {
                labels: Some(vec!["auto".into(), "profil:schwer".into()]),
                ..TicketPatch::default()
            },
        )
        .await;

    let report = f.tick().await;

    assert!(report.started.is_empty());
    assert!(f.launcher.launched().is_empty());
    let ticket = f.ticket(&id).await;
    assert_eq!(ticket.status, TicketStatus::Planned);
    assert_eq!(ticket.assignee, None);
    let status = f.service.status(Some(f.board.id)).await.expect("Zustand");
    let row = status
        .tickets
        .iter()
        .find(|row| row.id == id)
        .expect("Zeile des Tickets");
    assert_eq!(
        row.reason,
        "wartet – kein Account frei (max: voll (5 h 95 %))"
    );

    // Kennt Anthropic die Rücksetzung, nennt das Warten sie
    let resets_at = f.now_iso_in(45);
    usage.set_resets_at(Some(&resets_at));
    let bis = hhmm(&resets_at);
    let status = f.service.status(Some(f.board.id)).await.expect("Zustand");
    let row = status
        .tickets
        .iter()
        .find(|row| row.id == id)
        .expect("Zeile des Tickets");
    assert_eq!(
        row.reason,
        format!(
            "wartet – kein Account frei (max: voll (5 h 95 %) bis {bis}) – wieder frei ab {bis}"
        )
    );
}

#[tokio::test]
async fn sind_alle_aktiven_accounts_pausiert_pausiert_der_ganze_dispatcher() {
    let (f, _usage, _dirs) = setup(json!({
        "profiles": { "standard": { "model": "opus", "accounts": ["max"] } },
        "defaultProfile": "standard",
        "rules": [],
        "escalation": ["standard"],
    }))
    .await;
    let store = AccountStore::new(f.home.clone());
    for id in ["pro-1", "pro-2"] {
        store
            .update(
                id,
                &AccountPatch {
                    enabled: Some(false),
                    ..AccountPatch::default()
                },
            )
            .expect("Account abschalten");
    }
    let id = f.ready("Alle voll", TicketPatch::default()).await;

    f.tick().await;
    f.end(&id, LIMIT, Some(1));
    let report = f.tick().await;

    assert_eq!(
        report
            .evaluated
            .iter()
            .map(|run| run.outcome.clone())
            .collect::<Vec<_>>(),
        vec!["limit".to_string()]
    );
    let state = f.service.stores.state.read().expect("Zustand");
    assert_eq!(
        state.paused_until.as_deref(),
        state.account_pauses.get("max").map(String::as_str)
    );
}

#[tokio::test]
async fn pause_aufheben_gibt_einen_account_wieder_frei() {
    let (f, _usage, _dirs) = setup(json!({})).await;
    let until = f.now_iso_in(60);
    f.service
        .stores
        .state
        .update(|state| {
            state.account_pauses.insert("pro-1".into(), until);
        })
        .await
        .expect("Pause setzen");

    f.service.resume_account("pro-1").await.expect("Pause auf");

    assert!(f.service.account_pauses().expect("Pausen").is_empty());
}

#[tokio::test]
async fn fragt_die_limits_erst_ab_wenn_es_etwas_zu_verteilen_gibt() {
    let usage = FakeUsage::new();
    let cfg = dispatch_support::config(json!({}));
    let paths = AccountPaths::new(BTreeMap::new().into_iter().collect(), "/home/ich");
    let standard = nexus_core::accounts::standard_account();

    // Ein Account ohne eigene Obergrenze: keine Abfrage bei Anthropic
    let plain = StoredAccountPool::new(
        Arc::new(vec![standard.clone()]),
        usage.clone(),
        paths.clone(),
    );
    let choice = plain
        .pick(
            &Default::default(),
            &cfg,
            chrono::Utc::now(),
            &BTreeMap::new(),
            &[],
        )
        .await;
    assert!(
        matches!(&choice, nexus_core::dispatch::AccountChoice::Picked { account, .. } if account.id == "standard"),
        "{choice:?}"
    );

    // Mit eigener Obergrenze fragt der Pool nach – und der Account gilt als voll
    usage.set_load("standard", 60);
    let capped = StoredAccountPool::new(
        Arc::new(vec![ClaudeAccount {
            limit_five_hour: Some(50),
            ..standard
        }]),
        usage.clone(),
        paths,
    );
    let choice = capped
        .pick(
            &Default::default(),
            &cfg,
            chrono::Utc::now(),
            &BTreeMap::new(),
            &[],
        )
        .await;
    match choice {
        nexus_core::dispatch::AccountChoice::Waiting { reason, .. } => {
            assert!(reason.contains("Grenze 50 %"), "{reason}")
        }
        other => panic!("kein Warten: {other:?}"),
    }
    assert_eq!(*usage.with_usage.lock().unwrap(), vec![false, true]);
}

#[tokio::test]
async fn resets_at_fragt_frisch_und_rechnet_mit_den_eigenen_grenzen() {
    let usage = FakeUsage::new();
    let paths = AccountPaths::new(BTreeMap::new().into_iter().collect(), "/home/ich");
    let account = ClaudeAccount {
        id: "a".into(),
        name: "a".into(),
        config_dir: Some("/a/a".into()),
        enabled: true,
        fallback: false,
        limit_five_hour: None,
        limit_week: Some(80),
    };
    let pool = StoredAccountPool::new(Arc::new(vec![account]), usage.clone(), paths);
    let window = |percent: i64, at: Option<&str>| {
        Some(UsageWindowDto {
            percent,
            resets_at: at.map(str::to_string),
        })
    };
    let reply = |five, seven| ClaudeUsageDto {
        available: true,
        reason: None,
        plan: Some("max".into()),
        five_hour: five,
        seven_day: seven,
        fetched_at: None,
    };

    // Beide Fenster voll (die Woche nach der eigenen Grenze): frei erst nach dem letzten
    *usage.reply.lock().unwrap() = Some(reply(
        window(100, Some("2026-10-28T12:00:00Z")),
        window(85, Some("2026-10-30T08:00:00Z")),
    ));
    assert_eq!(
        pool.resets_at("a", 90).await.as_deref(),
        Some("2026-10-30T08:00:00Z")
    );

    // Nur das 5-Stunden-Fenster ist voll
    *usage.reply.lock().unwrap() = Some(reply(
        window(100, Some("2026-10-28T12:00:00Z")),
        window(50, Some("2026-10-30T08:00:00Z")),
    ));
    assert_eq!(
        pool.resets_at("a", 90).await.as_deref(),
        Some("2026-10-28T12:00:00Z")
    );

    // Kein volles Fenster: unbekannt
    *usage.reply.lock().unwrap() = Some(reply(
        window(40, Some("2026-10-28T12:00:00Z")),
        window(50, Some("2026-10-30T08:00:00Z")),
    ));
    assert_eq!(pool.resets_at("a", 90).await, None);

    // Voll, aber ohne Rücksetzzeit: ebenfalls unbekannt
    *usage.reply.lock().unwrap() = Some(reply(window(100, None), None));
    assert_eq!(pool.resets_at("a", 90).await, None);

    assert_eq!(*usage.fresh.lock().unwrap(), vec![true, true, true, true]);
}
