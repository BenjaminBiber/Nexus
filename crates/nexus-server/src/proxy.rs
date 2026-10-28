//! Reverse-Proxy zum Node-Server: alles, was Rust noch nicht selbst beantwortet, geht
//! unverändert weiter – auch Ereignisströme (Server-Sent Events).
//!
//! Nichts wird gepuffert: der Körper der Anfrage geht als Strom hin, der Körper der Antwort
//! als Strom zurück. Der Client hat deshalb **kein** Gesamt-Zeitlimit, sonst bräche ein
//! langer Ereignisstrom mitten im Betrieb ab. Beim Beenden des Servers enden die Ströme am
//! gemeinsamen Abbruch-Signal.

use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::{header, HeaderMap, HeaderName, HeaderValue, Method, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use futures::StreamExt;
use http_body_util::BodyStream;
use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::client::legacy::Client;
use hyper_util::rt::TokioExecutor;
use nexus_core::{HarnessError, Result};
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

use crate::state::AppState;

/// Kopf, der sagt, wer geantwortet hat: `rust` für eigene Antworten, `node` für die des
/// Upstreams
pub const BACKEND_HEADER: &str = "x-nexus-backend";
/// Wert des Kopfes bei eigenen Antworten
pub const BACKEND_RUST: &str = "rust";
/// Wert des Kopfes bei weitergeleiteten Antworten
pub const BACKEND_NODE: &str = "node";

/// Köpfe, die nur für eine Verbindung gelten und deshalb nicht weitergereicht werden
const HOP_BY_HOP: [&str; 8] = [
    "connection",
    "keep-alive",
    "proxy-authenticate",
    "proxy-authorization",
    "te",
    "trailer",
    "transfer-encoding",
    "upgrade",
];

/// Der Node-Server, an den weitergeleitet wird
#[derive(Clone, Debug)]
pub struct Upstream {
    /// Adresse, wie sie angegeben wurde (`http://127.0.0.1:4711`)
    base: String,
    /// Host und Port des Upstreams (`127.0.0.1:4711`)
    authority: String,
    client: Client<HttpConnector, Body>,
}

impl Upstream {
    /// Upstream aus seiner Adresse; ohne Host ist die Adresse unbrauchbar
    pub fn parse(url: &str) -> Result<Self> {
        let uri: Uri = url.parse().map_err(|_| invalid(url))?;
        let authority = uri.authority().ok_or_else(|| invalid(url))?.to_string();
        let scheme = uri.scheme_str().unwrap_or("http");
        Ok(Self {
            base: format!("{scheme}://{authority}"),
            authority,
            // Ohne Gesamt-Zeitlimit: ein Ereignisstrom bleibt beliebig lange offen
            client: Client::builder(TokioExecutor::new()).build(HttpConnector::new()),
        })
    }

    /// Host und Port des Upstreams
    pub fn authority(&self) -> &str {
        &self.authority
    }

    /// Die Adresse des Upstreams, wie der Browser sie sehen würde
    pub fn base(&self) -> &str {
        &self.base
    }

    /// Leitet eine Anfrage weiter und gibt die Antwort als Strom zurück; `Err` nennt den
    /// Grund, warum der Upstream nicht geantwortet hat
    pub async fn forward(
        &self,
        request: Request,
        shutdown: &CancellationToken,
    ) -> std::result::Result<Response, String> {
        let (mut parts, body) = request.into_parts();
        let path_and_query = parts
            .uri
            .path_and_query()
            .map(|value| value.as_str())
            .unwrap_or("/");
        parts.uri = format!("{}{path_and_query}", self.base)
            .parse::<Uri>()
            .map_err(|error| error.to_string())?;
        self.rewrite(&mut parts.headers);
        let outgoing = Request::from_parts(parts, body);

        let response = self
            .client
            .request(outgoing)
            .await
            .map_err(|error| error.to_string())?;
        let (mut parts, incoming) = response.into_parts();
        strip_hop_by_hop(&mut parts.headers);
        parts.headers.insert(
            HeaderName::from_static(BACKEND_HEADER),
            HeaderValue::from_static(BACKEND_NODE),
        );
        // Der Strom endet spätestens, wenn der Server beendet wird – sonst wartet das
        // geordnete Beenden auf einen Ereignisstrom, der von selbst nie endet
        let frames = BodyStream::new(incoming).filter_map(|frame| async move {
            match frame {
                Ok(frame) => frame.into_data().ok().map(Ok),
                Err(error) => Some(Err(error)),
            }
        });
        let body = Body::from_stream(frames.take_until(shutdown.clone().cancelled_owned()));
        Ok(Response::from_parts(parts, body))
    }

    /// Reicht eine schon beantwortete Anfrage am Upstream nach, ohne auf ihn zu warten.
    ///
    /// Gebraucht nach `PUT /api/board-config`: der Node-Server hält seine eigene
    /// Board-Laufzeit (Dispatcher, noch nicht portierte Routen) und muss sie verwerfen. Die
    /// Antwort zählt nicht, ein Fehlschlag steht nur im Protokoll. Der Aufruf endet
    /// spätestens, wenn der Server beendet wird.
    pub fn replay(
        &self,
        method: Method,
        path: &str,
        body: Vec<u8>,
        log: crate::log::ServerLog,
        shutdown: &CancellationToken,
    ) {
        let client = self.client.clone();
        let authority = self.authority.clone();
        let uri = format!("{}{path}", self.base);
        let shutdown = shutdown.clone();
        let label = format!("{method} {path}");
        tokio::spawn(async move {
            let request = Request::builder()
                .method(method)
                .uri(&uri)
                .header(header::HOST, authority)
                .header(header::CONTENT_TYPE, crate::error::JSON_CONTENT_TYPE)
                // Der Schutz des Node-Servers verlangt den Pflichtkopf bei jedem Schreibzugriff
                .header("x-harness", "1")
                .body(Body::from(body));
            let request = match request {
                Ok(request) => request,
                Err(error) => return log.write(&format!("Nachreichen von {label}: {error}")),
            };
            let sent = tokio::select! {
                result = client.request(request) => result,
                _ = shutdown.cancelled() => return,
            };
            if let Err(error) = sent {
                log.write(&format!("Nachreichen von {label} fehlgeschlagen: {error}"));
            }
        });
    }

    /// Köpfe für den Upstream: Verbindungsköpfe weg, `host` und `origin` auf den Upstream –
    /// sonst weist dessen eigener Schutz die Anfrage ab
    fn rewrite(&self, headers: &mut HeaderMap) {
        strip_hop_by_hop(headers);
        if let Ok(value) = HeaderValue::from_str(&self.authority) {
            headers.insert(header::HOST, value);
        }
        if headers.contains_key(header::ORIGIN) {
            if let Ok(value) = HeaderValue::from_str(&self.base) {
                headers.insert(header::ORIGIN, value);
            }
        }
    }
}

/// Entfernt die Köpfe, die nur für eine Verbindung gelten
fn strip_hop_by_hop(headers: &mut HeaderMap) {
    for name in HOP_BY_HOP {
        headers.remove(name);
    }
}

/// Der Node-Server antwortet nicht
fn unavailable() -> Response {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        crate::error::json(&serde_json::json!({
            "code": "upstream-unavailable",
            "message": "Der Node-Server antwortet nicht.",
        })),
    )
        .into_response()
}

/// Die Adresse des Upstreams ist unbrauchbar
fn invalid(url: &str) -> nexus_core::Error {
    HarnessError::new(
        "upstream-invalid",
        format!("Die Adresse des Node-Servers „{url}“ ist ungültig."),
    )
    .into()
}

/// Rückfall des Routers: `/api`-Pfade gehen zum Upstream, alles andere an die statische
/// Auslieferung; ohne Upstream gibt es 404 als JSON
pub async fn proxy_or_static(State(state): State<Arc<AppState>>, request: Request) -> Response {
    match (&state.upstream, request.uri().path().starts_with("/api")) {
        (Some(upstream), true) => {
            let method = request.method().clone();
            let path = request.uri().path().to_string();
            match upstream.forward(request, &state.shutdown).await {
                Ok(response) => response,
                Err(reason) => {
                    state.log.write(&format!(
                        "Node-Server antwortet nicht ({reason}): {method} {path}"
                    ));
                    unavailable()
                }
            }
        }
        (_, true) => crate::error::not_found_response(),
        (_, false) => crate::routes::static_files::serve(state, request).await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn liest_adresse_und_authority_des_upstreams() {
        let upstream = Upstream::parse("http://127.0.0.1:4711").unwrap();
        assert_eq!(upstream.authority(), "127.0.0.1:4711");
        assert_eq!(upstream.base(), "http://127.0.0.1:4711");
        // Ein angehängter Pfad zählt nicht; weitergeleitet wird immer der Pfad der Anfrage
        let mit_pfad = Upstream::parse("http://localhost:1/abc").unwrap();
        assert_eq!(mit_pfad.base(), "http://localhost:1");
    }

    #[test]
    fn meldet_eine_unbrauchbare_adresse() {
        // Ein reiner Hostname (ohne Schema) ist brauchbar; ohne Host geht es nicht
        for url in ["", "/nur/pfad", "http://"] {
            let error = Upstream::parse(url).unwrap_err();
            assert_eq!(error.code(), Some("upstream-invalid"), "{url}");
        }
    }

    #[test]
    fn schreibt_host_und_herkunft_auf_den_upstream_um_und_raumt_verbindungskopfe_ab() {
        let upstream = Upstream::parse("http://127.0.0.1:4711").unwrap();
        let mut headers = HeaderMap::new();
        headers.insert(header::HOST, HeaderValue::from_static("127.0.0.1:4710"));
        headers.insert(
            header::ORIGIN,
            HeaderValue::from_static("http://localhost:4710"),
        );
        headers.insert(header::CONNECTION, HeaderValue::from_static("keep-alive"));
        headers.insert("x-harness", HeaderValue::from_static("1"));
        upstream.rewrite(&mut headers);

        assert_eq!(headers[header::HOST], "127.0.0.1:4711");
        assert_eq!(headers[header::ORIGIN], "http://127.0.0.1:4711");
        assert_eq!(headers["x-harness"], "1");
        assert!(!headers.contains_key(header::CONNECTION));
    }

    #[test]
    fn eine_anfrage_ohne_herkunft_bekommt_keine() {
        let upstream = Upstream::parse("http://127.0.0.1:4711").unwrap();
        let mut headers = HeaderMap::new();
        headers.insert(header::HOST, HeaderValue::from_static("127.0.0.1:4710"));
        upstream.rewrite(&mut headers);
        assert!(!headers.contains_key(header::ORIGIN));
    }
}
