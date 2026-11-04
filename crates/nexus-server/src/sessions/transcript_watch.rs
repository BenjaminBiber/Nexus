//! Beobachtet die Transcripts offener Detailansichten: zwischen zwei Hooks (lange Antworten,
//! viele Tool-Aufrufe) wächst die Datei, ohne dass ein Ereignis kommt. Jede Anfrage des
//! Verlaufs meldet die Session für [`EXPIRY`] als beobachtet; ein Takt prüft alle
//! [`POLL`] Größe und Änderungszeit und sendet bei Änderung `session-activity`. Der Takt
//! läuft nur, solange etwas beobachtet wird.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, Weak};
use std::time::{Duration, Instant, SystemTime};

use tokio::task::JoinHandle;

use crate::events::{EventHub, LiveEvent};

/// Ablauf der Beobachtung nach der letzten Anfrage
pub const EXPIRY: Duration = Duration::from_secs(60);
/// Abstand der Prüfungen
pub const POLL: Duration = Duration::from_secs(2);

/// Größe und Änderungszeit einer Datei
type Marker = Option<(u64, Option<SystemTime>)>;

/// Eine beobachtete Session
struct Watched {
    path: PathBuf,
    until: Instant,
    marker: Marker,
}

/// Beobachtet Transcript-Dateien und meldet Änderungen
pub struct TranscriptWatch {
    events: Arc<EventHub>,
    poll: Duration,
    expiry: Duration,
    /// (Projekt, Session) → Beobachtung
    watched: Mutex<HashMap<(String, String), Watched>>,
    timer: Mutex<Option<JoinHandle<()>>>,
}

impl TranscriptWatch {
    /// Beobachtung mit den Standardzeiten
    pub fn new(events: Arc<EventHub>) -> Arc<Self> {
        Self::with_times(events, POLL, EXPIRY)
    }

    /// Beobachtung mit eigenen Zeiten (Tests)
    pub fn with_times(events: Arc<EventHub>, poll: Duration, expiry: Duration) -> Arc<Self> {
        Arc::new(Self {
            events,
            poll,
            expiry,
            watched: Mutex::new(HashMap::new()),
            timer: Mutex::new(None),
        })
    }

    /// Meldet die Session als beobachtet (verlängert eine laufende Beobachtung)
    pub fn watch(self: &Arc<Self>, project_id: &str, session_id: &str, path: &Path) {
        let until = Instant::now() + self.expiry;
        {
            let mut watched = self.watched.lock().expect("Sperre der Beobachtung");
            let key = (project_id.to_string(), session_id.to_string());
            match watched.get_mut(&key) {
                Some(entry) if entry.path == path => entry.until = until,
                _ => {
                    watched.insert(
                        key,
                        Watched {
                            path: path.to_path_buf(),
                            until,
                            marker: marker(path),
                        },
                    );
                }
            }
        }
        self.ensure_timer();
    }

    /// Anzahl beobachteter Sessions
    pub fn len(&self) -> usize {
        self.watched.lock().expect("Sperre der Beobachtung").len()
    }

    /// Ob nichts beobachtet wird
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Ob der Takt läuft
    pub fn running(&self) -> bool {
        self.timer
            .lock()
            .expect("Sperre des Takts")
            .as_ref()
            .is_some_and(|task| !task.is_finished())
    }

    /// Beendet Takt und Beobachtung (Herunterfahren)
    pub fn close(&self) {
        self.watched.lock().expect("Sperre der Beobachtung").clear();
        if let Some(task) = self.timer.lock().expect("Sperre des Takts").take() {
            task.abort();
        }
    }

    /// Prüft alle beobachteten Dateien einmal; liefert, ob weiter beobachtet wird
    pub fn tick(&self) -> bool {
        let now = Instant::now();
        let mut changed: Vec<(String, String)> = Vec::new();
        let left = {
            let mut watched = self.watched.lock().expect("Sperre der Beobachtung");
            watched.retain(|_, entry| entry.until > now);
            for (key, entry) in watched.iter_mut() {
                let current = marker(&entry.path);
                if current != entry.marker {
                    entry.marker = current;
                    changed.push(key.clone());
                }
            }
            watched.len()
        };
        for (project_id, session_id) in changed {
            self.events
                .publish(&project_id, LiveEvent::SessionActivity { session_id });
        }
        left > 0
    }

    /// Startet den Takt, falls er nicht läuft
    fn ensure_timer(self: &Arc<Self>) {
        let mut timer = self.timer.lock().expect("Sperre des Takts");
        if timer.as_ref().is_some_and(|task| !task.is_finished()) {
            return;
        }
        let weak: Weak<Self> = Arc::downgrade(self);
        let poll = self.poll;
        *timer = Some(tokio::spawn(async move {
            let mut interval = tokio::time::interval_at(tokio::time::Instant::now() + poll, poll);
            loop {
                interval.tick().await;
                let Some(watch) = weak.upgrade() else {
                    return;
                };
                if !watch.tick() {
                    return;
                }
            }
        }));
    }
}

/// Größe und Änderungszeit; nichts, wenn die Datei fehlt
fn marker(path: &Path) -> Marker {
    let meta = std::fs::metadata(path).ok()?;
    Some((meta.len(), meta.modified().ok()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn listener(
        hub: &Arc<EventHub>,
        project: &str,
    ) -> (
        crate::events::Subscription,
        tokio::sync::mpsc::UnboundedReceiver<LiveEvent>,
    ) {
        let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();
        (hub.subscribe(project, sender), receiver)
    }

    #[tokio::test]
    async fn meldet_eine_gewachsene_datei_und_sonst_nichts() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("s.jsonl");
        std::fs::write(&path, "{}\n").unwrap();
        let hub = EventHub::new();
        let (_subscription, mut receiver) = listener(&hub, "p1");
        let watch = TranscriptWatch::with_times(hub.clone(), Duration::from_secs(3600), EXPIRY);
        watch.watch("p1", "s1", &path);
        assert!(watch.running());
        assert!(watch.tick());
        assert!(receiver.try_recv().is_err(), "unverändert: keine Meldung");
        std::fs::write(&path, "{}\n{}\n").unwrap();
        assert!(watch.tick());
        assert_eq!(
            receiver.try_recv().unwrap(),
            LiveEvent::SessionActivity {
                session_id: "s1".into()
            }
        );
        watch.close();
        assert!(watch.is_empty());
        assert!(!watch.running());
    }

    #[tokio::test]
    async fn eine_abgelaufene_beobachtung_endet_samt_takt() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("s.jsonl");
        std::fs::write(&path, "{}\n").unwrap();
        let watch = TranscriptWatch::with_times(
            EventHub::new(),
            Duration::from_millis(20),
            Duration::from_millis(1),
        );
        watch.watch("p1", "s1", &path);
        tokio::time::sleep(Duration::from_millis(200)).await;
        assert!(watch.is_empty());
        assert!(!watch.running());
    }
}
