//! Detailansicht einer Session im Server: Transcript finden, Kennzahlen und Änderungen aus
//! dem Transcript (mit Zwischenspeicher, der beim Wachsen nur das Neue liest), Änderungen aus
//! Git (blockierend, mit Zeitlimit) und die Laufdaten des Dispatchers.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use chrono::{DateTime, Local, NaiveDateTime, TimeZone, Utc};
use nexus_core::sessions::detail::git_changes::not_found;
use nexus_core::sessions::detail::types::ChangeOrigin;
use nexus_core::sessions::detail::{
    run_kind_of, FileDiff, GitChanges, RunInfo, SessionChanges, TranscriptFacts, TranscriptLocator,
    TranscriptScan, TRANSCRIPT_NOTE,
};
use nexus_core::Result;

use crate::state::AppState;

/// Zeitlimit für Git
const GIT_TIMEOUT: Duration = Duration::from_secs(10);
/// Zeitlimit für die Board-Datenbank (Laufdaten)
const BOARD_TIMEOUT: Duration = Duration::from_secs(3);
/// Höchstzahl zwischengespeicherter Transcripts
const MAX_SCANS: usize = 32;

/// Ein zwischengespeicherter Stand
struct CachedScan {
    scan: TranscriptScan,
    cwd: String,
    used: Instant,
}

/// Zwischenspeicher der Transcript-Stände je Datei
#[derive(Default)]
pub struct SessionDetails {
    scans: Mutex<HashMap<PathBuf, CachedScan>>,
}

impl SessionDetails {
    /// Leerer Zwischenspeicher
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// Bringt den Stand der Datei auf den neuesten Stand und gibt ihn an `use_scan`
    /// (blockierend: Dateizugriff)
    pub fn with_scan<T>(
        &self,
        path: &Path,
        cwd: &str,
        use_scan: impl FnOnce(&TranscriptScan) -> T,
    ) -> Option<T> {
        let mut scans = self.scans.lock().expect("Sperre der Transcripts");
        let fresh = scans.get(path).is_none_or(|cached| cached.cwd != cwd);
        if fresh {
            if scans.len() >= MAX_SCANS {
                let oldest = scans
                    .iter()
                    .min_by_key(|(_, cached)| cached.used)
                    .map(|(key, _)| key.clone());
                if let Some(oldest) = oldest {
                    scans.remove(&oldest);
                }
            }
            scans.insert(
                path.to_path_buf(),
                CachedScan {
                    scan: TranscriptScan::new(cwd),
                    cwd: cwd.to_string(),
                    used: Instant::now(),
                },
            );
        }
        let cached = scans.get_mut(path)?;
        cached.used = Instant::now();
        cached.scan.update(path).ok()?;
        Some(use_scan(&cached.scan))
    }

    /// Kennzahlen des Transcripts
    pub fn facts(&self, path: &Path, cwd: &str) -> Option<TranscriptFacts> {
        self.with_scan(path, cwd, TranscriptScan::facts)
    }
}

/// Die Config-Ordner von Claude Code: Standard und die der Accounts
pub fn config_dirs(state: &AppState) -> Vec<PathBuf> {
    let paths = &state.accounts.paths;
    let mut dirs = vec![paths.files(None).config_dir];
    for account in state.accounts.store.list() {
        if let Some(dir) = account.config_dir.as_deref() {
            let dir = paths.files(Some(dir)).config_dir;
            if !dirs.contains(&dir) {
                dirs.push(dir);
            }
        }
    }
    dirs
}

/// Pfad des Transcripts einer Session (blockierend)
pub fn locate(
    state: &AppState,
    known: Option<&str>,
    cwd: &str,
    session_id: &str,
) -> Option<PathBuf> {
    TranscriptLocator::new(config_dirs(state)).locate(known, cwd, session_id)
}

/// Die Änderungen: Git, wenn die Session einen eigenen Worktree oder Branch hat und Git
/// antwortet; sonst das Transcript; sonst keine
pub async fn changes(
    state: &Arc<AppState>,
    cwd: &str,
    transcript: Option<PathBuf>,
) -> SessionChanges {
    let dir = PathBuf::from(cwd);
    let git = blocking(move || {
        let git = GitChanges::new();
        let scope = git.scope(&dir)?;
        let files = git.files(&scope).ok()?;
        Some(SessionChanges {
            origin: ChangeOrigin::Git,
            base: Some(scope.base),
            note: None,
            files,
        })
    })
    .await
    .flatten();
    if let Some(found) = git {
        return found;
    }
    let Some(path) = transcript else {
        return SessionChanges::none();
    };
    let details = state.session_details.clone();
    let cwd = cwd.to_string();
    blocking(move || {
        details.with_scan(&path, &cwd, |scan| SessionChanges {
            origin: ChangeOrigin::Transcript,
            base: None,
            note: Some(TRANSCRIPT_NOTE.to_string()),
            files: scan.changes().files(),
        })
    })
    .await
    .flatten()
    .unwrap_or_else(SessionChanges::none)
}

/// Diff einer Datei aus derselben Quelle wie [`changes`]
pub async fn file_diff(
    state: &Arc<AppState>,
    cwd: &str,
    transcript: Option<PathBuf>,
    path: &str,
) -> Result<FileDiff> {
    let dir = PathBuf::from(cwd);
    let wanted = path.to_string();
    let git = blocking(move || {
        let git = GitChanges::new();
        let scope = git.scope(&dir)?;
        Some(git.file(&scope, &wanted))
    })
    .await
    .flatten();
    if let Some(found) = git {
        return found;
    }
    let Some(transcript) = transcript else {
        return Err(not_found(path));
    };
    let details = state.session_details.clone();
    let cwd = cwd.to_string();
    let wanted = path.to_string();
    blocking(move || details.with_scan(&transcript, &cwd, |scan| scan.changes().diff(&wanted)))
        .await
        .flatten()
        .flatten()
        .ok_or_else(|| not_found(path))
}

/// Führt blockierende Arbeit mit [`GIT_TIMEOUT`] aus; nichts bei Zeitüberschreitung
async fn blocking<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static) -> Option<T> {
    match tokio::time::timeout(GIT_TIMEOUT, tokio::task::spawn_blocking(work)).await {
        Ok(Ok(value)) => Some(value),
        _ => None,
    }
}

/// Laufdaten zum Protokoll `name`: aus dem Lauf-Speicher (Art, Profil, Konto, Start),
/// ergänzt um den Eintrag in `DispatchRuns` (Ergebnis, Abbruchgrund, Ende)
pub async fn run_info(state: &Arc<AppState>, name: &str) -> RunInfo {
    let id = name.strip_suffix(".log").unwrap_or(name).to_string();
    let mut info = RunInfo {
        name: name.to_string(),
        kind: Some(run_kind_of(name)),
        ..RunInfo::default()
    };
    if let Ok(Some(record)) = state.dispatch.stores.runs.read(&id) {
        info.kind = Some(record.kind.as_str().to_string());
        info.profile = Some(record.run.profile.clone()).filter(|profile| !profile.is_empty());
        info.account = record.run.account.clone();
        info.started_at = Some(record.started_at.clone());
    }
    let Some(started) = started_at(&id) else {
        return info;
    };
    let from = started - chrono::Duration::days(1);
    let to = started + chrono::Duration::days(1);
    let lookup = state.board.run(move |backend| async move {
        backend.services().dispatch_runs.list_all(from, to).await
    });
    let Ok(Ok(runs)) = tokio::time::timeout(BOARD_TIMEOUT, lookup).await else {
        return info;
    };
    let prefix = format!("{id}-");
    if let Some(run) = runs
        .into_iter()
        .find(|run| run.id == id || run.id.starts_with(&prefix))
    {
        info.kind = Some(run.kind.as_str().to_string());
        if !run.profile.is_empty() {
            info.profile = Some(run.profile);
        }
        info.outcome = Some(run.outcome.as_str().to_string());
        info.abort_reason = run.abort_reason;
        info.started_at = Some(run.started_at);
        info.ended_at = run.ended_at;
    }
    info
}

/// Startzeit aus dem Namen des Laufs (`…-YYYYMMDD-HHMMSS[-art]`, Ortszeit)
pub fn started_at(id: &str) -> Option<DateTime<Utc>> {
    let parts: Vec<&str> = id.split('-').collect();
    let stamp = parts.windows(2).rev().find_map(|pair| {
        let (date, time) = (pair[0], pair[1]);
        let digits = |text: &str, len: usize| {
            text.len() == len && text.bytes().all(|byte| byte.is_ascii_digit())
        };
        (digits(date, 8) && digits(time, 6)).then(|| format!("{date}{time}"))
    })?;
    let naive = NaiveDateTime::parse_from_str(&stamp, "%Y%m%d%H%M%S").ok()?;
    Local
        .from_local_datetime(&naive)
        .earliest()
        .map(|local| local.with_timezone(&Utc))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn liest_die_startzeit_aus_dem_namen_des_laufs() {
        let expected = Local
            .with_ymd_and_hms(2026, 11, 4, 9, 15, 0)
            .unwrap()
            .with_timezone(&Utc);
        assert_eq!(started_at("ARC-0001-20261104-091500"), Some(expected));
        assert_eq!(started_at("ARC-0001-20261104-091500-merge"), Some(expected));
        assert_eq!(started_at("ARC-0001"), None);
    }
}
