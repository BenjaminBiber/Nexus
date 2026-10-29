//! Zustandsspeicher der Favoriten (reine Logik, ohne Dateien).
//! Portiert aus `packages/core/src/favorites/FavoriteStore.ts`.

use crate::clock::Clock;
use crate::error::{HarnessError, Result};

use super::favorite::{Favorite, SessionSnapshot};

/// Zustandsspeicher der Favoriten aller Projekte
#[derive(Debug)]
pub struct FavoriteStore {
    favorites: Vec<Favorite>,
    clock: Clock,
    /// Plattform für den Vergleich der Verzeichnisse (`process.platform` in TypeScript)
    platform: &'static str,
}

impl FavoriteStore {
    /// Speicher über diesem Bestand auf der Plattform, auf der er gerade läuft
    pub fn new(favorites: Vec<Favorite>, clock: Clock) -> Self {
        Self::with_platform(
            favorites,
            clock,
            if cfg!(windows) { "win32" } else { "linux" },
        )
    }

    /// Derselbe Speicher mit vorgegebener Plattform (für Tests der Pfadnormalisierung)
    pub fn with_platform(favorites: Vec<Favorite>, clock: Clock, platform: &'static str) -> Self {
        Self {
            favorites,
            clock,
            platform,
        }
    }

    /// Markiert eine Session; erneutes Markieren aktualisiert die Momentaufnahme und behält
    /// die Notiz
    pub fn add(
        &mut self,
        project_id: &str,
        snapshot: &SessionSnapshot,
        note: Option<&str>,
    ) -> Favorite {
        let index = match self.index(project_id, &snapshot.session_id) {
            Some(index) => {
                if let Some(note) = note {
                    self.favorites[index].note = note.to_string();
                }
                index
            }
            None => {
                self.favorites.push(Favorite {
                    project_id: project_id.to_string(),
                    session_id: snapshot.session_id.clone(),
                    note: note.unwrap_or_default().to_string(),
                    tickets: Vec::new(),
                    cwd: snapshot.cwd.clone(),
                    title: None,
                    marked_at: self.clock.iso(),
                    last_state: snapshot.state,
                    last_event_at: snapshot.last_event_at.clone(),
                    resumed_as: Vec::new(),
                });
                self.favorites.len() - 1
            }
        };
        apply(&mut self.favorites[index], snapshot);
        self.favorites[index].clone()
    }

    /// Aktualisiert die Momentaufnahme eines Favoriten (Unbekannte werden ignoriert)
    pub fn refresh(&mut self, project_id: &str, snapshot: &SessionSnapshot) {
        if let Some(index) = self.index(project_id, &snapshot.session_id) {
            apply(&mut self.favorites[index], snapshot);
        }
    }

    /// Ändert die Notiz
    pub fn set_note(&mut self, project_id: &str, session_id: &str, note: &str) -> Result<Favorite> {
        let index = self.index(project_id, session_id).ok_or_else(|| {
            HarnessError::new(
                "favorite-not-found",
                format!("Favorit {session_id} gibt es nicht."),
            )
        })?;
        self.favorites[index].note = note.to_string();
        Ok(self.favorites[index].clone())
    }

    /// Entfernt die Markierung; liefert, ob es sie gab
    pub fn remove(&mut self, project_id: &str, session_id: &str) -> bool {
        let before = self.favorites.len();
        self.favorites
            .retain(|f| !(f.project_id == project_id && f.session_id == session_id));
        self.favorites.len() < before
    }

    /// Favoriten eines Projekts, zuletzt markierte zuerst
    pub fn list(&self, project_id: &str) -> Vec<Favorite> {
        let mut favorites: Vec<Favorite> = self
            .favorites
            .iter()
            .filter(|f| f.project_id == project_id)
            .cloned()
            .collect();
        favorites.sort_by(|a, b| b.marked_at.cmp(&a.marked_at));
        favorites
    }

    /// Ist diese Session gemerkt?
    pub fn is_favorite(&self, project_id: &str, session_id: &str) -> bool {
        self.index(project_id, session_id).is_some()
    }

    /// Ordnet eine fortgesetzte Session dem einzigen Favoriten desselben Arbeitsverzeichnisses
    /// zu (sonst nichts)
    pub fn link_resumed(
        &mut self,
        project_id: &str,
        cwd: &str,
        new_session_id: &str,
    ) -> Option<Favorite> {
        let key = self.key(cwd);
        let matches: Vec<usize> = self
            .favorites
            .iter()
            .enumerate()
            .filter(|(_, f)| f.project_id == project_id && self.key(&f.cwd) == key)
            .map(|(index, _)| index)
            .collect();
        let [index] = matches[..] else { return None };
        if self.favorites[index].session_id == new_session_id {
            return None;
        }
        let favorite = &mut self.favorites[index];
        if !favorite.resumed_as.iter().any(|id| id == new_session_id) {
            favorite.resumed_as.push(new_session_id.to_string());
        }
        Some(favorite.clone())
    }

    /// Alle Favoriten zum Speichern
    pub fn to_vec(&self) -> Vec<Favorite> {
        self.favorites.clone()
    }

    /// Stelle des Favoriten im Bestand
    fn index(&self, project_id: &str, session_id: &str) -> Option<usize> {
        self.favorites
            .iter()
            .position(|f| f.project_id == project_id && f.session_id == session_id)
    }

    /// Vergleichsschlüssel für Verzeichnisse: Schrägstriche, ohne Endstrich, unter Windows
    /// ohne Beachtung der Schreibweise
    fn key(&self, path: &str) -> String {
        let cleaned = path.replace('\\', "/");
        let cleaned = cleaned.trim_end_matches('/');
        if self.platform == "win32" {
            cleaned.to_lowercase()
        } else {
            cleaned.to_string()
        }
    }
}

/// Übernimmt Angaben aus der Momentaufnahme
fn apply(favorite: &mut Favorite, snapshot: &SessionSnapshot) {
    favorite.tickets = snapshot.tickets.clone();
    favorite.cwd = snapshot.cwd.clone();
    if let Some(title) = snapshot.title.clone() {
        favorite.title = Some(title);
    }
    favorite.last_state = snapshot.state;
    favorite.last_event_at = snapshot.last_event_at.clone();
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use chrono::{DateTime, Duration, Utc};

    use crate::sessions::SessionState;

    use super::*;

    /// Eine Momentaufnahme dieser Session
    fn snap(id: &str) -> SessionSnapshot {
        SessionSnapshot {
            session_id: id.to_string(),
            tickets: vec!["T-0001".into()],
            cwd: "D:/Mein Projekt/wt".into(),
            title: Some("Login".into()),
            state: SessionState::Running,
            last_event_at: "2026-10-10T10:00:00.000Z".into(),
        }
    }

    /// Ein Speicher mit fester Uhr
    fn store() -> FavoriteStore {
        FavoriteStore::with_platform(
            Vec::new(),
            Clock::fixed(
                "2026-10-10T12:00:00.000Z"
                    .parse()
                    .expect("der Zeitpunkt ist gültig"),
            ),
            "linux",
        )
    }

    #[test]
    fn markiert_eine_session_mit_momentaufnahme_und_notiz() {
        let mut s = store();
        let f = s.add("p1", &snap("s1"), Some("Wichtig"));
        assert_eq!(f.project_id, "p1");
        assert_eq!(f.session_id, "s1");
        assert_eq!(f.note, "Wichtig");
        assert_eq!(f.tickets, ["T-0001"]);
        assert_eq!(f.cwd, "D:/Mein Projekt/wt");
        assert_eq!(f.title.as_deref(), Some("Login"));
        assert_eq!(f.last_state, SessionState::Running);
        assert!(f.resumed_as.is_empty());
        assert_eq!(f.marked_at, "2026-10-10T12:00:00.000Z");
        assert!(s.is_favorite("p1", "s1"));
        assert!(!s.is_favorite("p1", "x"));
    }

    #[test]
    fn ist_idempotent_behalt_die_notiz_und_aktualisiert_die_momentaufnahme() {
        let mut s = store();
        s.add("p1", &snap("s1"), Some("Notiz"));
        let again = s.add(
            "p1",
            &SessionSnapshot {
                state: SessionState::Idle,
                tickets: vec!["T-0001".into(), "T-0002".into()],
                ..snap("s1")
            },
            None,
        );
        assert_eq!(again.note, "Notiz");
        assert_eq!(again.last_state, SessionState::Idle);
        assert_eq!(again.tickets, ["T-0001", "T-0002"]);
        assert_eq!(s.list("p1").len(), 1);
    }

    #[test]
    fn andert_die_notiz_und_entfernt_favoriten() {
        let mut s = store();
        s.add("p1", &snap("s1"), None);
        assert_eq!(s.set_note("p1", "s1", "Neu").unwrap().note, "Neu");
        assert!(s.remove("p1", "s1"));
        assert!(!s.remove("p1", "s1"));
        assert!(s.list("p1").is_empty());
    }

    #[test]
    fn meldet_eine_unbekannte_notiz_anderung_verstandlich() {
        let error = store().set_note("p1", "gibtsnicht", "x").unwrap_err();
        assert_eq!(error.code(), Some("favorite-not-found"));
        assert!(error.to_string().contains("Favorit"));
    }

    #[test]
    fn halt_projekte_getrennt_und_sortiert_neueste_markierung_zuerst() {
        let now: Arc<Mutex<DateTime<Utc>>> = Arc::new(Mutex::new(
            "2026-10-10T12:00:00.000Z"
                .parse()
                .expect("der Zeitpunkt ist gültig"),
        ));
        let clock = Clock::from_fn({
            let now = Arc::clone(&now);
            move || {
                let mut at = now.lock().expect("die Uhr ist nicht vergiftet");
                let jetzt = *at;
                *at += Duration::seconds(1);
                jetzt
            }
        });
        let mut s = FavoriteStore::with_platform(Vec::new(), clock, "linux");
        s.add("p1", &snap("a"), None);
        s.add("p2", &snap("b"), None);
        s.add("p1", &snap("c"), None);
        assert_eq!(ids(&s.list("p1")), ["c", "a"]);
        assert_eq!(ids(&s.list("p2")), ["b"]);
    }

    /// Die Session-Kennungen der Favoriten in ihrer Reihenfolge
    fn ids(favorites: &[Favorite]) -> Vec<&str> {
        favorites.iter().map(|f| f.session_id.as_str()).collect()
    }

    #[test]
    fn aktualisiert_die_momentaufnahme_eines_favoriten_und_ignoriert_unbekannte() {
        let mut s = store();
        s.add("p1", &snap("s1"), None);
        s.refresh(
            "p1",
            &SessionSnapshot {
                state: SessionState::Ended,
                last_event_at: "2026-10-10T11:00:00.000Z".into(),
                ..snap("s1")
            },
        );
        s.refresh("p1", &snap("unbekannt"));
        let list = s.list("p1");
        assert_eq!(list[0].last_state, SessionState::Ended);
        assert_eq!(list[0].last_event_at, "2026-10-10T11:00:00.000Z");
        assert_eq!(list.len(), 1);
    }

    #[test]
    fn ordnet_eine_fortgesetzte_session_dem_einzigen_favoriten_im_selben_verzeichnis_zu() {
        let mut s = store();
        s.add("p1", &snap("s1"), None);
        let linked = s.link_resumed("p1", "D:\\Mein Projekt\\wt", "neu-1");
        assert_eq!(linked.map(|f| f.session_id), Some("s1".to_string()));
        assert_eq!(s.list("p1")[0].resumed_as, ["neu-1"]);
        s.link_resumed("p1", "D:/Mein Projekt/wt", "neu-1");
        assert_eq!(s.list("p1")[0].resumed_as, ["neu-1"]);
    }

    #[test]
    fn ordnet_bei_mehreren_passenden_favoriten_oder_anderem_verzeichnis_nichts_zu() {
        let mut s = store();
        s.add("p1", &snap("a"), None);
        s.add("p1", &snap("b"), None);
        assert!(s.link_resumed("p1", "D:/Mein Projekt/wt", "x").is_none());
        assert!(s.link_resumed("p1", "D:/anderswo", "x").is_none());
        assert!(s.link_resumed("p2", "D:/Mein Projekt/wt", "x").is_none());
    }

    #[test]
    fn ordnet_die_session_eines_favoriten_selbst_nicht_sich_selbst_zu() {
        let mut s = store();
        s.add("p1", &snap("s1"), None);
        assert!(s.link_resumed("p1", "D:/Mein Projekt/wt", "s1").is_none());
    }

    #[test]
    fn ubernimmt_gespeicherte_favoriten() {
        let mut s = store();
        s.add("p1", &snap("s1"), Some("N"));
        let copy = FavoriteStore::new(s.to_vec(), Clock::system());
        assert_eq!(copy.list("p1")[0].note, "N");
    }

    #[test]
    fn vergleicht_verzeichnisse_unter_windows_ohne_schreibweise() {
        let mut s = FavoriteStore::with_platform(Vec::new(), Clock::system(), "win32");
        s.add("p1", &snap("s1"), None);
        assert!(s.link_resumed("p1", "d:/mein projekt/WT", "neu").is_some());
    }
}
