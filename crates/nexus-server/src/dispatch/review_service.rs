//! Freigabe vor dem Merge: Gate nach dem Lauf, Ansicht (Diff, Tests) und „Zurück an den
//! Agenten“. Portiert aus `packages/server/src/dispatch/ReviewService.ts`.

use std::path::Path;

use async_trait::async_trait;
use nexus_core::board::basics::{is_human_interaction, BoardActor, TicketStatus};
use nexus_core::board::ticket_service::{TicketDetail, TicketPatch, WriteOptions};
use nexus_core::board::types::Board;
use nexus_core::dispatch::config::REVIEW_REASON;
use nexus_core::dispatch::git::{ReviewDiff, ReviewFile, ReviewPatch};
use nexus_core::dispatch::{markers, ReviewEntry, RunEnd, RunRecord, TestRun, TestRunExtractor};
use nexus_core::{HarnessError, Result};
use serde::{Deserialize, Serialize};

use crate::board_runtime::BoardRuntimeHolder;
use crate::dispatch::context::DispatchContext;
use crate::dispatch::notifier::{DispatchNotice, DispatchNoticeKind};
use crate::dispatch::result_handler::ReviewGate;

/// Höchstlänge der Rückmeldung bei „Zurück an den Agenten“
pub const MAX_RETURN_TEXT: usize = 10_000;

/// Review eines Tickets für das Panel
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewView {
    pub ticket_id: String,
    pub branch: Option<String>,
    /// Ziel des Merges (Zielbranch des Tickets, sonst ausgecheckter Branch des Hauptordners)
    pub target: Option<String>,
    /// Merge-Basis
    pub base: Option<String>,
    pub files: Vec<ReviewFile>,
    pub tests: Vec<TestRun>,
    /// Zeitpunkt der Testläufe (Gate) oder `None` ohne Review-Eintrag
    pub tests_at: Option<String>,
    /// Lauf-Protokoll (Dateiname) des Laufs
    pub run_log: Option<String>,
    /// false = kein Diff (fremdes Board, kein Repo, kein Branch, Git-Fehler); Grund in `reason`
    pub available: bool,
    pub reason: Option<String>,
}

/// Patch einer Datei des Reviews samt ihrem Pfad
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewFilePatch {
    pub path: String,
    pub patch: String,
    pub truncated: bool,
}

/// „Zurück an den Agenten“ für Kommandozeile und Oberfläche
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReturnedToAgent {
    pub ticket_id: String,
    pub text: String,
}

/// Freigabe vor dem Merge: Gate nach dem Lauf, Ansicht (Diff, Tests) und „Zurück an den
/// Agenten“.
#[derive(Debug, Default)]
pub struct ReviewService {
    tests: TestRunExtractor,
    diff: ReviewDiff,
}

impl ReviewService {
    /// Review-Dienst ohne Zustand
    pub fn new() -> Self {
        Self {
            tests: TestRunExtractor,
            diff: ReviewDiff::new(),
        }
    }

    /// Gate: Tests merken, Ticket auf Blockiert „Human Interaction: Review vor dem Merge“,
    /// Kommentar und Meldung
    pub async fn hold(
        &self,
        ctx: &DispatchContext,
        detail: &TicketDetail,
        record: &RunRecord,
        end: &RunEnd,
    ) -> Result<()> {
        let branch = detail
            .ticket
            .branch
            .clone()
            .unwrap_or_else(|| record.branch.clone());
        let tests = self.tests.extract(&end.result.output);
        ctx.stores.reviews.save(&ReviewEntry {
            ticket_id: detail.ticket.id.clone(),
            branch: branch.clone(),
            run_id: record.id.clone(),
            log_file: record.log_file.clone(),
            at: ctx.now_iso(),
            tests: tests.clone(),
        })?;
        let board = ctx.board(detail.ticket.board_id).await?;
        let changes = self
            .changes(
                ctx,
                board.as_ref(),
                &branch,
                detail.ticket.target_branch.as_deref(),
            )
            .await?;
        ctx.services()
            .tickets
            .update(
                &detail.ticket.id,
                &TicketPatch {
                    status: Some(TicketStatus::Blocked),
                    blocked_reason: Some(Some(REVIEW_REASON.to_string())),
                    assignee: Some(None),
                    ..TicketPatch::default()
                },
                ctx.actor(),
                &WriteOptions {
                    force: true,
                    ..WriteOptions::default()
                },
            )
            .await?;
        ctx.comment(
            &detail.ticket.id,
            &format!(
                "{}: wartet auf Freigabe – {changes}, Tests: {}.\nIm Panel „Übernehmen“ oder \
„Zurück an den Agenten“.",
                markers::REVIEW,
                test_summary(&tests)
            ),
        )
        .await?;
        ctx.log(&format!(
            "Review {} – wartet auf Freigabe vor dem Merge",
            detail.ticket.id
        ));
        ctx.notify(&DispatchNotice {
            kind: DispatchNoticeKind::Review,
            ticket_id: Some(detail.ticket.id.clone()),
            title: detail.ticket.title.clone(),
            text: format!("wartet auf Freigabe – {changes}"),
            repo_path: board.and_then(|board| board.repo_path),
        })
        .await;
        Ok(())
    }

    /// Ansicht für das Panel: Dateien aus Git (nur eigenes Board mit Repo), Tests aus dem
    /// Review-Eintrag
    pub async fn view(&self, ctx: &DispatchContext, id: &str) -> Result<ReviewView> {
        let detail = ctx.services().tickets.get(id).await?;
        let entry = ctx.stores.reviews.read(&detail.ticket.id)?;
        let base = ReviewView {
            ticket_id: detail.ticket.id.clone(),
            branch: detail.ticket.branch.clone(),
            target: None,
            base: None,
            files: Vec::new(),
            tests: entry
                .as_ref()
                .map(|entry| entry.tests.clone())
                .unwrap_or_default(),
            tests_at: entry.as_ref().map(|entry| entry.at.clone()),
            run_log: entry.as_ref().map(|entry| file_name(&entry.log_file)),
            available: false,
            reason: None,
        };
        let root = match self.unavailable(ctx, &detail).await? {
            Ok(root) => root,
            Err(problem) => {
                return Ok(ReviewView {
                    reason: Some(problem),
                    ..base
                })
            }
        };
        let branch = detail.ticket.branch.clone().unwrap_or_default();
        match self.diff.files(
            Path::new(&root),
            &branch,
            detail.ticket.target_branch.as_deref(),
        ) {
            Ok(list) => Ok(ReviewView {
                target: Some(list.target),
                base: Some(list.base),
                files: list.files,
                available: true,
                ..base
            }),
            Err(error) => {
                if BoardRuntimeHolder::is_connection_error(&error) {
                    return Err(error);
                }
                Ok(ReviewView {
                    reason: Some(format!("Diff nicht berechenbar: {error}")),
                    ..base
                })
            }
        }
    }

    /// Patch einer Datei der Dateiliste
    pub async fn file_diff(
        &self,
        ctx: &DispatchContext,
        id: &str,
        path: &str,
    ) -> Result<ReviewFilePatch> {
        let detail = ctx.services().tickets.get(id).await?;
        let root = self
            .unavailable(ctx, &detail)
            .await?
            .map_err(|problem| HarnessError::new("review-not-available", problem))?;
        let branch = detail.ticket.branch.clone().unwrap_or_default();
        let ReviewPatch { patch, truncated } = self.diff.file(
            Path::new(&root),
            &branch,
            path,
            detail.ticket.target_branch.as_deref(),
        )?;
        Ok(ReviewFilePatch {
            path: path.to_string(),
            patch,
            truncated,
        })
    }

    /// „Zurück an den Agenten“: Kommentar als Arbeitsanweisung, Ticket nach Geplant ohne Grund
    /// und Zuständigen; Branch und Worktree bleiben, der nächste Start setzt dort fort. Zählt
    /// nicht als Versuch
    pub async fn return_to_agent(
        &self,
        ctx: &DispatchContext,
        id: &str,
        text: &str,
        actor: &BoardActor,
    ) -> Result<ReturnedToAgent> {
        let detail = ctx.services().tickets.get(id).await?;
        let comment = text.trim();
        let refuse = |why: &str| -> nexus_core::Error {
            HarnessError::new(
                "return-not-possible",
                format!(
                    "{} kann nicht zurück an den Agenten: {why}",
                    detail.ticket.id
                ),
            )
            .into()
        };
        if comment.is_empty() {
            return Err(refuse("die Rückmeldung ist leer."));
        }
        if comment.chars().count() > MAX_RETURN_TEXT {
            return Err(refuse(&format!(
                "die Rückmeldung ist länger als {MAX_RETURN_TEXT} Zeichen."
            )));
        }
        if detail.ticket.status != TicketStatus::Blocked
            || !is_human_interaction(detail.ticket.blocked_reason.as_deref())
            || detail.ticket.branch.is_none()
        {
            return Err(refuse(
                "es wartet nicht auf Freigabe (nur Blockiert mit Grund „Human Interaction: …“ \
und Branch).",
            ));
        }
        if let Err(problem) = self.unavailable(ctx, &detail).await? {
            return Err(refuse(&problem));
        }
        if ctx
            .stores
            .runs
            .list()?
            .iter()
            .any(|run| run.ticket_id == detail.ticket.id)
        {
            return Err(refuse("für das Ticket läuft gerade ein Lauf."));
        }
        ctx.comment(
            &detail.ticket.id,
            &format!("{} (von {}): {comment}", markers::REVIEW_RETURN, actor.name),
        )
        .await?;
        ctx.services()
            .tickets
            .update(
                &detail.ticket.id,
                &TicketPatch {
                    status: Some(TicketStatus::Planned),
                    blocked_reason: Some(None),
                    assignee: Some(None),
                    ..TicketPatch::default()
                },
                actor,
                &WriteOptions {
                    force: true,
                    ..WriteOptions::default()
                },
            )
            .await?;
        ctx.log(&format!(
            "zurück an den Agenten {} (von {})",
            detail.ticket.id, actor.name
        ));
        Ok(ReturnedToAgent {
            ticket_id: detail.ticket.id.clone(),
            text: format!(
                "{} zurück an den Agenten – der nächste Lauf setzt im selben Worktree fort.",
                detail.ticket.id
            ),
        })
    }

    /// Hauptordner des eigenen Boards, sonst warum es keinen Diff gibt
    async fn unavailable(
        &self,
        ctx: &DispatchContext,
        detail: &TicketDetail,
    ) -> Result<std::result::Result<String, String>> {
        if detail.ticket.branch.is_none() {
            return Ok(Err("Am Ticket ist kein Branch eingetragen.".to_string()));
        }
        let board = ctx.board(detail.ticket.board_id).await?;
        let own = board
            .as_ref()
            .filter(|board| board.owner_id == ctx.me.user_id && board.repo_path.is_some());
        let Some(board) = own else {
            return Ok(Err(
                "Das Board gehört einem anderen Benutzer oder hat keinen Repo-Pfad – der Diff \
ist nur auf dem Rechner des Dispatchers sichtbar."
                    .to_string(),
            ));
        };
        Ok(Ok(ctx.repo(board).await?.root.clone()))
    }

    /// „n Dateien geändert (+a/−d)“; lässt sich der Diff nicht berechnen, ein Hinweis
    async fn changes(
        &self,
        ctx: &DispatchContext,
        board: Option<&Board>,
        branch: &str,
        target: Option<&str>,
    ) -> Result<String> {
        let Some(board) = board.filter(|board| board.repo_path.is_some()) else {
            return Ok("Diff nicht berechenbar (kein Repo-Pfad)".to_string());
        };
        let root = ctx.repo(board).await?.root.clone();
        match self.diff.files(Path::new(&root), branch, target) {
            Ok(list) => {
                let add: u64 = list.files.iter().map(|file| file.additions).sum();
                let del: u64 = list.files.iter().map(|file| file.deletions).sum();
                let count = list.files.len();
                let word = if count == 1 { "Datei" } else { "Dateien" };
                Ok(format!("{count} {word} geändert (+{add}/−{del})"))
            }
            Err(error) => {
                if BoardRuntimeHolder::is_connection_error(&error) {
                    return Err(error);
                }
                Ok(format!("Diff nicht berechenbar ({error})"))
            }
        }
    }
}

#[async_trait]
impl ReviewGate for ReviewService {
    async fn hold(
        &self,
        ctx: &DispatchContext,
        detail: &TicketDetail,
        record: &RunRecord,
        end: &RunEnd,
    ) -> Result<()> {
        ReviewService::hold(self, ctx, detail, record, end).await
    }
}

/// „x grün, y rot[, z ohne Ergebnis]“ oder „keine erkannt“
fn test_summary(tests: &[TestRun]) -> String {
    if tests.is_empty() {
        return "keine erkannt".to_string();
    }
    let green = tests.iter().filter(|test| test.ok == Some(true)).count();
    let red = tests.iter().filter(|test| test.ok == Some(false)).count();
    let none = tests.len() - green - red;
    let rest = if none > 0 {
        format!(", {none} ohne Ergebnis")
    } else {
        String::new()
    };
    format!("{green} grün, {red} rot{rest}")
}

/// Dateiname eines Pfades (wie `basename` in Node)
fn file_name(path: &str) -> String {
    Path::new(path)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test(ok: Option<bool>) -> TestRun {
        TestRun {
            command: "npm test".into(),
            ok,
            output: String::new(),
        }
    }

    #[test]
    fn zusammenfassung_der_testlaufe() {
        assert_eq!(test_summary(&[]), "keine erkannt");
        assert_eq!(
            test_summary(&[test(Some(true)), test(Some(false))]),
            "1 grün, 1 rot"
        );
        assert_eq!(
            test_summary(&[test(Some(true)), test(None)]),
            "1 grün, 0 rot, 1 ohne Ergebnis"
        );
    }
}
