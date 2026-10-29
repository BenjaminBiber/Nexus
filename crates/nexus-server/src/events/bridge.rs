//! Brücke zum Ereignisstrom des Node-Servers.
//!
//! Sessions (bis P4-9) und die Meldungen des Node-Dispatchers (bis Phase 5) entstehen noch
//! im Node-Prozess. Solange ein Upstream eingerichtet ist, öffnet jeder Rust-Strom deshalb
//! eine eigene Verbindung zum Node-Strom desselben Projekts und reicht genau die Ereignisse
//! weiter, die dort entstehen – `hello`, Herzschlag und alles, was Rust selbst schickt,
//! bleiben liegen.
//!
//! **Ohne** `dbBoard`/`dbProject`: sonst würde der Node-Server dieselbe Datenbank ein zweites
//! Mal abfragen, obwohl die Watcher in Rust laufen.

use std::collections::HashSet;
use std::time::Duration;

use axum::body::Body;
use axum::http::{header, Request};
use futures::StreamExt as _;
use http_body_util::BodyStream;
use tokio::sync::mpsc::UnboundedSender;
use tokio_util::sync::CancellationToken;

use crate::events::hub::LiveEvent;
use crate::log::ServerLog;
use crate::proxy::Upstream;

/// Ereignisse, die vom Node-Server kommen, solange sie dort entstehen
pub const BRIDGED_EVENTS: [&str; 2] = ["sessions", "notice"];

/// Wartezeit bis zum neuen Verbindungsversuch, wenn der Node-Strom abbricht
const RETRY: Duration = Duration::from_millis(2000);

/// Die Brücke eines Stroms: sie läuft, bis ihr Abbruch-Signal kommt (der Browser geht oder
/// der Server endet).
pub struct Bridge {
    cancel: CancellationToken,
}

impl Bridge {
    /// Öffnet die Verbindung zum Node-Strom des Projekts und reicht die Ereignisse an
    /// `sender` weiter. Bricht die Verbindung ab, wird sie nach zwei Sekunden neu aufgebaut.
    pub fn start(
        upstream: Upstream,
        project_id: &str,
        notify: bool,
        bridged: HashSet<&'static str>,
        sender: UnboundedSender<LiveEvent>,
        log: ServerLog,
        shutdown: &CancellationToken,
    ) -> Self {
        let cancel = shutdown.child_token();
        let task = cancel.clone();
        let path = format!(
            "/api/projects/{project_id}/events{}",
            if notify { "?notify=1" } else { "" }
        );
        tokio::spawn(async move {
            loop {
                if task.is_cancelled() || sender.is_closed() {
                    return;
                }
                if let Err(reason) = follow(&upstream, &path, &bridged, &sender, &task).await {
                    log.write(&format!("Ereignisse von {path} nicht erreichbar: {reason}"));
                }
                tokio::select! {
                    _ = tokio::time::sleep(RETRY) => {}
                    _ = task.cancelled() => return,
                }
            }
        });
        Self { cancel }
    }
}

impl Drop for Bridge {
    fn drop(&mut self) {
        // Geht der Browser, geht auch die Verbindung zum Node-Server
        self.cancel.cancel();
    }
}

/// Eine Verbindung zum Node-Strom: Zeilen lesen, bis sie endet
async fn follow(
    upstream: &Upstream,
    path: &str,
    bridged: &HashSet<&'static str>,
    sender: &UnboundedSender<LiveEvent>,
    cancel: &CancellationToken,
) -> std::result::Result<(), String> {
    let request = Request::builder()
        .method("GET")
        .uri(format!("{}{path}", upstream.base()))
        .header(header::HOST, upstream.authority())
        .header(header::ACCEPT, "text/event-stream")
        .body(Body::empty())
        .map_err(|error| error.to_string())?;
    let response = tokio::select! {
        result = upstream.send(request) => result?,
        _ = cancel.cancelled() => return Ok(()),
    };
    if !response.status().is_success() {
        return Err(format!("Status {}", response.status()));
    }
    let mut frames = BodyStream::new(response.into_body());
    let mut rest = String::new();
    loop {
        let frame = tokio::select! {
            frame = frames.next() => frame,
            _ = cancel.cancelled() => return Ok(()),
        };
        let Some(frame) = frame else { return Ok(()) };
        let frame = frame.map_err(|error| error.to_string())?;
        let Ok(data) = frame.into_data() else {
            continue;
        };
        rest.push_str(&String::from_utf8_lossy(&data));
        while let Some(at) = rest.find('\n') {
            let line: String = rest.drain(..=at).collect();
            if !deliver(line.trim_end_matches(['\r', '\n']), bridged, sender) {
                // Niemand hört mehr zu
                return Ok(());
            }
        }
    }
}

/// Reicht eine Zeile des Node-Stroms weiter; `false` heißt: der Strom hat keinen Empfänger
/// mehr. Kommentare (`: ping`), `hello` und alles, was nicht gebrückt wird, fallen weg.
fn deliver(
    line: &str,
    bridged: &HashSet<&'static str>,
    sender: &UnboundedSender<LiveEvent>,
) -> bool {
    let Some(payload) = line.strip_prefix("data:") else {
        return !sender.is_closed();
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(payload.trim()) else {
        return !sender.is_closed();
    };
    let kind = value.get("type").and_then(serde_json::Value::as_str);
    if !kind.is_some_and(|kind| bridged.contains(kind)) {
        return !sender.is_closed();
    }
    match serde_json::from_value::<LiveEvent>(value) {
        Ok(event) => sender.send(event).is_ok(),
        Err(_) => !sender.is_closed(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::notices::{BrowserNotice, ToastKind};

    /// Die Ereignisse, die der Standard brückt
    fn bridged() -> HashSet<&'static str> {
        BRIDGED_EVENTS.into_iter().collect()
    }

    #[test]
    fn reicht_nur_die_gebruckten_ereignisse_weiter() {
        let (sender, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let bridged = bridged();
        for line in [
            ": ping",
            "",
            "data: {\"type\":\"hello\"}",
            "data: {\"type\":\"dbboard\",\"boardId\":1}",
            "event: kaputt",
            "data: kein JSON",
            "data: {\"type\":\"sessions\"}",
        ] {
            assert!(deliver(line, &bridged, &sender), "{line}");
        }
        assert_eq!(rx.try_recv(), Ok(LiveEvent::Sessions));
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn eine_meldung_des_node_servers_kommt_unverandert_an() {
        let (sender, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let line = r#"data: {"type":"notice","kind":"wartet","tag":"attention","title":"T","body":"B","url":"/p/x/sessions"}"#;
        assert!(deliver(line, &bridged(), &sender));
        assert_eq!(
            rx.try_recv(),
            Ok(LiveEvent::Notice(BrowserNotice {
                kind: ToastKind::Wartet,
                tag: "attention".into(),
                title: "T".into(),
                body: "B".into(),
                url: "/p/x/sessions".into(),
            }))
        );
    }

    #[test]
    fn ohne_empfanger_endet_die_brucke() {
        let (sender, rx) = tokio::sync::mpsc::unbounded_channel();
        drop(rx);
        assert!(!deliver(
            "data: {\"type\":\"sessions\"}",
            &bridged(),
            &sender
        ));
        assert!(!deliver(": ping", &bridged(), &sender));
    }
}
