//! Fehler der HTTP-Schicht: Abbildung auf Status und JSON-Körper; portiert aus
//! `packages/server/src/errors/ErrorMapper.ts`.
//!
//! Alle JSON-Antworten tragen den Inhaltstyp `application/json; charset=utf-8` – genau wie
//! Fastify heute.

use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use nexus_core::{Error, HarnessError};
use serde::Serialize;

/// Inhaltstyp jeder JSON-Antwort (Fastify-Standard)
pub const JSON_CONTENT_TYPE: &str = "application/json; charset=utf-8";

/// Board-Datenbank fehlt, ist nicht erreichbar oder hat das falsche Schema: Dienst nicht
/// verfügbar
pub const UNAVAILABLE: [&str; 8] = [
    "board-config-missing",
    "board-config-broken",
    "board-db-unreachable",
    "board-db-overloaded",
    "board-db-storage",
    "board-schema-missing",
    "board-schema-outdated",
    "board-schema-newer",
];

/// Status und Körper einer Fehlerantwort
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MappedError {
    pub status: StatusCode,
    pub body: serde_json::Value,
}

/// Übersetzt Fehler in HTTP-Status und deutschsprachige Fehlermeldungen (`ErrorDto`).
#[derive(Debug, Clone, Copy, Default)]
pub struct ErrorMapper;

impl ErrorMapper {
    /// Erwartbarer Fehler → 400/403/404/409/422/503, alles andere → 500 ohne Interna
    pub fn map(error: &Error) -> MappedError {
        match error.harness() {
            Some(harness) => MappedError {
                status: Self::status_of(&harness.code),
                body: serde_json::to_value(harness).unwrap_or_else(|_| Self::internal_body()),
            },
            None => MappedError {
                status: StatusCode::INTERNAL_SERVER_ERROR,
                body: Self::internal_body(),
            },
        }
    }

    /// Status zum Fehlercode
    pub fn status_of(code: &str) -> StatusCode {
        if code.ends_with("-not-found") || code == "not-found" {
            return StatusCode::NOT_FOUND;
        }
        if code == "token-invalid" {
            return StatusCode::FORBIDDEN;
        }
        if code == "conflict"
            || code == "export-target-not-empty"
            || code == "doc-changed"
            || code == "setup-busy"
            || code == "plugin-not-bundled"
            // Code und Testnachricht brauchen einen laufenden Bot
            || code == "telegram-not-running"
        {
            return StatusCode::CONFLICT;
        }
        // Die Einstellung des Telegram-Bots ist falsch ausgefüllt (Token, Adresse, Uhrzeit)
        if code == "telegram-config-invalid" {
            return StatusCode::BAD_REQUEST;
        }
        // Zeilenbereich passt nicht zum Dokument: die Anfrage selbst ist falsch gebaut
        // Eine ungültige Theme-ID kann nie ein Theme treffen: die Adresse ist falsch gebaut
        if code == "doc-range-invalid" || code == "bad-request" || code == "theme-id-invalid" {
            return StatusCode::BAD_REQUEST;
        }
        // Spracherkennung ist nicht eingerichtet: nicht die Schuld der Anfrage, aber auch
        // nichts, was der Server reparieren kann
        if code == "speech-off" {
            return StatusCode::BAD_REQUEST;
        }
        // Der fremde Erkennungsdienst hat nicht oder falsch geantwortet
        if code == "speech-failed" {
            return StatusCode::BAD_GATEWAY;
        }
        if UNAVAILABLE.contains(&code) {
            return StatusCode::SERVICE_UNAVAILABLE;
        }
        StatusCode::UNPROCESSABLE_ENTITY
    }

    /// Körper eines unerwarteten Fehlers (auch bei einer Panik im Handler)
    pub fn internal_body() -> serde_json::Value {
        serde_json::json!({ "code": "internal", "message": "Interner Fehler im Server." })
    }
}

/// Ein Fehler, den die HTTP-Schicht als Antwort ausgeben kann
#[derive(Debug)]
pub struct ApiError(pub Error);

impl ApiError {
    /// Fehler mit Code und Meldung (wie `HarnessError` in TypeScript)
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self(HarnessError::new(code, message).into())
    }

    /// Die Anfrage selbst ist ungültig (Fastify: `FST_ERR_CTP_*` mit Status 400)
    pub fn bad_request() -> Self {
        Self::new("bad-request", "Die Anfrage ist ungültig.")
    }

    /// Diese Adresse gibt es nicht
    pub fn not_found() -> Self {
        Self::new("not-found", "Diese Adresse gibt es nicht.")
    }
}

impl From<Error> for ApiError {
    fn from(error: Error) -> Self {
        Self(error)
    }
}

impl From<HarnessError> for ApiError {
    fn from(error: HarnessError) -> Self {
        Self(error.into())
    }
}

impl From<nexus_platform::PlatformError> for ApiError {
    fn from(error: nexus_platform::PlatformError) -> Self {
        Self(HarnessError::from(error).into())
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let mapped = ErrorMapper::map(&self.0);
        (mapped.status, json(&mapped.body)).into_response()
    }
}

/// Ergebnis eines Handlers: Erfolg oder ein Fehler, den [`ErrorMapper`] abbildet
pub type ApiResult<T> = std::result::Result<T, ApiError>;

/// Baut eine JSON-Antwort mit dem Inhaltstyp von Fastify
pub fn json<T: Serialize>(value: &T) -> Response {
    let body = serde_json::to_vec(value).unwrap_or_else(|_| b"{}".to_vec());
    ([(header::CONTENT_TYPE, JSON_CONTENT_TYPE)], body).into_response()
}

/// 404 als JSON, wie der Rückfall in `StaticRoutes.ts`
pub fn not_found_response() -> Response {
    ApiError::not_found().into_response()
}

/// Antwort auf eine Panik im Handler: 500 ohne Interna (`tower_http::catch_panic`)
pub fn panic_response(_panic: Box<dyn std::any::Any + Send + 'static>) -> Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        json(&ErrorMapper::internal_body()),
    )
        .into_response()
}
