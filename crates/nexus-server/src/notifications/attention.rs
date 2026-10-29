//! Merkt sich die Sessions, die auf den Nutzer warten; portiert aus
//! `packages/server/src/notifications/AttentionTracker.ts`.

use indexmap::IndexMap;
use nexus_core::sessions::session::{DisplayState, SessionView};

/// Art der Meldung einer einzelnen Session
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttentionKind {
    /// Die Session ist fertig
    Bereit,
    /// Die Session braucht eine Antwort
    Wartet,
}

/// Eine Session, die auf den Nutzer wartet
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingSession {
    pub kind: AttentionKind,
    /// Ticketnummer, sonst Sessiontitel, sonst Projektname
    pub label: String,
}

/// Merkt sich, welche Sessions auf den Nutzer warten, und erkennt neue Ereignisse
/// (bereit, wartet). Die Reihenfolge ist die des ersten Auftretens, wie bei einer `Map` in
/// JavaScript.
#[derive(Debug, Default)]
pub struct AttentionTracker {
    pending: IndexMap<String, PendingSession>,
}

impl AttentionTracker {
    /// Ohne gemerkte Sessions
    pub fn new() -> Self {
        Self::default()
    }

    /// Prüft den Zustandswechsel einer Session; liefert die Art der Meldung oder `None`
    pub fn observe(
        &mut self,
        before: Option<DisplayState>,
        view: &SessionView,
        label: &str,
    ) -> Option<AttentionKind> {
        let key = format!("{}:{}", view.project_id, view.id);
        match view.display_state {
            DisplayState::Running | DisplayState::Ended | DisplayState::Stale => {
                self.pending.shift_remove(&key);
                None
            }
            DisplayState::Waiting if before != Some(DisplayState::Waiting) => {
                Some(self.add(key, AttentionKind::Wartet, label))
            }
            DisplayState::Idle
                if matches!(before, Some(DisplayState::Running | DisplayState::Waiting)) =>
            {
                Some(self.add(key, AttentionKind::Bereit, label))
            }
            _ => None,
        }
    }

    /// Alle wartenden Sessions
    pub fn list(&self) -> Vec<PendingSession> {
        self.pending.values().cloned().collect()
    }

    /// Nimmt die Session auf und gibt die Art zurück
    fn add(&mut self, key: String, kind: AttentionKind, label: &str) -> AttentionKind {
        self.pending.insert(
            key,
            PendingSession {
                kind,
                label: label.to_string(),
            },
        );
        kind
    }
}
