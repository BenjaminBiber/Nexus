//! Die Favoriten des Servers: Zustandsspeicher im Speicher, jede Änderung wird sofort
//! geschrieben. Portiert aus `packages/server/src/favorites/FavoriteService.ts`.

use std::sync::Mutex;

use nexus_core::clock::Clock;
use nexus_core::favorites::{Favorite, FavoritePersistence, FavoriteStore, SessionSnapshot};
use nexus_core::sessions::SessionView;
use nexus_core::{HarnessError, HarnessHome, Result};

use crate::log::ServerLog;

/// Hält die Favoriten des Servers
pub struct FavoriteService {
    store: Mutex<FavoriteStore>,
    persistence: FavoritePersistence,
    log: ServerLog,
}

impl FavoriteService {
    /// Lädt die gespeicherten Favoriten; eine beschädigte Datei wird gesichert und im
    /// Protokoll vermerkt
    pub fn new(home: HarnessHome, clock: Clock, log: ServerLog) -> Self {
        let persistence = FavoritePersistence::new(home);
        let read = persistence
            .read()
            .unwrap_or_else(|_| nexus_core::favorites::FavoritesRead {
                favorites: Vec::new(),
                recovered: false,
            });
        if read.recovered {
            log.write("favorites.json war beschädigt und wurde nach favorites.json.bak gesichert.");
        }
        Self {
            store: Mutex::new(FavoriteStore::new(read.favorites, clock)),
            persistence,
            log,
        }
    }

    /// Markiert eine bekannte Session (oder aktualisiert Favorit und Notiz)
    pub fn mark(
        &self,
        project_id: &str,
        session: Option<&SessionView>,
        note: Option<&str>,
    ) -> Result<Favorite> {
        let Some(session) = session else {
            return Err(
                HarnessError::new("session-unknown", "Diese Session ist nicht bekannt.").into(),
            );
        };
        let favorite = self.store.lock().expect("Sperre der Favoriten").add(
            project_id,
            &snapshot(session),
            note,
        );
        self.save()?;
        Ok(favorite)
    }

    /// Ändert die Notiz eines Favoriten
    pub fn set_note(&self, project_id: &str, session_id: &str, note: &str) -> Result<Favorite> {
        let favorite = self
            .store
            .lock()
            .expect("Sperre der Favoriten")
            .set_note(project_id, session_id, note)?;
        self.save()?;
        Ok(favorite)
    }

    /// Entfernt die Markierung; liefert, ob es sie gab
    pub fn unmark(&self, project_id: &str, session_id: &str) -> Result<bool> {
        let removed = self
            .store
            .lock()
            .expect("Sperre der Favoriten")
            .remove(project_id, session_id);
        if removed {
            self.save()?;
        }
        Ok(removed)
    }

    /// Favoriten eines Projekts, zuletzt markierte zuerst
    pub fn list(&self, project_id: &str) -> Vec<Favorite> {
        self.store
            .lock()
            .expect("Sperre der Favoriten")
            .list(project_id)
    }

    /// Ist diese Session gemerkt?
    pub fn is_favorite(&self, project_id: &str, session_id: &str) -> bool {
        self.store
            .lock()
            .expect("Sperre der Favoriten")
            .is_favorite(project_id, session_id)
    }

    /// Nach einem Hook-Ereignis: Momentaufnahme nachführen und fortgesetzte Session zuordnen
    pub fn on_session(&self, project_id: &str, session: &SessionView, resumed: bool) -> Result<()> {
        let mut changed = false;
        {
            let mut store = self.store.lock().expect("Sperre der Favoriten");
            if store.is_favorite(project_id, &session.id) {
                store.refresh(project_id, &snapshot(session));
                changed = true;
            }
            if resumed
                && store
                    .link_resumed(project_id, &session.cwd, &session.id)
                    .is_some()
            {
                changed = true;
            }
        }
        if changed {
            self.save()?;
        }
        Ok(())
    }

    /// Schreibt die Favoriten; ein Fehler wird protokolliert und weitergegeben
    fn save(&self) -> Result<()> {
        let favorites = self.store.lock().expect("Sperre der Favoriten").to_vec();
        self.persistence.write(&favorites).inspect_err(|error| {
            self.log.write(&format!(
                "Favoriten konnten nicht gespeichert werden: {error}"
            ));
        })
    }
}

/// Momentaufnahme einer Session
fn snapshot(session: &SessionView) -> SessionSnapshot {
    SessionSnapshot {
        session_id: session.id.clone(),
        tickets: session.tickets.clone(),
        cwd: session.cwd.clone(),
        title: session.title.clone(),
        state: session.state,
        last_event_at: session.last_event_at.clone(),
    }
}
