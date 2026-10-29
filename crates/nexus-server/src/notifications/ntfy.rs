//! Push-Meldungen über ntfy; portiert aus
//! `packages/server/src/notifications/NtfySender.ts`.
//!
//! Veröffentlicht wird als JSON an den ntfy-Server (https://ntfy.sh oder einen eigenen).
//! Der Abruf steckt hinter [`NtfyFetch`], damit Tests und der Vertragslauf nie wirklich
//! senden.

use async_trait::async_trait;
use serde::Serialize;

use crate::notifications::model::{ToastKind, ToastModel};

/// Ziel der Push-Meldung: ntfy-Server, Thema und optionales Zugriffstoken
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NtfyTarget {
    pub server: String,
    pub topic: String,
    pub token: String,
}

/// Die Antwort des ntfy-Servers, soweit die Meldung sie braucht
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NtfyResponse {
    pub ok: bool,
    pub status: u16,
    pub text: String,
}

/// Schickt eine HTTP-Anfrage; im Test ersetzbar
#[async_trait]
pub trait NtfyFetch: Send + Sync {
    /// `POST` mit Kopfzeilen und Körper; `Err` = der Server war nicht erreichbar
    async fn post(
        &self,
        url: &str,
        headers: &[(String, String)],
        body: &str,
    ) -> Result<NtfyResponse, String>;
}

/// Der Körper der Veröffentlichung, Feld für Feld wie in TypeScript
#[derive(Debug, Serialize)]
struct NtfyBody<'a> {
    topic: &'a str,
    title: &'a str,
    message: String,
    tags: [&'static str; 1],
    priority: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    click: Option<&'a str>,
}

/// Sendet eine Meldung als Push über ntfy.
pub struct NtfySender {
    fetch: Box<dyn NtfyFetch>,
}

impl Default for NtfySender {
    fn default() -> Self {
        Self::new(Box::new(ReqwestNtfyFetch::default()))
    }
}

impl NtfySender {
    /// Sender mit eigenem Abruf (Tests setzen eine Attrappe ein)
    pub fn new(fetch: Box<dyn NtfyFetch>) -> Self {
        Self { fetch }
    }

    /// Sendet die Meldung; Fehler als Text, sonst `None`. Wartende Sessions und neue Fragen
    /// kommen mit hoher Priorität.
    pub async fn send(&self, target: &NtfyTarget, model: &ToastModel) -> Option<String> {
        let mut headers = vec![("Content-Type".to_string(), "application/json".to_string())];
        if !target.token.is_empty() {
            headers.push((
                "Authorization".to_string(),
                format!("Bearer {}", target.token),
            ));
        }
        let body = NtfyBody {
            topic: &target.topic,
            title: &model.title,
            message: [model.line1.as_str(), model.line2.as_str()]
                .into_iter()
                .filter(|line| !line.is_empty())
                .collect::<Vec<_>>()
                .join("\n"),
            tags: [tag_of(model.kind)],
            priority: match model.kind {
                ToastKind::Wartet | ToastKind::FrageNeu => 4,
                _ => 3,
            },
            click: reachable(&model.open_url).then_some(model.open_url.as_str()),
        };
        let json = serde_json::to_string(&body).unwrap_or_default();
        let url = target.server.trim_end_matches('/');
        match self.fetch.post(url, &headers, &json).await {
            Ok(response) if response.ok => None,
            Ok(response) => {
                let detail = response.text.trim();
                let detail = match detail.is_empty() {
                    true => String::new(),
                    false => format!(": {detail}"),
                };
                Some(format!("ntfy antwortete mit {}{detail}.", response.status))
            }
            Err(error) => Some(format!("ntfy nicht erreichbar: {error}")),
        }
    }
}

/// Emoji-Tag je Art der Meldung (ntfy zeigt ihn vor dem Titel)
fn tag_of(kind: ToastKind) -> &'static str {
    match kind {
        ToastKind::Bereit => "white_check_mark",
        ToastKind::Wartet => "raising_hand",
        ToastKind::Gesammelt => "bell",
        ToastKind::FrageNeu => "grey_question",
        ToastKind::Recherche => "mag",
    }
}

/// Adressen auf dem eigenen Rechner (127.0.0.1, localhost) öffnen auf dem Handy nichts; sie
/// bleiben ohne Klick-Ziel
fn reachable(url: &str) -> bool {
    let Ok(parsed) = reqwest::Url::parse(url) else {
        return false;
    };
    // `Url::host_str` liefert IPv6-Adressen ohne Klammern, `URL.hostname` in JavaScript mit
    let host = parsed.host_str().unwrap_or_default();
    !matches!(
        host,
        "localhost" | "127.0.0.1" | "[::1]" | "::1" | "0.0.0.0"
    )
}

/// Der Abruf des Betriebs: ein HTTP-Client mit rustls
pub struct ReqwestNtfyFetch {
    client: reqwest::Client,
}

impl Default for ReqwestNtfyFetch {
    fn default() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl NtfyFetch for ReqwestNtfyFetch {
    async fn post(
        &self,
        url: &str,
        headers: &[(String, String)],
        body: &str,
    ) -> Result<NtfyResponse, String> {
        let mut request = self.client.post(url);
        for (name, value) in headers {
            request = request.header(name, value);
        }
        let response = request
            .body(body.to_string())
            .send()
            .await
            .map_err(|error| error.to_string())?;
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        Ok(NtfyResponse {
            ok: status.is_success(),
            status: status.as_u16(),
            text,
        })
    }
}
