//! Der Takt des Dispatchers; portiert aus
//! `packages/server/tests/unit/dispatch/DispatchScheduler.test.ts`.
//!
//! Die Zeit läuft in diesen Tests angehalten (`start_paused`): kein Test wartet auf eine echte
//! Minute.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use nexus_core::{HarnessError, Result};
use nexus_server::dispatch::scheduler::{DispatchScheduler, DispatchTick, LogLine, TickGate};
use tokio::sync::Mutex as AsyncMutex;

/// Zählt die Durchläufe; je nach Bauart endet jeder sofort, nie oder erst auf Zuruf
struct CountingTick {
    calls: Arc<AtomicUsize>,
    /// Sperre, die ein Durchlauf nimmt und erst hält, wenn sie von außen gehalten wird
    gate: Arc<AsyncMutex<()>>,
    /// Endet nie
    hangs: bool,
    /// Scheitert mit dieser Meldung
    fails: Option<String>,
}

impl CountingTick {
    /// Ein Durchlauf, der sofort fertig ist
    fn quick() -> (Arc<Self>, Arc<AtomicUsize>) {
        let calls = Arc::new(AtomicUsize::new(0));
        let tick = Arc::new(Self {
            calls: calls.clone(),
            gate: Arc::new(AsyncMutex::new(())),
            hangs: false,
            fails: None,
        });
        (tick, calls)
    }

    /// Ein Durchlauf, der endet, sobald die Sperre frei ist
    fn gated() -> (Arc<Self>, Arc<AtomicUsize>, Arc<AsyncMutex<()>>) {
        let (tick, calls) = Self::quick();
        let gate = tick.gate.clone();
        (tick, calls, gate)
    }

    /// Ein Durchlauf, der nie endet
    fn hanging() -> Arc<Self> {
        let calls = Arc::new(AtomicUsize::new(0));
        Arc::new(Self {
            calls,
            gate: Arc::new(AsyncMutex::new(())),
            hangs: true,
            fails: None,
        })
    }

    /// Ein Durchlauf, der mit dieser Meldung scheitert
    fn failing(message: &str) -> Arc<Self> {
        let calls = Arc::new(AtomicUsize::new(0));
        Arc::new(Self {
            calls,
            gate: Arc::new(AsyncMutex::new(())),
            hangs: false,
            fails: Some(message.to_string()),
        })
    }
}

#[async_trait]
impl DispatchTick for CountingTick {
    async fn tick(&self) -> Result<()> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if self.hangs {
            std::future::pending::<()>().await;
        }
        let _open = self.gate.lock().await;
        match &self.fails {
            Some(message) => Err(HarnessError::new("tick-failed", message.clone()).into()),
            None => Ok(()),
        }
    }
}

/// Ein Protokoll, das die Zeilen sammelt
fn collecting() -> (LogLine, Arc<Mutex<Vec<String>>>) {
    let lines = Arc::new(Mutex::new(Vec::new()));
    let sink = lines.clone();
    let log: LogLine = Arc::new(move |line: &str| {
        sink.lock().expect("Protokoll").push(line.to_string());
    });
    (log, lines)
}

#[tokio::test(start_paused = true)]
async fn tickt_im_takt_und_hoert_nach_stop_auf_null_heisst_aus() {
    let (tick, calls) = CountingTick::quick();
    let scheduler = DispatchScheduler::new(tick, Duration::from_secs(1));
    scheduler.start().await;
    tokio::time::sleep(Duration::from_millis(3_100)).await;
    assert_eq!(calls.load(Ordering::SeqCst), 3);

    scheduler.stop().await;
    tokio::time::sleep(Duration::from_secs(3)).await;
    assert_eq!(
        calls.load(Ordering::SeqCst),
        3,
        "nach stop tickt nichts mehr"
    );

    // Ein Intervall von null schaltet den Takt aus
    let (off, never) = CountingTick::quick();
    DispatchScheduler::new(off, Duration::ZERO).start().await;
    tokio::time::sleep(Duration::from_secs(3)).await;
    assert_eq!(never.load(Ordering::SeqCst), 0);
}

#[tokio::test(start_paused = true)]
async fn stop_wartet_auf_einen_laufenden_durchlauf() {
    let (tick, calls, gate) = CountingTick::gated();
    let held = gate.lock().await;
    let scheduler = Arc::new(DispatchScheduler::new(tick, Duration::from_secs(1)));
    scheduler.start().await;
    tokio::time::sleep(Duration::from_millis(1_100)).await;
    assert_eq!(calls.load(Ordering::SeqCst), 1);

    let stopping = tokio::spawn({
        let scheduler = scheduler.clone();
        async move { scheduler.stop().await }
    });
    tokio::time::sleep(Duration::from_secs(5)).await;
    assert!(!stopping.is_finished(), "stop wartet auf den Durchlauf");

    drop(held);
    stopping.await.expect("stop");
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[tokio::test(start_paused = true)]
async fn endet_der_durchlauf_nie_gibt_stop_nach_der_wartezeit_auf() {
    let (log, lines) = collecting();
    let scheduler = Arc::new(
        DispatchScheduler::new(CountingTick::hanging(), Duration::from_secs(1))
            .with_log(log)
            .with_stop_wait(Duration::from_secs(30)),
    );
    scheduler.start().await;
    tokio::time::sleep(Duration::from_millis(1_100)).await;

    let giving = tokio::spawn({
        let scheduler = scheduler.clone();
        async move { scheduler.stop().await }
    });
    tokio::time::sleep(Duration::from_secs(29)).await;
    assert!(
        !giving.is_finished(),
        "vor der Wartezeit gibt stop nicht auf"
    );
    tokio::time::sleep(Duration::from_secs(2)).await;
    giving.await.expect("stop");
    assert_eq!(
        lines.lock().expect("Protokoll").as_slice(),
        ["Dispatcher: laufender Durchlauf nach 30 s nicht beendet – Server schließt trotzdem"]
    );
}

#[tokio::test(start_paused = true)]
async fn ohne_laufenden_durchlauf_kehrt_stop_sofort_zurueck() {
    let (tick, _calls) = CountingTick::quick();
    let scheduler = DispatchScheduler::new(tick, Duration::from_secs(1));
    // Ohne start gibt es nichts anzuhalten
    scheduler.stop().await;
    scheduler.start().await;
    scheduler.stop().await;
    // Ein zweites stop ist ebenso harmlos
    scheduler.stop().await;
}

#[tokio::test(start_paused = true)]
async fn protokolliert_den_fehler_eines_durchlaufs_statt_abzustuerzen() {
    let (log, lines) = collecting();
    let scheduler = DispatchScheduler::new(CountingTick::failing("kaputt"), Duration::from_secs(1))
        .with_log(log);
    scheduler.start().await;
    tokio::time::sleep(Duration::from_millis(2_100)).await;
    scheduler.stop().await;
    let lines = lines.lock().expect("Protokoll").clone();
    assert_eq!(
        lines,
        vec![
            "Dispatcher: kaputt".to_string(),
            "Dispatcher: kaputt".to_string()
        ],
        "der Takt läuft nach einem Fehler weiter"
    );
}

#[tokio::test(start_paused = true)]
async fn ohne_freigabe_ruht_der_takt_und_meldet_jeden_wechsel_einmal() {
    let (tick, calls) = CountingTick::quick();
    let (log, lines) = collecting();
    let allowed = Arc::new(AtomicBool::new(false));
    let gate: TickGate = {
        let allowed = allowed.clone();
        Arc::new(move || allowed.load(Ordering::SeqCst))
    };
    let scheduler = DispatchScheduler::new(tick, Duration::from_secs(1)).with_log(log);
    scheduler.start_with(gate).await;

    tokio::time::sleep(Duration::from_millis(2_100)).await;
    assert_eq!(
        calls.load(Ordering::SeqCst),
        0,
        "ohne Freigabe kein Durchlauf"
    );

    allowed.store(true, Ordering::SeqCst);
    tokio::time::sleep(Duration::from_secs(2)).await;
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    scheduler.stop().await;

    assert_eq!(
        lines.lock().expect("Protokoll").as_slice(),
        [
            "Dispatcher: Takt ruht – ein anderer Server führt den Dispatcher",
            "Dispatcher: Takt läuft wieder"
        ]
    );
}

#[tokio::test(start_paused = true)]
async fn ein_zweites_start_legt_keinen_zweiten_takt_an() {
    let (tick, calls) = CountingTick::quick();
    let scheduler = DispatchScheduler::new(tick, Duration::from_secs(1));
    scheduler.start().await;
    scheduler.start().await;
    tokio::time::sleep(Duration::from_millis(2_100)).await;
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    scheduler.stop().await;
}
