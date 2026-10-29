//! Läufe des Dispatchers erfassen und auswerten; portiert aus
//! `packages/core/src/board/DispatchRunService.ts`.

use std::sync::Arc;

use chrono::{DateTime, Utc};

use crate::board::database::{BoardDatabase, TicketFilter};
use crate::board::dispatch_run_types::{
    DispatchConfigVersion, DispatchRun, DispatchRunFilter, DispatchRunKind, DispatchRunSource,
    DispatchStats, DispatchStatsFilter,
};
use crate::board::dispatch_stats::{DispatchStatsCalculator, DispatchStatsFacts};
use crate::clock::iso;
use crate::dispatch::run_backfill::{BackfillInput, DispatchRunBackfill};
use crate::error::Result;
use crate::tx;

/// Was ein Nachtragen gefunden und angelegt hat
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BackfillResult {
    /// Rekonstruierte Läufe insgesamt
    pub found: usize,
    /// Davon neu angelegte (bei `dry_run` nur gefundene)
    pub created: Vec<DispatchRun>,
}

/// Läufe des Dispatchers erfassen und auswerten.
#[derive(Debug, Clone)]
pub struct DispatchRunService<D: BoardDatabase> {
    db: Arc<D>,
}

impl<D: BoardDatabase> DispatchRunService<D> {
    /// Dienst auf dieser Datenbank
    pub fn new(db: Arc<D>) -> Self {
        Self { db }
    }

    /// Schreibt einen ausgewerteten Lauf; `false`, wenn er schon erfasst ist
    pub async fn record(&self, run: &DispatchRun) -> Result<bool> {
        let run = run.clone();
        tx!(self.db, [run], |u| { u.dispatch_runs().insert(&run).await }).await
    }

    /// Merkt die Fassung der Dispatcher-Einstellung vor; `false`, wenn dieser Hash schon
    /// bekannt ist
    pub async fn record_config_version(&self, version: &DispatchConfigVersion) -> Result<bool> {
        let version = version.clone();
        tx!(self.db, [version], |u| {
            u.dispatch_runs().record_config_version(&version).await
        })
        .await
    }

    /// Fassungen der Einstellung, die in [from, to) zum ersten Mal auftraten, älteste zuerst
    pub async fn config_versions(
        &self,
        from: &str,
        to: &str,
    ) -> Result<Vec<DispatchConfigVersion>> {
        let from = from.to_string();
        let to = to.to_string();
        tx!(self.db, [from, to], |u| {
            u.dispatch_runs().list_config_versions(&from, &to).await
        })
        .await
    }

    /// Läufe aller Projekte mit Start in [from, to), ältester zuerst
    /// (Optimierungs-Vorschläge)
    pub async fn list_all(
        &self,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Vec<DispatchRun>> {
        let from = iso(from);
        let to = iso(to);
        tx!(self.db, [from, to], |u| {
            u.dispatch_runs().list_all(&from, &to).await
        })
        .await
    }

    /// Läufe aus `[dispatch]`-Kommentaren nachtragen (Herkunft `comments`, ID
    /// `c<KommentarId>`); schon vorhandene bleiben unverändert, daher beliebig oft
    /// ausführbar. `dry_run` schreibt nichts
    pub async fn backfill(&self, project_id: i32, dry_run: bool) -> Result<BackfillResult> {
        tx!(self.db, |u| {
            let comments = u.dispatch_stats().dispatch_comments(project_id).await?;
            let ids: Vec<String> = {
                let mut seen: std::collections::HashSet<&str> = std::collections::HashSet::new();
                comments
                    .iter()
                    .filter(|comment| seen.insert(comment.ticket_id.as_str()))
                    .map(|comment| comment.ticket_id.clone())
                    .collect()
            };
            let (history, tickets) = if ids.is_empty() {
                (Vec::new(), Vec::new())
            } else {
                (
                    u.dispatch_stats().status_history(&ids).await?,
                    u.tickets()
                        .list(&TicketFilter {
                            ids: Some(ids.clone()),
                            without_texts: true,
                            ..TicketFilter::default()
                        })
                        .await?,
                )
            };
            let runs = DispatchRunBackfill::runs(&BackfillInput {
                project_id,
                comments,
                history,
                tickets,
            });
            let existing = if runs.is_empty() {
                Vec::new()
            } else {
                u.dispatch_runs()
                    .existing(&runs.iter().map(|run| run.id.clone()).collect::<Vec<_>>())
                    .await?
            };
            let created: Vec<DispatchRun> = runs
                .iter()
                .filter(|run| !existing.contains(&run.id))
                .cloned()
                .collect();
            if !dry_run {
                for run in &created {
                    u.dispatch_runs().insert(run).await?;
                }
            }
            Ok(BackfillResult {
                found: runs.len(),
                created,
            })
        })
        .await
    }

    /// Kennzahlen für Projekt, Board, Zeitraum und Lauf-Art (eine Lesetransaktion)
    pub async fn stats(&self, filter: &DispatchStatsFilter) -> Result<DispatchStats> {
        let filter = filter.clone();
        tx!(self.db, [filter], |u| {
            let runs = u
                .dispatch_runs()
                .list(&DispatchRunFilter {
                    project_id: filter.project_id,
                    board_id: filter.board_id,
                    from: iso(filter.from),
                    to: iso(filter.to),
                })
                .await?;
            // Neueste Fassung der Einstellung bis zum Ende des Zeitraums
            let versions = u
                .dispatch_runs()
                .list_config_versions(
                    &iso(DateTime::from_timestamp_millis(0).expect("die Epoche ist gültig")),
                    &iso(filter.to),
                )
                .await?;
            let runs_since = u
                .dispatch_runs()
                .first_started(filter.project_id, DispatchRunSource::Dispatcher)
                .await?;
            Ok(DispatchStatsCalculator::calculate(&DispatchStatsFacts {
                from: filter.from,
                to: filter.to,
                kind: filter.kind.unwrap_or(DispatchRunKind::Work),
                runs,
                runs_since,
                config_changed_at: versions.last().map(|version| version.first_seen_at.clone()),
            }))
        })
        .await
    }
}

#[cfg(test)]
mod tests;
