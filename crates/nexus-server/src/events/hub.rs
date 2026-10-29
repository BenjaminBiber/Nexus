//! Die Ereignisse des Stroms und ihre Verteilung je Projekt; portiert aus
//! `packages/server/src/events/EventHub.ts`.
//!
//! Ein Ereignis geht als `data: <JSON>\n\n` über die Leitung; die JSON-Form ist Teil des
//! Vertrags und steht deshalb an den Varianten von [`LiveEvent`].

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::UnboundedSender;

use crate::notices::BrowserNotice;

/// Ein Ereignis des Stroms. Die Namen der Varianten und ihrer Felder sind der Vertrag:
/// `{"type":"hello"}`, `{"type":"dbboard","boardId":1}` …
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum LiveEvent {
    /// Erstes Ereignis jedes Stroms
    #[serde(rename = "hello")]
    Hello,
    /// Die Sessions eines Projekts haben sich geändert
    #[serde(rename = "sessions")]
    Sessions,
    /// Das beobachtete DB-Board hat sich geändert
    #[serde(rename = "dbboard", rename_all = "camelCase")]
    DbBoard { board_id: i32 },
    /// Die Fragen des beobachteten Board-Projekts haben sich geändert
    #[serde(rename = "dbquestions", rename_all = "camelCase")]
    DbQuestions { project_id: i32 },
    /// Die Superpowers-Dokumente dieses Projekts haben sich geändert
    #[serde(rename = "docs-changed", rename_all = "camelCase")]
    DocsChanged { project_id: String },
    /// Meldung für einen Tab mit `notify=1`
    #[serde(rename = "notice")]
    Notice(BrowserNotice),
}

impl LiveEvent {
    /// Das Ereignis als Stück des Stroms (`data: <JSON>\n\n`)
    pub fn frame(&self) -> String {
        format!(
            "data: {}\n\n",
            serde_json::to_string(self).unwrap_or_else(|_| "{}".into())
        )
    }
}

/// Ein angemeldeter Empfänger
struct Listener {
    id: u64,
    sender: UnboundedSender<LiveEvent>,
}

/// Verteilt Änderungsmeldungen je Projekt an alle offenen Ereignisströme.
#[derive(Default)]
pub struct EventHub {
    channels: Mutex<HashMap<String, Vec<Listener>>>,
    next_id: AtomicU64,
}

impl EventHub {
    /// Ein leerer Verteiler
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// Meldet einen Empfänger an; die Anmeldung endet, wenn das Ergebnis fällt
    pub fn subscribe(
        self: &Arc<Self>,
        project_id: &str,
        sender: UnboundedSender<LiveEvent>,
    ) -> Subscription {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        self.channels
            .lock()
            .expect("Sperre des Verteilers")
            .entry(project_id.to_string())
            .or_default()
            .push(Listener { id, sender });
        Subscription {
            hub: self.clone(),
            project_id: project_id.to_string(),
            id,
        }
    }

    /// Sendet ein Ereignis an alle Empfänger eines Projekts (ohne Empfänger geschieht nichts)
    pub fn publish(&self, project_id: &str, event: LiveEvent) {
        let channels = self.channels.lock().expect("Sperre des Verteilers");
        let Some(listeners) = channels.get(project_id) else {
            return;
        };
        for listener in listeners {
            // Ein geschlossener Empfänger meldet sich gleich selbst ab
            let _ = listener.sender.send(event.clone());
        }
    }

    /// Anzahl der Empfänger eines Projekts
    pub fn subscribers(&self, project_id: &str) -> usize {
        self.channels
            .lock()
            .expect("Sperre des Verteilers")
            .get(project_id)
            .map(Vec::len)
            .unwrap_or(0)
    }

    /// Empfänger über alle Projekte (Nachweis, dass beim Beenden nichts offen bleibt)
    pub fn subscribers_total(&self) -> usize {
        self.channels
            .lock()
            .expect("Sperre des Verteilers")
            .values()
            .map(Vec::len)
            .sum()
    }

    /// Meldet alle Empfänger ab
    pub fn close(&self) {
        self.channels.lock().expect("Sperre des Verteilers").clear();
    }

    /// Meldet einen Empfänger ab; das letzte Kanalende nimmt den Kanal mit
    fn unsubscribe(&self, project_id: &str, id: u64) {
        let mut channels = self.channels.lock().expect("Sperre des Verteilers");
        let Some(listeners) = channels.get_mut(project_id) else {
            return;
        };
        listeners.retain(|listener| listener.id != id);
        if listeners.is_empty() {
            channels.remove(project_id);
        }
    }
}

/// Die Anmeldung eines Empfängers; sie endet, wenn sie fällt (der Browser geht, der Server
/// endet)
pub struct Subscription {
    hub: Arc<EventHub>,
    project_id: String,
    id: u64,
}

impl Drop for Subscription {
    fn drop(&mut self) {
        self.hub.unsubscribe(&self.project_id, self.id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Empfänger mit seinem Kanal
    fn listener() -> (
        UnboundedSender<LiveEvent>,
        tokio::sync::mpsc::UnboundedReceiver<LiveEvent>,
    ) {
        tokio::sync::mpsc::unbounded_channel()
    }

    #[test]
    fn die_json_form_jedes_ereignisses_ist_der_vertrag() {
        assert_eq!(LiveEvent::Hello.frame(), "data: {\"type\":\"hello\"}\n\n");
        assert_eq!(
            serde_json::to_string(&LiveEvent::Sessions).unwrap(),
            r#"{"type":"sessions"}"#
        );
        assert_eq!(
            serde_json::to_string(&LiveEvent::DbBoard { board_id: 1 }).unwrap(),
            r#"{"type":"dbboard","boardId":1}"#
        );
        assert_eq!(
            serde_json::to_string(&LiveEvent::DbQuestions { project_id: 7 }).unwrap(),
            r#"{"type":"dbquestions","projectId":7}"#
        );
        assert_eq!(
            serde_json::to_string(&LiveEvent::DocsChanged {
                project_id: "p1".into()
            })
            .unwrap(),
            r#"{"type":"docs-changed","projectId":"p1"}"#
        );
    }

    #[tokio::test]
    async fn sendet_an_alle_empfanger_eines_projekts_und_an_fremde_nicht() {
        let hub = EventHub::new();
        let (eins, mut eins_rx) = listener();
        let (zwei, mut zwei_rx) = listener();
        let (fremd, mut fremd_rx) = listener();
        let _a = hub.subscribe("p1", eins);
        let _b = hub.subscribe("p1", zwei);
        let _c = hub.subscribe("p2", fremd);
        assert_eq!(hub.subscribers("p1"), 2);

        hub.publish("p1", LiveEvent::Sessions);
        assert_eq!(eins_rx.recv().await, Some(LiveEvent::Sessions));
        assert_eq!(zwei_rx.recv().await, Some(LiveEvent::Sessions));
        assert!(fremd_rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn ohne_empfanger_geschieht_nichts() {
        let hub = EventHub::new();
        hub.publish("p1", LiveEvent::Sessions);
        assert_eq!(hub.subscribers("p1"), 0);
    }

    #[tokio::test]
    async fn eine_gefallene_anmeldung_bekommt_nichts_mehr() {
        let hub = EventHub::new();
        let (sender, mut empfang) = listener();
        let anmeldung = hub.subscribe("p1", sender);
        drop(anmeldung);
        assert_eq!(hub.subscribers("p1"), 0);
        hub.publish("p1", LiveEvent::Sessions);
        assert!(empfang.try_recv().is_err());
    }

    #[tokio::test]
    async fn close_meldet_alle_ab() {
        let hub = EventHub::new();
        let (sender, _empfang) = listener();
        let _anmeldung = hub.subscribe("p1", sender);
        hub.close();
        assert_eq!(hub.subscribers("p1"), 0);
    }
}
