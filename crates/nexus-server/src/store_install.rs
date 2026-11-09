//! Lädt ein Theme-Paket aus dem Store: Download mit Zeit- und Größenlimit in den Speicher.
//! Geprüft und installiert wird danach in `routes::store_install`; auf die Platte kommt hier
//! nichts.

use std::time::Duration;

use nexus_core::theme::store_index::check_url;
use nexus_core::themes::package::MAX_PACKAGE_BYTES;
use nexus_core::HarnessError;

/// Standard-Zeitlimit eines Downloads
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(60);

/// Fehlercode: Zeitlimit überschritten (HTTP 504)
pub const STORE_DOWNLOAD_TIMEOUT: &str = "store-download-timeout";
/// Fehlercode: keine Verbindung (HTTP 502)
pub const STORE_DOWNLOAD_UNREACHABLE: &str = "store-download-unreachable";
/// Fehlercode: HTTP-Status ≠ 2xx (HTTP 502)
pub const STORE_DOWNLOAD_HTTP: &str = "store-download-http";
/// Fehlercode: Paket größer als erlaubt (HTTP 413)
pub const STORE_DOWNLOAD_TOO_LARGE: &str = "store-download-too-large";
/// Fehlercode: Download-Adresse nicht erlaubt (HTTP 502)
pub const STORE_DOWNLOAD_URL_INVALID: &str = "store-download-url-invalid";

/// Lädt Pakete mit Zeit- und Größenlimit
#[derive(Debug, Clone)]
pub struct StoreDownloader {
    client: reqwest::Client,
    timeout: Duration,
    max_bytes: u64,
}

impl StoreDownloader {
    /// Downloader mit dem gemeinsamen Client, Zeitlimit und der Paket-Höchstgröße
    pub fn new(client: reqwest::Client, timeout: Duration) -> Self {
        Self {
            client,
            timeout,
            max_bytes: MAX_PACKAGE_BYTES,
        }
    }

    /// Lädt das Paket vollständig in den Speicher; Fehler mit Code
    pub async fn download(&self, url: &str) -> Result<Vec<u8>, HarnessError> {
        check_url(url).map_err(|problem| {
            HarnessError::new(
                STORE_DOWNLOAD_URL_INVALID,
                format!("Die Download-Adresse {problem}"),
            )
        })?;
        match tokio::time::timeout(self.timeout, self.fetch(url)).await {
            Ok(result) => result,
            Err(_) => Err(timeout_error()),
        }
    }

    async fn fetch(&self, url: &str) -> Result<Vec<u8>, HarnessError> {
        let mut response = self
            .client
            .get(url)
            .timeout(self.timeout)
            .send()
            .await
            .map_err(request_error)?;
        let status = response.status();
        if !status.is_success() {
            return Err(HarnessError::new(
                STORE_DOWNLOAD_HTTP,
                format!(
                    "Der Download des Pakets ist fehlgeschlagen: HTTP {}.",
                    status.as_u16()
                ),
            ));
        }
        if response
            .content_length()
            .is_some_and(|length| length > self.max_bytes)
        {
            return Err(self.too_large());
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(request_error)? {
            if (bytes.len() + chunk.len()) as u64 > self.max_bytes {
                return Err(self.too_large());
            }
            bytes.extend_from_slice(&chunk);
        }
        Ok(bytes)
    }

    fn too_large(&self) -> HarnessError {
        HarnessError::new(
            STORE_DOWNLOAD_TOO_LARGE,
            format!(
                "Das Paket ist größer als {} MiB.",
                self.max_bytes / (1024 * 1024)
            ),
        )
    }
}

fn request_error(error: reqwest::Error) -> HarnessError {
    if error.is_timeout() {
        timeout_error()
    } else {
        HarnessError::new(
            STORE_DOWNLOAD_UNREACHABLE,
            format!("Der Download-Server ist nicht erreichbar: {error}"),
        )
    }
}

fn timeout_error() -> HarnessError {
    HarnessError::new(
        STORE_DOWNLOAD_TIMEOUT,
        "Der Download des Pakets hat zu lange gedauert.",
    )
}
