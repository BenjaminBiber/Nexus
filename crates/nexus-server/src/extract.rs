//! Extraktoren mit dem Verhalten von Fastify: JSON-Körper und Abfrageteil.
//!
//! Fastify verhält sich an zwei Stellen anders als die Standard-Extraktoren von axum, und der
//! Vertrag hängt daran:
//!
//! * Ein leerer oder kaputter JSON-Körper ergibt 400 `bad-request` („Die Anfrage ist
//!   ungültig.“, Fastify: `FST_ERR_CTP_EMPTY_JSON_BODY`), kein 422.
//! * Ein wiederholter Abfrage-Parameter kommt als Liste an, nicht nur mit seinem letzten Wert.

use std::collections::BTreeMap;

use axum::body::Bytes;
use axum::extract::{FromRequest, FromRequestParts, Request};
use axum::http::request::Parts;
use axum::http::Method;

use crate::error::ApiError;

/// Größe, ab der ein Körper abgelehnt wird (Fastify-Standard: 1 MiB)
pub const MAX_BODY_BYTES: usize = 1024 * 1024;

/// Der Körper einer Anfrage als JSON-Wert; fehlt er bei `GET`/`HEAD`, ist er `null`
#[derive(Debug, Clone, PartialEq)]
pub struct JsonBody(pub serde_json::Value);

impl JsonBody {
    /// Der Wert selbst
    pub fn into_inner(self) -> serde_json::Value {
        self.0
    }
}

impl<S: Send + Sync> FromRequest<S> for JsonBody {
    type Rejection = ApiError;

    async fn from_request(request: Request, state: &S) -> Result<Self, Self::Rejection> {
        let read = request.method() != Method::GET && request.method() != Method::HEAD;
        let json_type = request
            .headers()
            .get(axum::http::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| value.to_lowercase().starts_with("application/json"));
        if !read {
            return Ok(JsonBody(serde_json::Value::Null));
        }
        let bytes = Bytes::from_request(request, state)
            .await
            .map_err(|_| ApiError::bad_request())?;
        // Fastify meldet einen zu großen Körper mit 413, den der ErrorMapper als 500 ausgibt
        if bytes.len() > MAX_BODY_BYTES {
            return Err(ApiError::from(nexus_core::Error::unexpected_message(
                "Der Körper der Anfrage ist zu groß.",
            )));
        }
        if !json_type {
            // Ohne JSON als Inhaltstyp liest Fastify den Körper nicht; der Schutz lässt
            // schreibende Anfragen ohne JSON ohnehin nicht durch
            return Ok(JsonBody(serde_json::Value::Null));
        }
        serde_json::from_slice(&bytes)
            .map(JsonBody)
            .map_err(|_| ApiError::bad_request())
    }
}

/// Werte eines Abfrage-Parameters: fehlt, einmal oder mehrfach
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueryValue {
    Missing,
    One(String),
    Many(Vec<String>),
}

impl QueryValue {
    /// Der erste Wert; `None`, wenn der Parameter fehlt
    pub fn first(&self) -> Option<&str> {
        match self {
            QueryValue::Missing => None,
            QueryValue::One(value) => Some(value),
            QueryValue::Many(values) => values.first().map(String::as_str),
        }
    }

    /// Alle Werte in der Reihenfolge der Adresse
    pub fn all(&self) -> Vec<&str> {
        match self {
            QueryValue::Missing => Vec::new(),
            QueryValue::One(value) => vec![value.as_str()],
            QueryValue::Many(values) => values.iter().map(String::as_str).collect(),
        }
    }
}

/// Der Abfrageteil der Adresse; wiederholte Parameter bleiben als Liste erhalten
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct QueryMap(BTreeMap<String, QueryValue>);

impl QueryMap {
    /// Liest den Abfrageteil einer Adresse (ohne `?`)
    pub fn parse(query: &str) -> Self {
        let mut map: BTreeMap<String, QueryValue> = BTreeMap::new();
        for pair in query.split('&').filter(|pair| !pair.is_empty()) {
            let (name, value) = match pair.split_once('=') {
                Some((name, value)) => (name, value),
                None => (pair, ""),
            };
            let name = decode(name);
            let value = decode(value);
            match map.remove(&name) {
                None => map.insert(name, QueryValue::One(value)),
                Some(QueryValue::One(first)) => {
                    map.insert(name, QueryValue::Many(vec![first, value]))
                }
                Some(QueryValue::Many(mut values)) => {
                    values.push(value);
                    map.insert(name, QueryValue::Many(values))
                }
                Some(QueryValue::Missing) => map.insert(name, QueryValue::One(value)),
            };
        }
        Self(map)
    }

    /// Die Werte eines Parameters
    pub fn get(&self, name: &str) -> QueryValue {
        self.0.get(name).cloned().unwrap_or(QueryValue::Missing)
    }

    /// Der erste Wert eines Parameters
    pub fn first(&self, name: &str) -> Option<&str> {
        self.0.get(name).and_then(QueryValue::first)
    }

    /// Alle Werte eines Parameters
    pub fn all(&self, name: &str) -> Vec<&str> {
        self.0.get(name).map(QueryValue::all).unwrap_or_default()
    }

    /// Ist ein Parameter gesetzt (auch leer)?
    pub fn has(&self, name: &str) -> bool {
        self.0.contains_key(name)
    }
}

impl<S: Send + Sync> FromRequestParts<S> for QueryMap {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        Ok(QueryMap::parse(parts.uri.query().unwrap_or_default()))
    }
}

/// Prozent- und Plus-Kodierung eines Parameters auflösen (wie `URLSearchParams` in Node)
fn decode(raw: &str) -> String {
    let bytes = raw.replace('+', " ").into_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while at < bytes.len() {
        let byte = bytes[at];
        let hex = (at + 2 < bytes.len()).then(|| &bytes[at + 1..at + 3]);
        match (byte, hex.and_then(from_hex)) {
            (b'%', Some(value)) => {
                out.push(value);
                at += 3;
            }
            _ => {
                out.push(byte);
                at += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Zwei Hex-Ziffern als Byte
fn from_hex(pair: &[u8]) -> Option<u8> {
    let digit = |byte: u8| char::from(byte).to_digit(16).map(|value| value as u8);
    Some(digit(pair[0])? * 16 + digit(pair[1])?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn behalt_jeden_wert_eines_wiederholten_parameters() {
        let query = QueryMap::parse("label=a&label=b&label=c&board=15");
        assert_eq!(query.all("label"), ["a", "b", "c"]);
        assert_eq!(query.first("label"), Some("a"));
        assert_eq!(query.get("board"), QueryValue::One("15".into()));
        assert_eq!(query.get("fehlt"), QueryValue::Missing);
        assert_eq!(query.all("fehlt"), Vec::<&str>::new());
        assert!(query.has("board"));
        assert!(!query.has("fehlt"));
    }

    #[test]
    fn liest_leere_werte_und_kodierte_zeichen() {
        let query = QueryMap::parse("q=&name=Mein+Projekt+%C3%A4&flag");
        assert_eq!(query.first("q"), Some(""));
        assert!(query.has("q"));
        assert_eq!(query.first("name"), Some("Mein Projekt ä"));
        assert_eq!(query.first("flag"), Some(""));
        assert_eq!(QueryMap::parse(""), QueryMap::default());
    }

    #[test]
    fn eine_unvollstandige_kodierung_bleibt_wie_sie_ist() {
        let query = QueryMap::parse("a=100%&b=%zz");
        assert_eq!(query.first("a"), Some("100%"));
        assert_eq!(query.first("b"), Some("%zz"));
    }
}
