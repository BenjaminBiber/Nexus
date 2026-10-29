//! Offene Tabs, die Benachrichtigungen zeigen dürfen; portiert aus
//! `packages/server/src/notifications/BrowserNoticeHub.ts`.
//!
//! Solange ein Tab mit `notify=1` offen ist, zeigt er die Meldung statt des Windows-Toasts;
//! ein Klick bleibt dann im selben Tab. Die Meldungen selbst entstehen erst in Phase 5
//! (Dispatcher); hier steht nur die Weitergabe an die Ströme.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::UnboundedSender;

use crate::events::LiveEvent;

/// Anlass einer Meldung; wortgleich zu `ToastKind` in TypeScript
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ToastKind {
    Bereit,
    Wartet,
    Gesammelt,
    FrageNeu,
    Recherche,
}

/// Meldung, die ein offener Tab als Browser-Benachrichtigung zeigt; `url` ist ein Pfad der
/// Oberfläche. Der Typ `notice` kommt beim Senden aus [`LiveEvent`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BrowserNotice {
    pub kind: ToastKind,
    /// Gleiche `tag` ersetzen sich im Browser
    pub tag: String,
    pub title: String,
    pub body: String,
    pub url: String,
}

/// Ein angemeldeter Tab
struct Tab {
    id: u64,
    sender: UnboundedSender<LiveEvent>,
}

/// Die offenen Tabs mit `notify=1`, projektübergreifend.
#[derive(Default)]
pub struct BrowserNoticeHub {
    tabs: Mutex<Vec<Tab>>,
    next_id: AtomicU64,
}

impl BrowserNoticeHub {
    /// Ohne offenen Tab
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// Meldet einen Tab an; die Anmeldung endet, wenn das Ergebnis fällt
    pub fn attach(self: &Arc<Self>, sender: UnboundedSender<LiveEvent>) -> AttachedTab {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        self.tabs
            .lock()
            .expect("Sperre der Tabs")
            .push(Tab { id, sender });
        AttachedTab {
            hub: self.clone(),
            id,
        }
    }

    /// Schickt die Meldung an alle Tabs; `false`: kein Tab offen
    pub fn deliver(&self, notice: BrowserNotice) -> bool {
        let tabs = self.tabs.lock().expect("Sperre der Tabs");
        if tabs.is_empty() {
            return false;
        }
        for tab in tabs.iter() {
            let _ = tab.sender.send(LiveEvent::Notice(notice.clone()));
        }
        true
    }

    /// Zahl der offenen Tabs
    pub fn tabs(&self) -> usize {
        self.tabs.lock().expect("Sperre der Tabs").len()
    }

    /// Meldet alle Tabs ab (Ende des Servers)
    pub fn close(&self) {
        self.tabs.lock().expect("Sperre der Tabs").clear();
    }

    /// Meldet einen Tab ab
    fn detach(&self, id: u64) {
        self.tabs
            .lock()
            .expect("Sperre der Tabs")
            .retain(|tab| tab.id != id);
    }
}

/// Die Anmeldung eines Tabs; sie endet, wenn sie fällt
pub struct AttachedTab {
    hub: Arc<BrowserNoticeHub>,
    id: u64,
}

impl Drop for AttachedTab {
    fn drop(&mut self) {
        self.hub.detach(self.id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Eine Meldung, wie der Dispatcher sie schicken würde
    fn notice() -> BrowserNotice {
        BrowserNotice {
            kind: ToastKind::Wartet,
            tag: "attention".into(),
            title: "T".into(),
            body: "B".into(),
            url: "/p/x/sessions".into(),
        }
    }

    #[test]
    fn die_json_form_einer_meldung_ist_der_vertrag() {
        assert_eq!(
            serde_json::to_string(&LiveEvent::Notice(notice())).unwrap(),
            r#"{"type":"notice","kind":"wartet","tag":"attention","title":"T","body":"B","url":"/p/x/sessions"}"#
        );
        assert_eq!(
            serde_json::to_string(&ToastKind::FrageNeu).unwrap(),
            r#""frage-neu""#
        );
    }

    #[tokio::test]
    async fn ohne_offenen_tab_wird_nichts_zugestellt() {
        let hub = BrowserNoticeHub::new();
        assert!(!hub.deliver(notice()));
    }

    #[tokio::test]
    async fn jeder_offene_tab_bekommt_die_meldung_und_nach_dem_abmelden_keiner_mehr() {
        let hub = BrowserNoticeHub::new();
        let (eins, mut eins_rx) = tokio::sync::mpsc::unbounded_channel();
        let (zwei, mut zwei_rx) = tokio::sync::mpsc::unbounded_channel();
        let erster = hub.attach(eins);
        let zweiter = hub.attach(zwei);
        assert!(hub.deliver(notice()));
        assert_eq!(eins_rx.recv().await, Some(LiveEvent::Notice(notice())));
        assert_eq!(zwei_rx.recv().await, Some(LiveEvent::Notice(notice())));

        drop(erster);
        assert_eq!(hub.tabs(), 1);
        assert!(hub.deliver(notice()));
        assert!(eins_rx.try_recv().is_err());
        drop(zweiter);
        assert!(!hub.deliver(notice()));
    }
}
