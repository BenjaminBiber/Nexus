//! Beobachtet den Änderungskennwert der Board-Datenbank; portiert aus
//! `packages/server/src/events/BoardChangeWatcher.ts`.
//!
//! Für jede beobachtete ID (ein DB-Board oder ein Board-Projekt) fragt der Takt regelmäßig
//! einen Kennwert ab und meldet jede Änderung an alle Ströme, die diese ID beobachten. Der
//! Takt läuft nur, solange es Beobachter gibt. Es gibt keinen Dateisystem-Beobachter: beim
//! SQL Server gibt es keine Datei, auf die man hören könnte.

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

/// Die Abfrage eines Kennwerts (sie geht in die Datenbank und dauert deshalb)
pub type MarkerFuture = Pin<Box<dyn Future<Output = Result<String>> + Send>>;

/// Woher der Kennwert einer ID kommt
pub type MarkerOf = Arc<dyn Fn(i32) -> MarkerFuture + Send + Sync>;

/// Ein angemeldeter Empfänger
struct Listener {
    id: u64,
    sender: UnboundedSender<LiveEvent>,
}

/// Eine beobachtete ID mit ihrem zuletzt gesehenen Kennwert
#[derive(Default)]
struct Entry {
    listeners: Vec<Listener>,
    /// `None`, solange der Ausgangswert fehlt – bis dahin wird nichts gemeldet
    marker: Option<String>,
}

/// Fragt für jede beobachtete ID regelmäßig einen Änderungskennwert ab und meldet
/// Änderungen; welches Ereignis daraus wird, bestimmt `event_of` (`dbboard` oder
/// `dbquestions`).
pub struct ChangeWatcher {
    entries: Mutex<HashMap<i32, Entry>>,
    marker_of: MarkerOf,
    event_of: fn(i32) -> LiveEvent,
    interval: Duration,
    /// Nie zwei Durchläufe gleichzeitig
    polling: AtomicBool,
    timer: Mutex<Option<JoinHandle<()>>>,
    next_id: AtomicU64,
}

impl ChangeWatcher {
    /// Beobachter der DB-Boards: eine Änderung wird zu `{"type":"dbboard","boardId":…}`
    pub fn boards(marker_of: MarkerOf, interval_ms: u64) -> Arc<Self> {
        Self::new(marker_of, interval_ms, |id| LiveEvent::DbBoard {
            board_id: id,
        })
    }

    /// Beobachter der Fragen eines Board-Projekts:
    /// `{"type":"dbquestions","projectId":…}`
    pub fn questions(marker_of: MarkerOf, interval_ms: u64) -> Arc<Self> {
        Self::new(marker_of, interval_ms, |id| LiveEvent::DbQuestions {
            project_id: id,
        })
    }

    /// Beobachter mit eigener Ereignisfunktion
    pub fn new(marker_of: MarkerOf, interval_ms: u64, event_of: fn(i32) -> LiveEvent) -> Arc<Self> {
        Arc::new(Self {
            entries: Mutex::new(HashMap::new()),
            marker_of,
            event_of,
            interval: Duration::from_millis(interval_ms.max(1)),
            polling: AtomicBool::new(false),
            timer: Mutex::new(None),
            next_id: AtomicU64::new(0),
        })
    }

    /// Beobachtet eine ID; die Beobachtung endet, wenn das Ergebnis fällt
    pub fn watch(self: &Arc<Self>, id: i32, sender: UnboundedSender<LiveEvent>) -> Watched {
        let listener = self.next_id.fetch_add(1, Ordering::Relaxed);
        let fresh = {
            let mut entries = self.entries.lock().expect("Sperre der Beobachtungen");
            let entry = entries.entry(id).or_default();
            let fresh = entry.listeners.is_empty() && entry.marker.is_none();
            entry.listeners.push(Listener {
                id: listener,
                sender,
            });
            fresh
        };
        // Ausgangswert ohne Meldung lesen; ein Fehler wartet auf den nächsten Durchlauf
        if fresh {
            let watcher = self.clone();
            tokio::spawn(async move { watcher.prime(id).await });
        }
        self.ensure_timer();
        Watched {
            watcher: self.clone(),
            id,
            listener,
        }
    }

    /// Ein Durchlauf: Kennwerte abfragen und Änderungen melden (nie zwei gleichzeitig)
    pub async fn poll(&self) {
        if self.polling.swap(true, Ordering::SeqCst) {
            return;
        }
        let _running = Polling(&self.polling);
        for id in self.ids() {
            // Ein Fehler beim Kennwert überspringt diese ID; der nächste Durchlauf versucht
            // es wieder
            let Ok(marker) = (self.marker_of)(id).await else {
                continue;
            };
            let mut entries = self.entries.lock().expect("Sperre der Beobachtungen");
            let Some(entry) = entries.get_mut(&id) else {
                continue;
            };
            let changed = entry.marker.as_deref().is_some_and(|seen| seen != marker);
            entry.marker = Some(marker);
            if changed {
                let event = (self.event_of)(id);
                for listener in &entry.listeners {
                    let _ = listener.sender.send(event.clone());
                }
            }
        }
    }

    /// Beendet alle Beobachtungen (Ende des Servers)
    pub fn close(&self) {
        self.entries
            .lock()
            .expect("Sperre der Beobachtungen")
            .clear();
        self.stop_timer();
    }

    /// Zahl der Beobachter einer ID (Tests und der Aufräum-Nachweis)
    pub fn watchers(&self, id: i32) -> usize {
        self.entries
            .lock()
            .expect("Sperre der Beobachtungen")
            .get(&id)
            .map(|entry| entry.listeners.len())
            .unwrap_or(0)
    }

    /// Die beobachteten IDs in fester Reihenfolge
    fn ids(&self) -> Vec<i32> {
        let mut ids: Vec<i32> = self
            .entries
            .lock()
            .expect("Sperre der Beobachtungen")
            .keys()
            .copied()
            .collect();
        ids.sort_unstable();
        ids
    }

    /// Ausgangswert beim ersten Beobachten, ohne Meldung
    async fn prime(&self, id: i32) {
        let Ok(marker) = (self.marker_of)(id).await else {
            return;
        };
        let mut entries = self.entries.lock().expect("Sperre der Beobachtungen");
        if let Some(entry) = entries.get_mut(&id) {
            if entry.marker.is_none() {
                entry.marker = Some(marker);
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

    /// Meldet einen Empfänger ab; die letzte Abmeldung einer ID nimmt die Beobachtung mit,
    /// die letzte Beobachtung den Takt
    fn unwatch(&self, id: i32, listener: u64) {
        let empty = {
            let mut entries = self.entries.lock().expect("Sperre der Beobachtungen");
            if let Some(entry) = entries.get_mut(&id) {
                entry.listeners.retain(|other| other.id != listener);
                if entry.listeners.is_empty() {
                    entries.remove(&id);
                }
            }
            entries.is_empty()
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

/// Eine laufende Beobachtung; sie endet, wenn sie fällt (der Browser geht, der Server endet)
pub struct Watched {
    watcher: Arc<ChangeWatcher>,
    id: i32,
    listener: u64,
}

impl Drop for Watched {
    fn drop(&mut self) {
        self.watcher.unwatch(self.id, self.listener);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Kennwerte aus einer Tabelle, die der Test ändern kann
    fn markers(values: &[(i32, &str)]) -> (MarkerOf, Arc<Mutex<HashMap<i32, String>>>) {
        let table: Arc<Mutex<HashMap<i32, String>>> = Arc::new(Mutex::new(
            values
                .iter()
                .map(|(id, value)| (*id, (*value).to_string()))
                .collect(),
        ));
        let shared = table.clone();
        let marker_of: MarkerOf = Arc::new(move |id| {
            let table = shared.clone();
            Box::pin(async move {
                let value = table.lock().expect("Sperre").get(&id).cloned();
                value.ok_or_else(|| {
                    nexus_core::HarnessError::new("kein-kennwert", "kein Kennwert").into()
                })
            })
        });
        (marker_of, table)
    }

    /// Setzt einen Kennwert der Tabelle
    fn set(table: &Arc<Mutex<HashMap<i32, String>>>, id: i32, value: &str) {
        table
            .lock()
            .expect("Sperre")
            .insert(id, value.to_string())
            .expect("vorher gesetzt");
    }

    /// Alle Ereignisse, die bis jetzt angekommen sind
    fn seen(rx: &mut tokio::sync::mpsc::UnboundedReceiver<LiveEvent>) -> Vec<LiveEvent> {
        let mut events = Vec::new();
        while let Ok(event) = rx.try_recv() {
            events.push(event);
        }
        events
    }

    /// Lässt die Uhr laufen und die Takt-Aufgaben arbeiten
    async fn advance(ms: u64) {
        tokio::time::advance(Duration::from_millis(ms)).await;
        tokio::task::yield_now().await;
        tokio::task::yield_now().await;
    }

    #[tokio::test(start_paused = true)]
    async fn meldet_eine_anderung_des_kennwerts_genau_einmal_an_alle_empfanger_dieses_boards() {
        let (marker_of, table) = markers(&[(1, "a"), (2, "x")]);
        let watcher = ChangeWatcher::boards(marker_of, 2500);
        let (eins, mut eins_rx) = tokio::sync::mpsc::unbounded_channel();
        let (zwei, mut zwei_rx) = tokio::sync::mpsc::unbounded_channel();
        let (fremd, mut fremd_rx) = tokio::sync::mpsc::unbounded_channel();
        let _a = watcher.watch(1, eins);
        let _b = watcher.watch(1, zwei);
        let _c = watcher.watch(2, fremd);
        advance(0).await;
        advance(2500).await;
        assert_eq!(seen(&mut eins_rx), []);

        set(&table, 1, "b");
        advance(2500).await;
        assert_eq!(seen(&mut eins_rx), [LiveEvent::DbBoard { board_id: 1 }]);
        assert_eq!(seen(&mut zwei_rx), [LiveEvent::DbBoard { board_id: 1 }]);
        assert_eq!(seen(&mut fremd_rx), []);

        advance(2500).await;
        assert_eq!(seen(&mut eins_rx), []);
        watcher.close();
    }

    #[tokio::test(start_paused = true)]
    async fn fragt_nur_boards_mit_empfangern_ab_und_halt_ohne_empfanger_an() {
        let calls = Arc::new(AtomicU64::new(0));
        let gezahlt = calls.clone();
        let marker_of: MarkerOf = Arc::new(move |_id| {
            gezahlt.fetch_add(1, Ordering::SeqCst);
            Box::pin(async { Ok("a".to_string()) })
        });
        let watcher = ChangeWatcher::boards(marker_of, 2500);
        advance(10_000).await;
        assert_eq!(calls.load(Ordering::SeqCst), 0);

        let (sender, _rx) = tokio::sync::mpsc::unbounded_channel();
        let beobachtet = watcher.watch(7, sender);
        advance(2500).await;
        assert!(calls.load(Ordering::SeqCst) > 0);

        drop(beobachtet);
        calls.store(0, Ordering::SeqCst);
        advance(10_000).await;
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test(start_paused = true)]
    async fn ubersteht_fehler_beim_abfragen_und_meldet_danach_anderungen() {
        let state = Arc::new(Mutex::new(("a".to_string(), true)));
        let shared = state.clone();
        let marker_of: MarkerOf = Arc::new(move |_id| {
            let (marker, fail) = shared.lock().expect("Sperre").clone();
            Box::pin(async move {
                if fail {
                    return Err(nexus_core::HarnessError::new(
                        "board-db-unreachable",
                        "Datenbank weg",
                    )
                    .into());
                }
                Ok(marker)
            })
        });
        let watcher = ChangeWatcher::boards(marker_of, 2500);
        let (sender, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let _beobachtet = watcher.watch(1, sender);
        advance(2500).await;

        state.lock().expect("Sperre").1 = false;
        advance(2500).await;
        assert_eq!(seen(&mut rx), []);

        state.lock().expect("Sperre").0 = "b".to_string();
        advance(2500).await;
        assert_eq!(seen(&mut rx), [LiveEvent::DbBoard { board_id: 1 }]);
        watcher.close();
    }

    #[tokio::test(start_paused = true)]
    async fn meldet_mit_eigener_ereignisfunktion_dbquestions_je_projekt() {
        let (marker_of, table) = markers(&[(7, "a")]);
        let watcher = ChangeWatcher::questions(marker_of, 2500);
        let (sender, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let _beobachtet = watcher.watch(7, sender);
        advance(0).await;

        set(&table, 7, "b");
        advance(2500).await;
        assert_eq!(seen(&mut rx), [LiveEvent::DbQuestions { project_id: 7 }]);
        watcher.close();
    }

    #[tokio::test(start_paused = true)]
    async fn die_letzte_abmeldung_einer_id_nimmt_die_beobachtung_mit() {
        let (marker_of, _table) = markers(&[(1, "a")]);
        let watcher = ChangeWatcher::boards(marker_of, 2500);
        let (eins, _eins_rx) = tokio::sync::mpsc::unbounded_channel();
        let (zwei, _zwei_rx) = tokio::sync::mpsc::unbounded_channel();
        let erste = watcher.watch(1, eins);
        let zweite = watcher.watch(1, zwei);
        assert_eq!(watcher.watchers(1), 2);
        drop(erste);
        assert_eq!(watcher.watchers(1), 1);
        drop(zweite);
        assert_eq!(watcher.watchers(1), 0);
    }
}
