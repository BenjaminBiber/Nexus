//! Asset-Proxy für Themes (D7): fremde Ressourcen lädt ein Theme nur über den Server.
//!
//! Sicherheitsregeln:
//! - Nur `http`/`https` ohne Zugangsdaten, nur Hosts aus `permissions.network` des Manifests –
//!   auch jede Weiterleitung wird erneut geprüft (höchstens [`MAX_REDIRECTS`]).
//! - Größenlimit: zu große Antworten brechen schon beim Streamen ab.
//! - Optionale sha256-Prüfung; bei Abweichung wird nichts gecacht.
//! - Cache nur je Theme unter `<theme>/cache/`, atomar geschrieben (erst `.tmp`, dann
//!   `rename`). Vom Browser wird nichts (Cookies, Köpfe) an den Upstream weitergereicht.

use std::path::{Path, PathBuf};
use std::time::Duration;

use nexus_core::themes::host_allowed;
use reqwest::{header, Url};
use sha2::{Digest, Sha256};

use crate::error::ApiError;

/// Standardlimit einer Antwort: 10 MiB
pub const DEFAULT_MAX_BYTES: u64 = 10 * 1024 * 1024;
/// Höchstzahl an Weiterleitungen
pub const MAX_REDIRECTS: usize = 5;
/// Inhaltstyp, wenn der Upstream keinen nennt
const FALLBACK_TYPE: &str = "application/octet-stream";

/// Ergebnis eines Abrufs
#[derive(Debug, Clone)]
pub struct Fetched {
    pub bytes: Vec<u8>,
    pub content_type: String,
    pub from_cache: bool,
}

/// Warum ein Abruf scheitert
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProxyError {
    BadUrl(String),
    Forbidden(String),
    TooLarge,
    ChecksumMismatch,
    Upstream(String),
}

impl From<ProxyError> for ApiError {
    fn from(error: ProxyError) -> Self {
        match error {
            ProxyError::BadUrl(message) => ApiError::new("bad-request", message),
            ProxyError::Forbidden(host) => ApiError::new(
                "theme_fetch_forbidden",
                format!("Host „{host}“ ist für dieses Theme nicht freigegeben."),
            ),
            ProxyError::TooLarge => ApiError::new(
                "theme_fetch_too_large",
                "Die Ressource ist größer als erlaubt.",
            ),
            ProxyError::ChecksumMismatch => ApiError::new(
                "theme_fetch_checksum",
                "Die Prüfsumme der Ressource passt nicht.",
            ),
            ProxyError::Upstream(message) => ApiError::new("theme_fetch_upstream", message),
        }
    }
}

/// Lädt und cacht fremde Ressourcen für Themes
#[derive(Debug, Clone)]
pub struct ThemeProxy {
    client: reqwest::Client,
    max_bytes: u64,
}

impl ThemeProxy {
    /// Proxy mit Limit je Antwort; Weiterleitungen folgt er selbst, damit jede geprüft wird
    pub fn new(max_bytes: u64) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap_or_default();
        Self { client, max_bytes }
    }

    /// Liefert die Ressource aus dem Cache des Themes oder lädt sie vom Upstream
    pub async fn fetch(
        &self,
        theme_dir: &Path,
        network: &[String],
        url: &str,
        sha256: Option<&str>,
    ) -> Result<Fetched, ProxyError> {
        let url = check_url(network, url)?;
        let expected = match sha256 {
            Some(sum) if sum.len() == 64 && sum.chars().all(|c| c.is_ascii_hexdigit()) => {
                Some(sum.to_ascii_lowercase())
            }
            Some(_) => {
                return Err(ProxyError::BadUrl(
                    "sha256 muss aus 64 Hex-Zeichen bestehen.".into(),
                ))
            }
            None => None,
        };
        let cache = theme_dir.join("cache");
        let key = hex(&Sha256::digest(url.as_str().as_bytes()));
        let bin = cache.join(format!("{key}.bin"));
        let typ = cache.join(format!("{key}.type"));

        if let Ok(bytes) = tokio::fs::read(&bin).await {
            if expected.as_deref().is_none_or(|sum| sum == sha_hex(&bytes)) {
                let content_type = tokio::fs::read_to_string(&typ)
                    .await
                    .unwrap_or_else(|_| FALLBACK_TYPE.into());
                return Ok(Fetched {
                    bytes,
                    content_type,
                    from_cache: true,
                });
            }
            // Veralteter Eintrag mit falscher Prüfsumme: verwerfen und neu laden
            let _ = tokio::fs::remove_file(&bin).await;
            let _ = tokio::fs::remove_file(&typ).await;
        }

        let (bytes, content_type) = self.download(network, url).await?;
        if expected.is_some_and(|sum| sum != sha_hex(&bytes)) {
            return Err(ProxyError::ChecksumMismatch);
        }
        store(&cache, &key, &bin, &typ, &bytes, &content_type)
            .await
            .map_err(|e| ProxyError::Upstream(format!("Cache nicht beschreibbar: {e}")))?;
        Ok(Fetched {
            bytes,
            content_type,
            from_cache: false,
        })
    }

    /// Lädt mit geprüften Weiterleitungen und Größenlimit
    async fn download(
        &self,
        network: &[String],
        mut url: Url,
    ) -> Result<(Vec<u8>, String), ProxyError> {
        let mut redirects = 0;
        let mut response =
            loop {
                let response =
                    self.client.get(url.clone()).send().await.map_err(|e| {
                        ProxyError::Upstream(format!("Upstream nicht erreichbar: {e}"))
                    })?;
                if !response.status().is_redirection() {
                    break response;
                }
                redirects += 1;
                if redirects > MAX_REDIRECTS {
                    return Err(ProxyError::Upstream("Zu viele Weiterleitungen.".into()));
                }
                let location = response
                    .headers()
                    .get(header::LOCATION)
                    .and_then(|value| value.to_str().ok())
                    .ok_or_else(|| ProxyError::Upstream("Weiterleitung ohne Ziel.".into()))?;
                let next = url.join(location).map_err(|_| {
                    ProxyError::Upstream("Weiterleitung mit ungültigem Ziel.".into())
                })?;
                url = check_url(network, next.as_str())?;
            };
        if !response.status().is_success() {
            return Err(ProxyError::Upstream(format!(
                "Upstream antwortet mit {}.",
                response.status()
            )));
        }
        if response
            .content_length()
            .is_some_and(|len| len > self.max_bytes)
        {
            return Err(ProxyError::TooLarge);
        }
        let content_type = response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or(FALLBACK_TYPE)
            .to_string();
        let mut bytes = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|e| ProxyError::Upstream(format!("Upstream bricht ab: {e}")))?
        {
            if (bytes.len() + chunk.len()) as u64 > self.max_bytes {
                return Err(ProxyError::TooLarge);
            }
            bytes.extend_from_slice(&chunk);
        }
        Ok((bytes, content_type))
    }
}

/// Prüft Schema, Zugangsdaten und Host-Freigabe
fn check_url(network: &[String], url: &str) -> Result<Url, ProxyError> {
    let url =
        Url::parse(url).map_err(|_| ProxyError::BadUrl("url ist keine gültige Adresse.".into()))?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(ProxyError::BadUrl("url muss http oder https sein.".into()));
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(ProxyError::BadUrl(
            "url darf keine Zugangsdaten enthalten.".into(),
        ));
    }
    let host = url
        .host_str()
        .ok_or_else(|| ProxyError::BadUrl("url braucht einen Host.".into()))?
        .to_string();
    if !host_allowed(network, &host) {
        return Err(ProxyError::Forbidden(host));
    }
    Ok(url)
}

/// Schreibt Inhalt und Inhaltstyp atomar in den Cache
async fn store(
    cache: &Path,
    key: &str,
    bin: &PathBuf,
    typ: &PathBuf,
    bytes: &[u8],
    content_type: &str,
) -> std::io::Result<()> {
    tokio::fs::create_dir_all(cache).await?;
    let tmp_type = cache.join(format!(".{key}.type.tmp"));
    tokio::fs::write(&tmp_type, content_type).await?;
    tokio::fs::rename(&tmp_type, typ).await?;
    let tmp = cache.join(format!(".{key}.tmp"));
    tokio::fs::write(&tmp, bytes).await?;
    tokio::fs::rename(&tmp, bin).await
}

fn sha_hex(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
