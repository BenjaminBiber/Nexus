//! Lädt die Index-Dateien aller Store-Repos parallel, jedes mit eigenem Zeitlimit und eigenem
//! Fehler; ein kaputtes oder langsames Repo hält die anderen nicht auf. Kein Cache: jede
//! Anfrage lädt neu.

use std::time::Duration;

use nexus_core::theme::store_index::{
    parse_index, theme_sdk_version, ParsedIndex, SkippedEntry, StoreIndexEntry, MAX_INDEX_BYTES,
    STORE_INDEX_HTTP, STORE_INDEX_TIMEOUT, STORE_INDEX_TOO_LARGE, STORE_INDEX_UNREACHABLE,
};
use nexus_core::theme::StoreRepo;
use nexus_core::HarnessError;
use serde::Serialize;

/// Standard-Zeitlimit je Repo
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);
/// Höchstzahl an Weiterleitungen
const MAX_REDIRECTS: usize = 3;

/// Ein Eintrag mit seinem Repo
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoEntry {
    pub repo_id: String,
    pub repo_url: String,
    #[serde(flatten)]
    pub entry: StoreIndexEntry,
}

/// Ergebnis je Repo
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoStatus {
    pub id: String,
    pub url: String,
    pub ok: bool,
    pub entry_count: usize,
    pub skipped: Vec<SkippedEntry>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<HarnessError>,
}

/// Antwort von `GET /api/themes/store/index`
#[derive(Debug, Clone, Serialize)]
pub struct StoreIndexResponse {
    pub entries: Vec<RepoEntry>,
    pub repos: Vec<RepoStatus>,
}

/// Lädt Index-Dateien mit Zeitlimit je Repo
#[derive(Debug, Clone)]
pub struct StoreIndexLoader {
    client: reqwest::Client,
    timeout: Duration,
}

impl StoreIndexLoader {
    /// Loader mit dem angegebenen Zeitlimit je Repo
    pub fn new(timeout: Duration) -> Self {
        let client = reqwest::Client::builder()
            .timeout(timeout)
            .redirect(reqwest::redirect::Policy::limited(MAX_REDIRECTS))
            .build()
            .unwrap_or_default();
        Self { client, timeout }
    }

    /// Lädt alle Repos parallel; Reihenfolge von `repos` bleibt erhalten
    pub async fn load_all(&self, repos: Vec<StoreRepo>) -> StoreIndexResponse {
        let results =
            futures::future::join_all(repos.iter().map(|repo| self.load(&repo.url))).await;
        let sdk = theme_sdk_version();
        let mut entries = Vec::new();
        let mut statuses = Vec::new();
        for (repo, result) in repos.into_iter().zip(results) {
            let mut status = RepoStatus {
                id: repo.id.clone(),
                url: repo.url.clone(),
                ok: false,
                entry_count: 0,
                skipped: Vec::new(),
                error: None,
            };
            match result.and_then(|bytes| parse_index(&bytes, &sdk)) {
                Ok(parsed) => {
                    status.ok = true;
                    status.entry_count = parsed.entries.len();
                    status.skipped = parsed.skipped;
                    entries.extend(parsed.entries.into_iter().map(|entry| RepoEntry {
                        repo_id: repo.id.clone(),
                        repo_url: repo.url.clone(),
                        entry,
                    }));
                }
                Err(error) => status.error = Some(error),
            }
            statuses.push(status);
        }
        entries.sort_by(|a, b| {
            a.entry
                .name
                .to_lowercase()
                .cmp(&b.entry.name.to_lowercase())
                .then_with(|| a.entry.id.cmp(&b.entry.id))
                .then_with(|| a.repo_id.cmp(&b.repo_id))
        });
        StoreIndexResponse {
            entries,
            repos: statuses,
        }
    }

    /// Lädt und prüft die Index-Datei eines einzelnen Repos
    pub async fn load_repo(&self, repo: &StoreRepo) -> Result<ParsedIndex, HarnessError> {
        let bytes = self.load(&repo.url).await?;
        parse_index(&bytes, &theme_sdk_version())
    }

    /// Der gemeinsame HTTP-Client (Weiterleitungen begrenzt)
    pub fn client(&self) -> &reqwest::Client {
        &self.client
    }

    /// Lädt eine Index-Datei; Fehler mit Code
    async fn load(&self, url: &str) -> Result<Vec<u8>, HarnessError> {
        match tokio::time::timeout(self.timeout, self.fetch(url)).await {
            Ok(result) => result,
            Err(_) => Err(timeout_error()),
        }
    }

    async fn fetch(&self, url: &str) -> Result<Vec<u8>, HarnessError> {
        let mut response = self.client.get(url).send().await.map_err(request_error)?;
        let status = response.status();
        if !status.is_success() {
            return Err(HarnessError::new(
                STORE_INDEX_HTTP,
                format!("Das Repo antwortet mit HTTP {}.", status.as_u16()),
            ));
        }
        if response
            .content_length()
            .is_some_and(|length| length > MAX_INDEX_BYTES as u64)
        {
            return Err(too_large());
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(request_error)? {
            if bytes.len() + chunk.len() > MAX_INDEX_BYTES {
                return Err(too_large());
            }
            bytes.extend_from_slice(&chunk);
        }
        Ok(bytes)
    }
}

impl Default for StoreIndexLoader {
    fn default() -> Self {
        Self::new(DEFAULT_TIMEOUT)
    }
}

fn request_error(error: reqwest::Error) -> HarnessError {
    if error.is_timeout() {
        timeout_error()
    } else {
        HarnessError::new(
            STORE_INDEX_UNREACHABLE,
            format!("Das Repo ist nicht erreichbar: {error}"),
        )
    }
}

fn timeout_error() -> HarnessError {
    HarnessError::new(
        STORE_INDEX_TIMEOUT,
        "Das Repo hat nicht rechtzeitig geantwortet.",
    )
}

fn too_large() -> HarnessError {
    HarnessError::new(
        STORE_INDEX_TOO_LARGE,
        "Die Index-Datei ist größer als 1 MiB.",
    )
}
