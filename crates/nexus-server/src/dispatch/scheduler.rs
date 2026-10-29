//! Takt des Dispatchers im Server (Standard: jede Minute). Portiert aus
//! `packages/server/src/dispatch/DispatchScheduler.ts`.
//!
//! Überlappende Durchläufe verhindert [`DispatchService::tick`] selbst; der Takt wartet
//! ohnehin das Ende eines Durchlaufs ab, bevor er den nächsten anstößt.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tokio::sync::{watch, Mutex};
use tokio::task::JoinHandle;
use tokio::time::{interval_at, Instant, MissedTickBehavior};

use crate::dispatch::service::DispatchService;

/// So lange wartet [`DispatchScheduler::stop`] höchstens auf einen laufenden Durchlauf
pub const STOP_WAIT: Duration = Duration::from_secs(30);

/// Schreibt eine Zeile in die Protokolldatei des Servers
pub type LogLine = Arc<dyn Fn(&str) + Send + Sync>;

/// Was der Takt anstößt; im Betrieb der [`DispatchService`], in Tests ein Doppel
#[async_trait]
pub trait DispatchTick: Send + Sync + 'static {
    /// Ein Durchlauf; ein Fehler landet im Protokoll und hält den Takt nicht an
    async fn tick(&self) -> nexus_core::Result<()>;
}

#[async_trait]
impl DispatchTick for DispatchService {
    async fn tick(&self) -> nexus_core::Result<()> {
        // Fehler eines Durchlaufs stehen in seinem Bericht, nicht im Ergebnis
        DispatchService::tick(self).await;
        Ok(())
    }
}

/// Stößt den Dispatcher im Takt an.
pub struct DispatchScheduler {
    service: Arc<dyn DispatchTick>,
    interval: Duration,
    log: LogLine,
    stop_wait: Duration,
    /// Die laufende Takt-Aufgabe und der Schalter, mit dem `stop` sie beendet
    task: Mutex<Option<(JoinHandle<()>, watch::Sender<bool>)>>,
}

impl std::fmt::Debug for DispatchScheduler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DispatchScheduler")
            .field("interval", &self.interval)
            .field("stop_wait", &self.stop_wait)
            .finish_non_exhaustive()
    }
}

impl DispatchScheduler {
    /// Takt auf diesem Service; `interval` von null heißt aus. Ohne eigenes Protokoll
    /// verschwinden Fehler eines Durchlaufs, und `stop` wartet [`STOP_WAIT`]
    pub fn new(service: Arc<dyn DispatchTick>, interval: Duration) -> Self {
        Self {
            service,
            interval,
            log: Arc::new(|_| {}),
            stop_wait: STOP_WAIT,
            task: Mutex::new(None),
        }
    }

    /// Protokoll für Fehler eines Durchlaufs und die Meldung beim Aufgeben in `stop`
    pub fn with_log(mut self, log: LogLine) -> Self {
        self.log = log;
        self
    }

    /// Wartezeit von `stop` auf einen laufenden Durchlauf
    pub fn with_stop_wait(mut self, wait: Duration) -> Self {
        self.stop_wait = wait;
        self
    }

    /// Startet den Takt; ein Intervall von null und ein schon laufender Takt tun nichts. Der
    /// erste Durchlauf kommt nach einem Intervall, nicht sofort
    pub async fn start(&self) {
        if self.interval.is_zero() {
            return;
        }
        let mut slot = self.task.lock().await;
        if slot.is_some() {
            return;
        }
        let (stop, mut stopped) = watch::channel(false);
        let service = self.service.clone();
        let log = self.log.clone();
        let every = self.interval;
        let handle = tokio::spawn(async move {
            let mut timer = interval_at(Instant::now() + every, every);
            // Ein langer Durchlauf schiebt den nächsten Takt, statt Versäumtes nachzuholen
            timer.set_missed_tick_behavior(MissedTickBehavior::Delay);
            loop {
                tokio::select! {
                    _ = stopped.changed() => return,
                    _ = timer.tick() => {}
                }
                if let Err(error) = service.tick().await {
                    log(&format!("Dispatcher: {error}"));
                }
                // Nach einem Durchlauf, der über `stop` hinausgeht, hier aufhören
                if *stopped.borrow_and_update() {
                    return;
                }
            }
        });
        *slot = Some((handle, stop));
    }

    /// Hält den Takt an und wartet auf einen laufenden Durchlauf (höchstens die eingestellte
    /// Wartezeit), damit er nicht mitten im Schließen des Servers auf eine schon abgebaute
    /// Board-Laufzeit trifft
    pub async fn stop(&self) {
        let Some((handle, stop)) = self.task.lock().await.take() else {
            return;
        };
        let _ = stop.send(true);
        if tokio::time::timeout(self.stop_wait, handle).await.is_err() {
            (self.log)(&format!(
                "Dispatcher: laufender Durchlauf nach {} s nicht beendet – Server schließt trotzdem",
                self.stop_wait.as_secs()
            ));
        }
    }
}
