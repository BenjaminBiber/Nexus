//! Lädt ein Theme-Paket aus dem Store: Download mit Zeit- und Größenlimit in den Speicher.
//! Geprüft und installiert wird danach in `routes::store_install`; auf die Platte kommt hier
//! nichts. Derselbe Weg holt die Vorschaubilder der Einträge (`routes::store_preview`), mit
//! kleinerem Limit.

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

/// Höchstgröße eines Vorschaubilds
pub const MAX_PREVIEW_BYTES: u64 = 5 * 1024 * 1024;

/// Was geladen wird: bestimmt das Größenlimit und den Wortlaut der Fehler
#[derive(Debug, Clone, Copy)]
struct Target {
    /// „Das Paket“
    name: &'static str,
    /// „des Pakets“
    of: &'static str,
    max_bytes: u64,
}

const PACKAGE: Target = Target {
    name: "Das Paket",
    of: "des Pakets",
    max_bytes: MAX_PACKAGE_BYTES,
};

const PREVIEW: Target = Target {
    name: "Das Vorschaubild",
    of: "des Vorschaubilds",
    max_bytes: MAX_PREVIEW_BYTES,
};

/// Lädt Pakete und Vorschaubilder mit Zeit- und Größenlimit
#[derive(Debug, Clone)]
pub struct StoreDownloader {
    client: reqwest::Client,
    timeout: Duration,
}

impl StoreDownloader {
    /// Downloader mit dem gemeinsamen Client und Zeitlimit
    pub fn new(client: reqwest::Client, timeout: Duration) -> Self {
        Self { client, timeout }
    }

    /// Lädt das Paket vollständig in den Speicher; Fehler mit Code
    pub async fn download(&self, url: &str) -> Result<Vec<u8>, HarnessError> {
        self.load(url, PACKAGE).await
    }

    /// Lädt das Vorschaubild eines Eintrags in den Speicher (höchstens
    /// [`MAX_PREVIEW_BYTES`]); Fehler mit denselben Codes wie beim Paket
    pub async fn download_preview(&self, url: &str) -> Result<Vec<u8>, HarnessError> {
        self.load(url, PREVIEW).await
    }

    async fn load(&self, url: &str, target: Target) -> Result<Vec<u8>, HarnessError> {
        check_url(url).map_err(|problem| {
            HarnessError::new(
                STORE_DOWNLOAD_URL_INVALID,
                format!("Die Download-Adresse {problem}"),
            )
        })?;
        match tokio::time::timeout(self.timeout, self.fetch(url, target)).await {
            Ok(result) => result,
            Err(_) => Err(timeout_error(target)),
        }
    }

    async fn fetch(&self, url: &str, target: Target) -> Result<Vec<u8>, HarnessError> {
        let mut response = self
            .client
            .get(url)
            .timeout(self.timeout)
            .send()
            .await
            .map_err(|error| request_error(error, target))?;
        let status = response.status();
        if !status.is_success() {
            return Err(HarnessError::new(
                STORE_DOWNLOAD_HTTP,
                format!(
                    "Der Download {} ist fehlgeschlagen: HTTP {}.",
                    target.of,
                    status.as_u16()
                ),
            ));
        }
        if response
            .content_length()
            .is_some_and(|length| length > target.max_bytes)
        {
            return Err(too_large(target));
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|error| request_error(error, target))?
        {
            if (bytes.len() + chunk.len()) as u64 > target.max_bytes {
                return Err(too_large(target));
            }
            bytes.extend_from_slice(&chunk);
        }
        Ok(bytes)
    }
}

fn too_large(target: Target) -> HarnessError {
    HarnessError::new(
        STORE_DOWNLOAD_TOO_LARGE,
        format!(
            "{} ist größer als {} MiB.",
            target.name,
            target.max_bytes / (1024 * 1024)
        ),
    )
}

fn request_error(error: reqwest::Error, target: Target) -> HarnessError {
    if error.is_timeout() {
        timeout_error(target)
    } else {
        HarnessError::new(
            STORE_DOWNLOAD_UNREACHABLE,
            format!("Der Download-Server ist nicht erreichbar: {error}"),
        )
    }
}

fn timeout_error(target: Target) -> HarnessError {
    HarnessError::new(
        STORE_DOWNLOAD_TIMEOUT,
        format!("Der Download {} hat zu lange gedauert.", target.of),
    )
}
