//! Beobachtet `dispatch/runs` und stößt den Schattenbetrieb an, sobald der Node-Dispatcher
//! dort eine Lauf- oder Ergebnisdatei anlegt.
//!
//! Der Beobachter ist die schnelle Seite: Rust liest das Ticket umso näher am Start, je
//! früher es den Lauf bemerkt. Verlässlich ist dagegen der Takt, der denselben Ordner noch
//! einmal abgleicht – geht ein Ereignis verloren oder gibt es den Ordner beim Start noch
//! nicht, fehlt deshalb nichts. Aus demselben Grund legt der Beobachter den Ordner nicht an:
//! im Schattenbetrieb schreibt Rust nur unter `dispatch/shadow/`.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::{mpsc, watch, Mutex};
use tokio::task::JoinHandle;

use crate::dispatch::scheduler::LogLine;
use crate::dispatch::shadow::service::ShadowService;

/// So lange werden Ereignisse gesammelt, bevor verglichen wird (eine Lauf-Datei entsteht in
/// mehreren Schreibvorgängen)
pub const DEBOUNCE: Duration = Duration::from_millis(250);

/// So oft wird nachgesehen, ob es den beobachteten Ordner inzwischen gibt
pub const RETRY: Duration = Duration::from_secs(10);

/// Beobachtet den Lauf-Ordner und stößt den Vergleich an.
pub struct ShadowWatcher {
    task: Mutex<Option<(JoinHandle<()>, watch::Sender<bool>)>>,
}

impl std::fmt::Debug for ShadowWatcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ShadowWatcher").finish_non_exhaustive()
    }
}

impl ShadowWatcher {
    /// Beobachter, der noch nicht läuft
    pub fn new() -> Self {
        Self {
            task: Mutex::new(None),
        }
    }

    /// Beginnt zu beobachten; ein zweiter Aufruf tut nichts
    pub async fn start(&self, service: Arc<ShadowService>, dir: PathBuf, log: LogLine) {
        let mut slot = self.task.lock().await;
        if slot.is_some() {
            return;
        }
        let (stop, stopped) = watch::channel(false);
        let handle = tokio::spawn(run(service, dir, log, stopped));
        *slot = Some((handle, stop));
    }

    /// Hält den Beobachter an
    pub async fn stop(&self) {
        let Some((handle, stop)) = self.task.lock().await.take() else {
            return;
        };
        let _ = stop.send(true);
        let _ = handle.await;
    }
}

impl Default for ShadowWatcher {
    fn default() -> Self {
        Self::new()
    }
}

/// Die Schleife des Beobachters: anmelden, sobald es den Ordner gibt, und auf Ereignisse
/// warten
async fn run(
    service: Arc<ShadowService>,
    dir: PathBuf,
    log: LogLine,
    mut stopped: watch::Receiver<bool>,
) {
    let (sender, mut events) = mpsc::unbounded_channel();
    let mut watcher: Option<RecommendedWatcher> = None;
    loop {
        if watcher.is_none() && dir.exists() {
            match attach(&dir, sender.clone()) {
                Ok(started) => watcher = Some(started),
                Err(error) => log(&format!(
                    "Schattenbetrieb: Beobachter auf {} nicht gestartet ({error}) – der Takt \
gleicht trotzdem ab",
                    dir.to_string_lossy()
                )),
            }
        }
        tokio::select! {
            _ = stopped.changed() => return,
            _ = tokio::time::sleep(RETRY) => continue,
            got = events.recv() => {
                if got.is_none() {
                    return;
                }
            }
        }
        // Alles, was in der Sammelzeit noch kommt, gehört zum selben Vergleich
        tokio::time::sleep(DEBOUNCE).await;
        while events.try_recv().is_ok() {}
        if *stopped.borrow_and_update() {
            return;
        }
        service.scan_runs().await;
    }
}

/// Meldet den Beobachter auf dem Ordner an; nur Ereignisse, die eine Datei anlegen oder
/// ändern, sind von Belang
fn attach(dir: &Path, sender: mpsc::UnboundedSender<()>) -> notify::Result<RecommendedWatcher> {
    let mut watcher = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
        let Ok(event) = event else {
            return;
        };
        if event.kind.is_create() || event.kind.is_modify() {
            let _ = sender.send(());
        }
    })?;
    watcher.watch(dir, RecursiveMode::NonRecursive)?;
    Ok(watcher)
}
