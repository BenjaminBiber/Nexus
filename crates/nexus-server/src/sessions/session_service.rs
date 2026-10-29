//! Die Sessions des Servers: Zustandsspeicher im Speicher, entprelltes Speichern und die
//! Meldung `sessions` an offene Seiten. Portiert aus
//! `packages/server/src/sessions/SessionService.ts`.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use nexus_core::clock::Clock;
use nexus_core::sessions::{HookEvent, Session, SessionPersistence, SessionStore, SessionView};
use nexus_core::{HarnessHome, Result};
use tokio::task::JoinHandle;

use crate::events::{EventHub, LiveEvent};

/// So lange wird nach einer Änderung gewartet, bevor `sessions.json` geschrieben wird
pub const SAVE_DELAY_MS: u64 = 300;

/// Speicher und Datei; der entprellte Schreibvorgang arbeitet nur auf diesem Teil
struct Stored {
    store: Mutex<SessionStore>,
    persistence: SessionPersistence,
}

impl Stored {
    /// Schreibt alle Sessions; ein Fehler kostet nur den Verlauf und bricht nichts ab
    fn save(&self) {
        let sessions = self.store.lock().expect("Sperre der Sessions").to_vec();
        let _ = self.persistence.write(&sessions);
    }
}

/// Hält die Sessions des Servers: jedes Hook-Ereignis geht in den Speicher, wird gemeldet und
/// kurz danach geschrieben.
pub struct SessionService {
    stored: Arc<Stored>,
    events: Arc<EventHub>,
    /// Der laufende Schreibauftrag; es gibt immer höchstens einen
    timer: Mutex<Option<JoinHandle<()>>>,
}

impl SessionService {
    /// Lädt die gespeicherten Sessions; eine beschädigte Datei führt zu einem leeren Start
    pub fn new(home: HarnessHome, events: Arc<EventHub>, clock: Clock) -> Self {
        let persistence = SessionPersistence::new(home);
        // Beschädigte Datei: neu beginnen, sie wird beim nächsten Speichern ersetzt
        let sessions = persistence.read().unwrap_or_default();
        let mut store = SessionStore::new(sessions, clock);
        store.prune();
        Self {
            stored: Arc::new(Stored {
                store: Mutex::new(store),
                persistence,
            }),
            events,
            timer: Mutex::new(None),
        }
    }

    /// Wendet ein Ereignis an, plant das Speichern und meldet die Änderung
    pub fn apply(&self, project_id: &str, event: &HookEvent) -> Session {
        let session = self
            .stored
            .store
            .lock()
            .expect("Sperre der Sessions")
            .apply(project_id, event);
        self.changed(project_id);
        session
    }

    /// Ordnet eine Session einem Ticket zu
    pub fn claim(&self, project_id: &str, session_id: &str, ticket_id: &str) -> Result<Session> {
        let session = self
            .stored
            .store
            .lock()
            .expect("Sperre der Sessions")
            .claim(project_id, session_id, ticket_id)?;
        self.changed(project_id);
        Ok(session)
    }

    /// Sessions eines Projekts, neueste Aktivität zuerst
    pub fn list(&self, project_id: &str) -> Vec<SessionView> {
        self.stored
            .store
            .lock()
            .expect("Sperre der Sessions")
            .list(Some(project_id))
    }

    /// Eine Session eines Projekts, falls sie bekannt ist
    pub fn find(&self, project_id: &str, session_id: &str) -> Option<SessionView> {
        self.list(project_id)
            .into_iter()
            .find(|view| view.id == session_id)
    }

    /// Sessions eines Tickets
    pub fn for_ticket(&self, project_id: &str, ticket_id: &str) -> Vec<SessionView> {
        self.stored
            .store
            .lock()
            .expect("Sperre der Sessions")
            .for_ticket(project_id, ticket_id)
    }

    /// Schreibt ausstehende Änderungen sofort (beim Beenden des Servers)
    pub async fn close(&self) {
        let pending = self
            .timer
            .lock()
            .expect("Sperre des Schreibauftrags")
            .take();
        if let Some(task) = pending {
            task.abort();
            let stored = self.stored.clone();
            let _ = tokio::task::spawn_blocking(move || stored.save()).await;
        }
    }

    /// Nach einer Änderung: melden und das Speichern einplanen
    fn changed(&self, project_id: &str) {
        self.events.publish(project_id, LiveEvent::Sessions);
        let mut timer = self.timer.lock().expect("Sperre des Schreibauftrags");
        if timer.as_ref().is_some_and(|task| !task.is_finished()) {
            return;
        }
        let stored = self.stored.clone();
        *timer = Some(tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(SAVE_DELAY_MS)).await;
            let _ = tokio::task::spawn_blocking(move || stored.save()).await;
        }));
    }
}
