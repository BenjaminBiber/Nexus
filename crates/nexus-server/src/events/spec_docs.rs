//! Beobachtet die Superpowers-Dokumente der Projekte mit offenem Ereignisstrom; portiert aus
//! `packages/server/src/events/SpecDocWatcher.ts`.
//!
//! Abgefragt wird eine Signatur über Pfad, Änderungszeit und Größe aller Dokumente
//! ([`nexus_core::docs::SuperpowersDocScanner::signature`]) – keine Dateiereignisse:
//! Netzlaufwerke und WSL-Pfade melden sie unzuverlässig.

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use nexus_core::Result;
use tokio::sync::mpsc::UnboundedSender;
use tokio::task::JoinHandle;

use crate::events::hub::LiveEvent;

/// Die Abfrage der Signatur eines Projekts (sie liest das Dateisystem und dauert deshalb)
pub type SignatureFuture = Pin<Box<dyn Future<Output = Result<String>> + Send>>;

/// Woher die Signatur eines Projekts kommt
pub type SignatureOf = Arc<dyn Fn(String) -> SignatureFuture + Send + Sync>;

/// Ein angemeldeter Empfänger
struct Listener {
    id: u64,
    sender: UnboundedSender<LiveEvent>,
}

/// Ein beobachtetes Projekt mit seiner zuletzt gesehenen Signatur
#[derive(Default)]
struct Entry {
    listeners: Vec<Listener>,
    /// `None`, solange der Ausgangswert fehlt – bis dahin wird nichts gemeldet
    signature: Option<String>,
}

/// Fragt für jedes Projekt mit offenem Ereignisstrom regelmäßig die Signatur seiner
/// Superpowers-Dokumente ab und meldet Änderungen als `docs-changed`.
pub struct SpecDocWatcher {
    projects: Mutex<HashMap<String, Entry>>,
    signature_of: SignatureOf,
    interval: Duration,
    /// Nie zwei Durchläufe gleichzeitig
    polling: AtomicBool,
    timer: Mutex<Option<JoinHandle<()>>>,
    next_id: AtomicU64,
}

impl SpecDocWatcher {
    /// Beobachter über einer Signatur-Abfrage
    pub fn new(signature_of: SignatureOf, interval_ms: u64) -> Arc<Self> {
        Arc::new(Self {
            projects: Mutex::new(HashMap::new()),
            signature_of,
            interval: Duration::from_millis(interval_ms.max(1)),
            polling: AtomicBool::new(false),
            timer: Mutex::new(None),
            next_id: AtomicU64::new(0),
        })
    }

    /// Beobachtet ein Projekt; die Beobachtung endet, wenn das Ergebnis fällt
    pub fn watch(self: &Arc<Self>, pid: &str, sender: UnboundedSender<LiveEvent>) -> WatchedDocs {
        let listener = self.next_id.fetch_add(1, Ordering::Relaxed);
        let fresh = {
            let mut projects = self.projects.lock().expect("Sperre der Beobachtungen");
            let entry = projects.entry(pid.to_string()).or_default();
            let fresh = entry.listeners.is_empty() && entry.signature.is_none();
            entry.listeners.push(Listener {
                id: listener,
                sender,
            });
            fresh
        };
        // Ausgangswert ohne Meldung lesen; ein Fehler wartet auf den nächsten Durchlauf
        if fresh {
            let watcher = self.clone();
            let pid = pid.to_string();
            tokio::spawn(async move { watcher.prime(&pid).await });
        }
        self.ensure_timer();
        WatchedDocs {
            watcher: self.clone(),
            pid: pid.to_string(),
            listener,
        }
    }

    /// Ein Durchlauf: Signaturen abfragen und Änderungen melden (nie zwei gleichzeitig)
    pub async fn poll(&self) {
        if self.polling.swap(true, Ordering::SeqCst) {
            return;
        }
        let _running = Polling(&self.polling);
        for pid in self.ids() {
            // Ein Fehler überspringt dieses Projekt; der nächste Durchlauf versucht es wieder
            let Ok(signature) = (self.signature_of)(pid.clone()).await else {
                continue;
            };
            let mut projects = self.projects.lock().expect("Sperre der Beobachtungen");
            let Some(entry) = projects.get_mut(&pid) else {
                continue;
            };
            let changed = entry
                .signature
                .as_deref()
                .is_some_and(|seen| seen != signature);
            entry.signature = Some(signature);
            if changed {
                let event = LiveEvent::DocsChanged {
                    project_id: pid.clone(),
                };
                for listener in &entry.listeners {
                    let _ = listener.sender.send(event.clone());
                }
            }
        }
    }

    /// Beendet alle Beobachtungen (Ende des Servers)
    pub fn close(&self) {
        self.projects
            .lock()
            .expect("Sperre der Beobachtungen")
            .clear();
        self.stop_timer();
    }

    /// Zahl der Beobachter eines Projekts (Tests und der Aufräum-Nachweis)
    pub fn watchers(&self, pid: &str) -> usize {
        self.projects
            .lock()
            .expect("Sperre der Beobachtungen")
            .get(pid)
            .map(|entry| entry.listeners.len())
            .unwrap_or(0)
    }

    /// Die beobachteten Projekte in fester Reihenfolge
    fn ids(&self) -> Vec<String> {
        let mut ids: Vec<String> = self
            .projects
            .lock()
            .expect("Sperre der Beobachtungen")
            .keys()
            .cloned()
            .collect();
        ids.sort();
        ids
    }

    /// Ausgangswert beim ersten Beobachten, ohne Meldung
    async fn prime(&self, pid: &str) {
        let Ok(signature) = (self.signature_of)(pid.to_string()).await else {
            return;
        };
        let mut projects = self.projects.lock().expect("Sperre der Beobachtungen");
        if let Some(entry) = projects.get_mut(pid) {
            if entry.signature.is_none() {
                entry.signature = Some(signature);
            }
        }
    }

    /// Startet den Takt, falls er noch nicht läuft
    fn ensure_timer(self: &Arc<Self>) {
        let mut timer = self.timer.lock().expect("Sperre des Takts");
        if timer.is_some() {
            return;
        }
        let watcher = self.clone();
        let period = self.interval;
        *timer = Some(tokio::spawn(async move {
            // Der erste Schlag kommt erst nach einem Takt, nicht sofort
            let mut ticker = tokio::time::interval_at(tokio::time::Instant::now() + period, period);
            loop {
                ticker.tick().await;
                watcher.poll().await;
            }
        }));
    }

    /// Hält den Takt an
    fn stop_timer(&self) {
        if let Some(timer) = self.timer.lock().expect("Sperre des Takts").take() {
            timer.abort();
        }
    }

    /// Meldet einen Empfänger ab; die letzte Abmeldung eines Projekts nimmt die Beobachtung
    /// mit, die letzte Beobachtung den Takt
    fn unwatch(&self, pid: &str, listener: u64) {
        let empty = {
            let mut projects = self.projects.lock().expect("Sperre der Beobachtungen");
            if let Some(entry) = projects.get_mut(pid) {
                entry.listeners.retain(|other| other.id != listener);
                if entry.listeners.is_empty() {
                    projects.remove(pid);
                }
            }
            projects.is_empty()
        };
        if empty {
            self.stop_timer();
        }
    }
}

/// Setzt die Sperre gegen gleichzeitige Durchläufe auch dann zurück, wenn der Durchlauf
/// abgebrochen wird
struct Polling<'a>(&'a AtomicBool);

impl Drop for Polling<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

/// Eine laufende Beobachtung der Dokumente; sie endet, wenn sie fällt
pub struct WatchedDocs {
    watcher: Arc<SpecDocWatcher>,
    pid: String,
    listener: u64,
}

impl Drop for WatchedDocs {
    fn drop(&mut self) {
        self.watcher.unwatch(&self.pid, self.listener);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Signaturen aus einer Tabelle, die der Test ändern kann
    fn signatures(values: &[(&str, &str)]) -> (SignatureOf, Arc<Mutex<HashMap<String, String>>>) {
        let table: Arc<Mutex<HashMap<String, String>>> = Arc::new(Mutex::new(
            values
                .iter()
                .map(|(pid, value)| ((*pid).to_string(), (*value).to_string()))
                .collect(),
        ));
        let shared = table.clone();
        let signature_of: SignatureOf = Arc::new(move |pid| {
            let table = shared.clone();
            Box::pin(async move {
                let value = table.lock().expect("Sperre").get(&pid).cloned();
                value.ok_or_else(|| {
                    nexus_core::HarnessError::new("keine-signatur", "keine Signatur").into()
                })
            })
        });
        (signature_of, table)
    }

    /// Setzt eine Signatur der Tabelle
    fn set(table: &Arc<Mutex<HashMap<String, String>>>, pid: &str, value: &str) {
        table
            .lock()
            .expect("Sperre")
            .insert(pid.to_string(), value.to_string());
    }

    /// Alle Ereignisse, die bis jetzt angekommen sind
    fn seen(rx: &mut tokio::sync::mpsc::UnboundedReceiver<LiveEvent>) -> Vec<LiveEvent> {
        let mut events = Vec::new();
        while let Ok(event) = rx.try_recv() {
            events.push(event);
        }
        events
    }

    /// Das Ereignis eines Projekts
    fn docs_changed(pid: &str) -> LiveEvent {
        LiveEvent::DocsChanged {
            project_id: pid.to_string(),
        }
    }

    /// Lässt die Uhr laufen und die Takt-Aufgaben arbeiten
    async fn advance(ms: u64) {
        tokio::time::advance(Duration::from_millis(ms)).await;
        tokio::task::yield_now().await;
        tokio::task::yield_now().await;
    }

    #[tokio::test(start_paused = true)]
    async fn meldet_eine_neue_datei_in_docs_superpowers_specs_genau_einmal() {
        let base = tempfile::Builder::new()
            .prefix("nexus-docs-")
            .tempdir()
            .expect("Ordner");
        let specs = base.path().join("docs").join("superpowers").join("specs");
        std::fs::create_dir_all(&specs).expect("Ordner");
        let scanner = nexus_core::docs::SuperpowersDocScanner::new();
        let root = base.path().to_path_buf();
        let signature_of: SignatureOf = Arc::new(move |_pid| {
            let root = root.clone();
            Box::pin(async move { Ok(scanner.signature(&root)) })
        });
        let watcher = SpecDocWatcher::new(signature_of, 10_000);
        let (sender, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let _beobachtet = watcher.watch("p1", sender);
        advance(0).await;
        advance(10_000).await;
        assert_eq!(seen(&mut rx), []);

        std::fs::write(specs.join("2026-10-22-neu-design.md"), "# Neu\n").expect("Datei");
        advance(10_000).await;
        assert_eq!(seen(&mut rx), [docs_changed("p1")]);

        advance(10_000).await;
        assert_eq!(seen(&mut rx), []);
        watcher.close();
    }

    #[tokio::test(start_paused = true)]
    async fn meldet_ohne_anderung_der_signatur_nichts() {
        let (signature_of, _table) = signatures(&[("p1", "gleich")]);
        let watcher = SpecDocWatcher::new(signature_of, 10_000);
        let (sender, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let _beobachtet = watcher.watch("p1", sender);
        advance(30_000).await;
        assert_eq!(seen(&mut rx), []);
        watcher.close();
    }

    #[tokio::test(start_paused = true)]
    async fn fragt_ohne_zuhorer_nicht_ab_und_halt_nach_der_abmeldung_an() {
        let calls = Arc::new(AtomicU64::new(0));
        let gezahlt = calls.clone();
        let signature_of: SignatureOf = Arc::new(move |_pid| {
            gezahlt.fetch_add(1, Ordering::SeqCst);
            Box::pin(async { Ok("a".to_string()) })
        });
        let watcher = SpecDocWatcher::new(signature_of, 10_000);
        advance(30_000).await;
        assert_eq!(calls.load(Ordering::SeqCst), 0);

        let (sender, _rx) = tokio::sync::mpsc::unbounded_channel();
        let beobachtet = watcher.watch("p1", sender);
        advance(10_000).await;
        assert!(calls.load(Ordering::SeqCst) > 0);

        drop(beobachtet);
        calls.store(0, Ordering::SeqCst);
        advance(30_000).await;
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        watcher.close();
    }

    #[tokio::test(start_paused = true)]
    async fn meldet_allen_empfangern_desselben_projekts_anderen_projekten_nicht() {
        let (signature_of, table) = signatures(&[("p1", "a"), ("p2", "x")]);
        let watcher = SpecDocWatcher::new(signature_of, 10_000);
        let (eins, mut eins_rx) = tokio::sync::mpsc::unbounded_channel();
        let (zwei, mut zwei_rx) = tokio::sync::mpsc::unbounded_channel();
        let (fremd, mut fremd_rx) = tokio::sync::mpsc::unbounded_channel();
        let _a = watcher.watch("p1", eins);
        let _b = watcher.watch("p1", zwei);
        let _c = watcher.watch("p2", fremd);
        advance(0).await;

        set(&table, "p1", "b");
        advance(10_000).await;
        assert_eq!(seen(&mut eins_rx), [docs_changed("p1")]);
        assert_eq!(seen(&mut zwei_rx), [docs_changed("p1")]);
        assert_eq!(seen(&mut fremd_rx), []);
        watcher.close();
    }

    #[tokio::test(start_paused = true)]
    async fn ubersteht_einen_fehler_beim_abfragen_und_meldet_danach_weiter() {
        let state = Arc::new(Mutex::new(("a".to_string(), false)));
        let shared = state.clone();
        let signature_of: SignatureOf = Arc::new(move |_pid| {
            let (signature, fail) = shared.lock().expect("Sperre").clone();
            Box::pin(async move {
                if fail {
                    return Err(nexus_core::HarnessError::new("kaputt", "kaputt").into());
                }
                Ok(signature)
            })
        });
        let watcher = SpecDocWatcher::new(signature_of, 10_000);
        let (sender, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let _beobachtet = watcher.watch("p1", sender);
        advance(0).await;

        state.lock().expect("Sperre").1 = true;
        advance(10_000).await;
        assert_eq!(seen(&mut rx), []);

        {
            let mut state = state.lock().expect("Sperre");
            state.1 = false;
            state.0 = "b".to_string();
        }
        advance(10_000).await;
        assert_eq!(seen(&mut rx), [docs_changed("p1")]);
        watcher.close();
    }

    #[tokio::test(start_paused = true)]
    async fn fragt_im_takt_von_zehn_sekunden_ab() {
        let calls = Arc::new(AtomicU64::new(0));
        let gezahlt = calls.clone();
        let signature_of: SignatureOf = Arc::new(move |_pid| {
            gezahlt.fetch_add(1, Ordering::SeqCst);
            Box::pin(async { Ok("a".to_string()) })
        });
        let watcher = SpecDocWatcher::new(signature_of, 10_000);
        let (sender, _rx) = tokio::sync::mpsc::unbounded_channel();
        let _beobachtet = watcher.watch("p1", sender);
        advance(0).await;
        calls.store(0, Ordering::SeqCst);

        advance(9_000).await;
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        advance(1_000).await;
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        watcher.close();
    }
}
