//! Verbrauchsproben nach beendeten Umsetzungsläufen.
//!
//! Der Pool bekommt eine Attrappe der Limits: so steht die Auslastung am Ende des Laufs fest,
//! ohne Anthropic zu fragen.

mod common;
mod dispatch_support;

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use nexus_core::accounts::{AccountPaths, AccountStore, ClaudeAccount, STANDARD_ACCOUNT_ID};
use nexus_core::board::basics::TicketStatus;
use nexus_core::board::ticket_service::{MoveOptions, TicketPatch};
use nexus_core::dispatch::{AccountState, RunKind, RunRecord, UsageHistory, UsageSnapshot};
use nexus_core::HarnessHome;
use nexus_server::accounts::AccountLimits;
use nexus_server::dispatch::account_pool::{AccountPool, StoredAccountPool};
use nexus_server::dispatch::run_recorder::{HandleOutcome, HandleResult, RunRecorder};
use nexus_server::dispatch::ticket_starter::StartOutcome;
use nexus_server::usage::{ClaudeUsageDto, UsageWindowDto};
use serde_json::json;

use dispatch_support::{DispatchFixture, OK_OUTPUT};

/// Limits mit fester Auslastung: 5 Stunden `five_hour`, Woche `seven_day`
#[derive(Debug)]
struct FakeUsage {
    five_hour: Mutex<i64>,
    seven_day: i64,
}

#[async_trait]
impl AccountLimits for FakeUsage {
    async fn states(
        &self,
        accounts: &[ClaudeAccount],
        _pauses: &BTreeMap<String, String>,
        _with_usage: bool,
    ) -> BTreeMap<String, AccountState> {
        let five_hour = *self.five_hour.lock().unwrap();
        accounts
            .iter()
            .map(|account| {
                (
                    account.id.clone(),
                    AccountState {
                        logged_in: Some(true),
                        plugin: None,
                        five_hour: Some(five_hour),
                        seven_day: Some(self.seven_day),
                        five_hour_resets_at: Some("2026-11-03T15:00:00.000Z".into()),
                        seven_day_resets_at: None,
                        paused_until: None,
                    },
                )
            })
            .collect()
    }

    async fn usage(&self, _account: &ClaudeAccount, _fresh: bool) -> ClaudeUsageDto {
        ClaudeUsageDto {
            available: true,
            reason: None,
            plan: Some("max".into()),
            five_hour: Some(UsageWindowDto {
                percent: *self.five_hour.lock().unwrap(),
                resets_at: Some("2026-11-03T15:00:00.000Z".into()),
            }),
            seven_day: Some(UsageWindowDto {
                percent: self.seven_day,
                resets_at: None,
            }),
            fetched_at: None,
        }
    }
}

/// Fixture mit dem Standard-Account im Pool; Auslastung beim Start 10 %/20 %
async fn setup() -> (DispatchFixture, Arc<FakeUsage>) {
    let usage = Arc::new(FakeUsage {
        five_hour: Mutex::new(10),
        seven_day: 20,
    });
    let limits = usage.clone();
    let f = DispatchFixture::create_with_accounts(json!({}), |home: &HarnessHome| {
        Some(Arc::new(StoredAccountPool::new(
            Arc::new(AccountStore::new(home.clone())),
            limits,
            AccountPaths::new(BTreeMap::new().into_iter().collect(), home.dir()),
        )) as Arc<dyn AccountPool>)
    })
    .await;
    (f, usage)
}

/// Startet ein Ticket, meldet es erledigt und lässt den Lauf enden
async fn finish(f: &DispatchFixture, title: &str) -> String {
    let id = f.ready(title, TicketPatch::default()).await;
    match f.start(&id).await.expect("Start") {
        StartOutcome::Started(_) => {}
        other => panic!("Start erwartet, nicht {other:?}"),
    }
    f.backend
        .services
        .tickets
        .move_to(
            &id,
            TicketStatus::Done,
            &f.agent(&id),
            &MoveOptions::default(),
        )
        .await
        .expect("erledigt melden");
    f.advance(4);
    f.end(&id, OK_OUTPUT, Some(0));
    id
}

#[tokio::test]
async fn nach_einem_umsetzungslauf_steht_genau_eine_probe_in_der_datei() {
    let (f, usage) = setup().await;
    let id = finish(&f, "Probe").await;
    let record = f.run(&id);
    assert!(record.run.usage_at_start.is_some());
    *usage.five_hour.lock().unwrap() = 17;
    f.evaluate(&id).await.expect("Auswertung");

    let samples = UsageHistory::new(f.home.clone())
        .recent(10)
        .expect("Proben");
    assert_eq!(samples.len(), 1);
    let sample = &samples[0];
    assert_eq!(sample.account, STANDARD_ACCOUNT_ID);
    assert_eq!(sample.profile, record.run.profile);
    assert_eq!(sample.model, record.run.model);
    assert_eq!(sample.effort, record.run.effort);
    assert_eq!(sample.started_at, record.started_at);
    assert_eq!(sample.before.five_hour, Some(10));
    assert_eq!(sample.before.seven_day, Some(20));
    assert_eq!(
        sample.after,
        UsageSnapshot {
            five_hour: Some(17),
            seven_day: Some(20),
            five_hour_resets_at: Some("2026-11-03T15:00:00.000Z".into()),
            seven_day_resets_at: None,
        }
    );
}

#[tokio::test]
async fn ohne_account_verwaltung_entsteht_keine_probe() {
    let f = DispatchFixture::create(json!({})).await;
    let id = finish(&f, "Ohne Accounts").await;
    assert_eq!(f.run(&id).run.usage_at_start, None);
    f.evaluate(&id).await.expect("Auswertung");
    let samples = UsageHistory::new(f.home.clone())
        .recent(10)
        .expect("Proben");
    assert!(samples.is_empty());
}

#[tokio::test]
async fn plan_und_merge_laeufe_schreiben_keine_probe() {
    let (f, _usage) = setup().await;
    let id = finish(&f, "Andere Art").await;
    let record = f.run(&id);
    let end = f.result(&record.id);
    let result = HandleResult::new(HandleOutcome::Done);
    for kind in [RunKind::Plan, RunKind::Merge] {
        let other = RunRecord {
            kind,
            ..record.clone()
        };
        RunRecorder::new(&f.ctx).keep(&other, &end, &result).await;
    }
    let samples = UsageHistory::new(f.home.clone())
        .recent(10)
        .expect("Proben");
    assert!(samples.is_empty());
}

#[tokio::test]
async fn ein_schreibfehler_bricht_die_auswertung_nicht_ab() {
    let (f, _usage) = setup().await;
    // Ein Ordner an Stelle der Datei: Anhängen schlägt fehl
    std::fs::create_dir_all(f.home.account_usage_file()).expect("Ordner anlegen");
    let id = finish(&f, "Schreibfehler").await;
    f.evaluate(&id)
        .await
        .expect("Auswertung trotz Schreibfehler");
    assert_eq!(f.ticket(&id).await.status, TicketStatus::Done);
}
