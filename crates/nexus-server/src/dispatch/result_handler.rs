//! Wertet einen beendeten Lauf aus; portiert aus
//! `packages/server/src/dispatch/ResultHandler.ts`.
//!
//! Freigabe vor dem Merge ([`ReviewGate`]) und das Ende einer Merge-Session
//! ([`MergeSessionEnd`]) stehen hier als Trait: dahinter stehen im Betrieb
//! [`crate::dispatch::review_service::ReviewService`] und
//! [`crate::dispatch::merge_service::MergeService`]. Den Kontext bekommen sie als Parameter,
//! weil der Dispatcher ihn je Durchlauf baut.

use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use nexus_core::board::basics::{is_human_interaction, TicketStatus, MAX_BLOCKED_REASON_LENGTH};
use nexus_core::board::ticket_service::{TicketDetail, TicketPatch, WriteOptions};
use nexus_core::board::types::Board;
use nexus_core::clock::iso;
use nexus_core::dispatch::autopilot::autopilot_for;
use nexus_core::dispatch::config::AUTOPILOT_PREFIX;
use nexus_core::dispatch::{
    limit_pause, markers, MergeEntry, MergePhase, RunEnd, RunKind, RunRecord, SetupFailure,
    SETUP_REASON_PREFIX,
};
use nexus_core::Result;

use crate::board_runtime::BoardRuntimeHolder;
use crate::dispatch::context::DispatchContext;
use crate::dispatch::notifier::{DispatchNotice, DispatchNoticeKind};
use crate::dispatch::run_recorder::{AbortInfo, HandleOutcome, HandleResult};

/// So viele Limit-Pausen in Folge zählen nicht als Versuch; die nächste schon
const LIMIT_PAUSES_FREE: usize = 2;

/// Freigabe vor dem Merge: hält das Ticket nach dem Lauf an, bis ein Mensch entscheidet
/// (`ReviewService.hold`).
#[async_trait]
pub trait ReviewGate: std::fmt::Debug + Send + Sync {
    /// Hält das Ticket im Review
    async fn hold(
        &self,
        ctx: &DispatchContext,
        detail: &TicketDetail,
        record: &RunRecord,
        end: &RunEnd,
    ) -> Result<()>;
}

/// Hält nichts an (für Tests, die den Review nicht brauchen).
#[derive(Debug, Clone, Copy, Default)]
pub struct NoReviewGate;

#[async_trait]
impl ReviewGate for NoReviewGate {
    async fn hold(
        &self,
        _ctx: &DispatchContext,
        _detail: &TicketDetail,
        _record: &RunRecord,
        _end: &RunEnd,
    ) -> Result<()> {
        Ok(())
    }
}

/// Eine beendete Merge-Session (`MergeService.afterSession`).
#[async_trait]
pub trait MergeSessionEnd: std::fmt::Debug + Send + Sync {
    /// Wertet das Ende einer Merge-Session aus
    async fn session_ended(
        &self,
        ctx: &DispatchContext,
        record: &RunRecord,
        end: &RunEnd,
    ) -> Result<()>;
}

/// Tut nichts (für Tests, die den Merge nicht brauchen).
#[derive(Debug, Clone, Copy, Default)]
pub struct NoMergeSessionEnd;

#[async_trait]
impl MergeSessionEnd for NoMergeSessionEnd {
    async fn session_ended(
        &self,
        _ctx: &DispatchContext,
        _record: &RunRecord,
        _end: &RunEnd,
    ) -> Result<()> {
        Ok(())
    }
}

/// Wertet beendete Läufe aus (Port von `finishRun`/`abortTicket` des alten Boards, erweitert um
/// Fragen und Merge).
#[derive(Debug)]
pub struct ResultHandler<'a> {
    ctx: &'a DispatchContext,
    reviews: Arc<dyn ReviewGate>,
    merges: Arc<dyn MergeSessionEnd>,
}

impl<'a> ResultHandler<'a> {
    /// Auswertung auf diesem Kontext
    pub fn new(
        ctx: &'a DispatchContext,
        reviews: Arc<dyn ReviewGate>,
        merges: Arc<dyn MergeSessionEnd>,
    ) -> Self {
        Self {
            ctx,
            reviews,
            merges,
        }
    }

    /// Auswertung mit den Leer-Umsetzungen für Review und Merge-Session
    pub fn without_gates(ctx: &'a DispatchContext) -> Self {
        Self::new(ctx, Arc::new(NoReviewGate), Arc::new(NoMergeSessionEnd))
    }

    /// Ausgang (done, review, blocked, question, limit, claude-missing, released, taken, other,
    /// gone, merge-session, setup-failed) samt Abbruch- bzw. Blockiert-Grund für die Auswertung
    pub async fn handle(
        &self,
        record: &RunRecord,
        end: &RunEnd,
        reason: Option<&str>,
    ) -> Result<HandleResult> {
        if record.kind == RunKind::Merge {
            self.merges.session_ended(self.ctx, record, end).await?;
            return Ok(HandleResult::new(HandleOutcome::MergeSession));
        }
        self.evaluate(record, end, reason).await
    }

    /// Auswertung eines Umsetzungslaufs
    async fn evaluate(
        &self,
        record: &RunRecord,
        end: &RunEnd,
        reason: Option<&str>,
    ) -> Result<HandleResult> {
        let ctx = self.ctx;
        let detail = match ctx.services().tickets.get(&record.ticket_id).await {
            Ok(detail) => detail,
            // Nur ein gelöschtes Ticket ist weg; Verbindungsfehler gehen an den Durchlauf
            // (der Lauf bleibt für später)
            Err(error) if error.code() == Some("ticket-not-found") => {
                return Ok(HandleResult::new(HandleOutcome::Gone))
            }
            Err(error) => return Err(error),
        };
        let board = ctx.board(detail.ticket.board_id).await?;
        if record.kind == RunKind::Work && record.setup.as_ref().is_some_and(|s| !s.is_empty()) {
            if end.result.setup_ok == Some(true) {
                self.setup_done(record, board.as_ref()).await?;
            }
            if let Some(failed) = &end.result.setup_failed {
                if detail.ticket.assignee.as_deref() != Some(record.agent.as_str()) {
                    return Ok(HandleResult::new(HandleOutcome::Taken));
                }
                return self
                    .setup_failed(&detail, record, failed, board.as_ref())
                    .await;
            }
        }
        if detail.ticket.status == TicketStatus::Done {
            // Hat ein Mensch das Ticket übernommen und selbst abgeschlossen, merged der
            // Dispatcher nicht
            if detail
                .ticket
                .assignee
                .as_deref()
                .is_some_and(|assignee| assignee != record.agent)
            {
                return Ok(HandleResult::new(HandleOutcome::Taken));
            }
            // Freigabe vor dem Merge: wartet als Human Interaction auf „Übernehmen“ oder
            // „Zurück an den Agenten“
            // Im Autopilot gibt es keine Freigabe
            if ctx.cfg.review_before_merge
                && detail.ticket.branch.is_some()
                && !autopilot_for(&ctx.cfg, &detail.ticket.labels)
            {
                self.reviews.hold(ctx, &detail, record, end).await?;
                return Ok(HandleResult::new(HandleOutcome::Review));
            }
            return self.schedule_merge(&detail, record).await;
        }
        if detail.ticket.assignee.as_deref() != Some(record.agent.as_str()) {
            return Ok(HandleResult::new(HandleOutcome::Taken));
        }
        if detail.ticket.status == TicketStatus::Blocked
            && autopilot_for(&ctx.cfg, &detail.ticket.labels)
            && is_human_interaction(detail.ticket.blocked_reason.as_deref())
        {
            return self.auto_accept(&detail, record).await;
        }
        if detail.ticket.status == TicketStatus::Blocked {
            ctx.notify(&DispatchNotice {
                kind: DispatchNoticeKind::Blocked,
                ticket_id: Some(detail.ticket.id.clone()),
                title: detail.ticket.title.clone(),
                text: detail
                    .ticket
                    .blocked_reason
                    .clone()
                    .unwrap_or_else(|| "Blockiert".to_string()),
                repo_path: board.as_ref().and_then(|board| board.repo_path.clone()),
            })
            .await;
            return Ok(HandleResult {
                outcome: HandleOutcome::Blocked,
                abort: None,
                blocked_reason: detail.ticket.blocked_reason.clone(),
            });
        }
        self.evaluate_open(&detail, record, end, reason, board.as_ref())
            .await
    }

    /// Autopilot: nimmt ein auf „Human Interaction: …“ blockiertes Ticket selbst ab
    /// (Abnahme-Text fehlt → der Grund), vermerkt das am Ticket und merkt den Merge vor
    async fn auto_accept(&self, detail: &TicketDetail, record: &RunRecord) -> Result<HandleResult> {
        let ctx = self.ctx;
        let id = &detail.ticket.id;
        let reason = detail
            .ticket
            .blocked_reason
            .clone()
            .unwrap_or_default()
            .trim()
            .to_string();
        let tickets = &ctx.services().tickets;
        if detail.ticket.acceptance.trim().is_empty() {
            tickets
                .update(
                    id,
                    &TicketPatch {
                        acceptance: Some(reason.clone()),
                        ..TicketPatch::default()
                    },
                    ctx.actor(),
                    &WriteOptions::default(),
                )
                .await?;
        }
        tickets
            .accept(id, ctx.actor(), &WriteOptions::default())
            .await?;
        ctx.comment(
            id,
            &format!("{AUTOPILOT_PREFIX} automatisch abgenommen – {reason}"),
        )
        .await?;
        let accepted = tickets.get(id).await?;
        self.schedule_merge(&accepted, record).await
    }

    /// Merkt den Merge eines erledigten Tickets vor (Ausgang done)
    async fn schedule_merge(
        &self,
        detail: &TicketDetail,
        record: &RunRecord,
    ) -> Result<HandleResult> {
        let ctx = self.ctx;
        {
            let id = detail.ticket.id.clone();
            let branch = detail
                .ticket
                .branch
                .clone()
                .unwrap_or_else(|| record.branch.clone());
            let since = ctx.now_iso();
            ctx.stores
                .state
                .update(|state| {
                    state.merges.insert(
                        id.clone(),
                        MergeEntry {
                            ticket_id: id.clone(),
                            branch,
                            phase: MergePhase::Pending,
                            session_used: false,
                            wait_noted: false,
                            target: None,
                            target_head: None,
                            conflict_seen: None,
                            held_by: None,
                            since,
                        },
                    );
                })
                .await?;
            ctx.log(&format!("erledigt {} – Merge vorgemerkt", detail.ticket.id));
        }
        Ok(HandleResult::new(HandleOutcome::Done))
    }

    /// Auswertung eines Laufs, dessen Ticket weder erledigt noch blockiert ist
    async fn evaluate_open(
        &self,
        detail: &TicketDetail,
        record: &RunRecord,
        end: &RunEnd,
        reason: Option<&str>,
        board: Option<&Board>,
    ) -> Result<HandleResult> {
        let ctx = self.ctx;
        if detail.ticket.status != TicketStatus::InProgress {
            return Ok(HandleResult::new(HandleOutcome::Other));
        }
        if !detail.view.waiting_on_questions.is_empty() {
            let ids = detail
                .view
                .waiting_on_questions
                .iter()
                .map(|question| question.id.clone())
                .collect::<Vec<_>>()
                .join(", ");
            ctx.services()
                .tickets
                .release(&detail.ticket.id, ctx.actor(), Some(TicketStatus::Planned))
                .await?;
            let branch = detail.ticket.branch.as_deref().unwrap_or(&record.branch);
            ctx.comment(
                &detail.ticket.id,
                &format!(
                    "{}: wartet auf {ids} – Ticket zurück nach Geplant, Branch {branch} und \
Worktree bleiben. Nach der Entscheidung geht es dort weiter.",
                    markers::QUESTION
                ),
            )
            .await?;
            ctx.notify(&DispatchNotice {
                kind: DispatchNoticeKind::Question,
                ticket_id: Some(detail.ticket.id.clone()),
                title: detail.ticket.title.clone(),
                text: format!("wartet auf {ids}"),
                repo_path: board.as_ref().and_then(|board| board.repo_path.clone()),
            })
            .await;
            return Ok(HandleResult::new(HandleOutcome::Question));
        }
        let minutes = ctx.cfg.pause_minutes_on_limit;
        if reason.is_none() && ctx.output.claude_missing(&end.result) {
            // Kein Fehler des Tickets: nicht als Versuch zählen, sondern pausieren, bis Claude
            // Code wieder da ist
            self.pause(
                detail,
                board.as_ref().and_then(|board| board.repo_path.clone()),
                &format!(
                    "{}: Claude Code wurde nicht gefunden (Befehl „{}“), Ticket zurück nach \
Geplant. Der Dispatcher startet {minutes} min lang nichts Neues. Bitte Claude Code \
installieren oder den Befehl in der Einstellung prüfen. Zählt nicht als Versuch.",
                    markers::PAUSE,
                    record.command
                ),
                &format!("Claude Code nicht gefunden – Pause {minutes} min"),
            )
            .await?;
            return Ok(HandleResult::new(HandleOutcome::ClaudeMissing));
        }
        let summary = ctx.output.summarize(&end.result.output);
        if reason.is_none() && ctx.output.hit_usage_limit(&end.result) {
            // Ab der dritten Limit-Pause in Folge liegt es vermutlich am Ticket selbst: dann
            // zählt es als Versuch
            // Nur die Pausen des Accounts, mit dem dieser Lauf gestartet ist
            if ctx
                .attempts()
                .limit_pauses_in_row(&detail.comments, record.run.account.as_deref())
                < LIMIT_PAUSES_FREE
            {
                self.limit_pause(
                    detail,
                    record,
                    board.as_ref().and_then(|board| board.repo_path.clone()),
                )
                .await?;
                return Ok(HandleResult::new(HandleOutcome::Limit));
            }
            return self
                .abort(
                    detail,
                    &format!("Nutzungslimit zum {}. Mal in Folge", LIMIT_PAUSES_FREE + 1),
                    &ctx.output.shown(&end.result.output, &summary),
                    Some(&record.log_file),
                )
                .await;
        }
        let reason = match reason {
            Some(reason) => reason.to_string(),
            None => ctx
                .output
                .abort_reason(&end.result, &summary, &record.run, None),
        };
        self.abort(
            detail,
            &reason,
            &ctx.output.shown(&end.result.output, &summary),
            Some(&record.log_file),
        )
        .await
    }

    /// Einrichtung gelungen: im Verzeichnis der Worktrees merken, damit ein Fortsetzen nicht
    /// erneut installiert
    async fn setup_done(&self, record: &RunRecord, board: Option<&Board>) -> Result<()> {
        let Some(board) = board.filter(|board| board.repo_path.is_some()) else {
            return Ok(());
        };
        let marked = async {
            self.ctx.repo(board).await?.manager.mark_setup(
                &record.branch,
                nexus_core::worktrees::registry::SetupState::Done,
            )
        }
        .await;
        if let Err(error) = marked {
            if BoardRuntimeHolder::is_connection_error(&error) {
                return Err(error);
            }
            self.ctx.log(&format!(
                "FEHLER {}: Einrichtung nicht vermerkt: {error}",
                record.ticket_id
            ));
        }
        Ok(())
    }

    /// Einrichtung gescheitert (Claude lief nicht): Blockiert mit Grund „Einrichtung
    /// fehlgeschlagen: …“, Ausgabe als Kommentar, Meldung. Zählt nicht als Versuch – es liegt an
    /// der Umgebung, ein größeres Profil hilft nicht; beim nächsten Start wird die Einrichtung
    /// erneut versucht
    async fn setup_failed(
        &self,
        detail: &TicketDetail,
        record: &RunRecord,
        failed: &SetupFailure,
        board: Option<&Board>,
    ) -> Result<HandleResult> {
        let ctx = self.ctx;
        let ended = if failed.timed_out {
            "Zeitlimit".to_string()
        } else {
            format!(
                "Exit {}",
                failed.code.map(|c| c.to_string()).unwrap_or_default()
            )
        };
        let step = format!("{}$ {} ({ended})", failed.cwd, failed.run);
        let full = collapse(&format!("{SETUP_REASON_PREFIX} {step}"));
        let reason = shorten(&full, MAX_BLOCKED_REASON_LENGTH);
        let mut lines = vec![
            format!(
                "{} fehlgeschlagen: {step}. Claude wurde nicht gestartet; das zählt nicht als \
Versuch.",
                markers::SETUP
            ),
            "Nach dem Beheben das Ticket wieder auf Geplant setzen – beim nächsten Start läuft \
die Einrichtung erneut."
                .to_string(),
        ];
        let shown = ctx.output.tail(&failed.output, TAIL);
        if !shown.is_empty() {
            lines.extend([
                String::new(),
                "Letzte Ausgabe:".to_string(),
                "```".to_string(),
                shown,
                "```".to_string(),
            ]);
        }
        lines.extend([
            String::new(),
            format!("Protokoll: {}", file_name(&record.log_file)),
        ]);
        ctx.comment(&detail.ticket.id, &lines.join("\n")).await?;
        ctx.services()
            .tickets
            .update(
                &detail.ticket.id,
                &TicketPatch {
                    status: Some(TicketStatus::Blocked),
                    blocked_reason: Some(Some(reason.clone())),
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
        ctx.log(&format!(
            "Einrichtung fehlgeschlagen {}: {step}",
            detail.ticket.id
        ));
        ctx.notify(&DispatchNotice {
            kind: DispatchNoticeKind::Blocked,
            ticket_id: Some(detail.ticket.id.clone()),
            title: detail.ticket.title.clone(),
            text: reason,
            repo_path: board.and_then(|board| board.repo_path.clone()),
        })
        .await;
        Ok(HandleResult::new(HandleOutcome::SetupFailed))
    }

    /// Nutzungslimit: nur der Account des Laufs pausiert (mit nur einem Account der ganze
    /// Dispatcher wie bisher); das Ticket geht zurück nach Geplant und startet beim nächsten
    /// Durchlauf auf einem anderen freien Account
    async fn limit_pause(
        &self,
        detail: &TicketDetail,
        record: &RunRecord,
        repo_path: Option<String>,
    ) -> Result<()> {
        let ctx = self.ctx;
        let pause = ctx.pause_account(record.run.account.as_deref()).await?;
        let bis = ctx.hhmm(&pause.until);
        if !pause.multi {
            return self
                .released(
                    detail,
                    repo_path,
                    &format!(
                        "{} erreicht, Ticket zurück nach Geplant. Der Dispatcher startet bis \
{bis} nichts Neues. Zählt nicht als Versuch.",
                        limit_pause()
                    ),
                    &format!("Nutzungslimit – Pause bis {bis}"),
                    &pause.until,
                )
                .await;
        }
        let rest = if pause.global {
            format!("Alle Accounts sind pausiert – der Dispatcher startet bis {bis} nichts Neues.")
        } else {
            "Das Ticket startet beim nächsten Durchlauf auf einem anderen freien Account."
                .to_string()
        };
        self.released(
            detail,
            repo_path,
            &format!(
                "{} von Account {} erreicht, Ticket zurück nach Geplant. Account {} pausiert \
bis {bis}. {rest} Zählt nicht als Versuch.",
                limit_pause(),
                pause.account,
                pause.account
            ),
            &format!("Nutzungslimit {} – Pause bis {bis}", pause.account),
            &pause.until,
        )
        .await
    }

    /// Kommentar, zurück nach Geplant, Meldung; die Pause hat [`DispatchContext::pause_account`]
    /// schon gesetzt
    async fn released(
        &self,
        detail: &TicketDetail,
        repo_path: Option<String>,
        comment: &str,
        notice: &str,
        until: &str,
    ) -> Result<()> {
        let ctx = self.ctx;
        ctx.comment(&detail.ticket.id, comment).await?;
        ctx.services()
            .tickets
            .release(&detail.ticket.id, ctx.actor(), Some(TicketStatus::Planned))
            .await?;
        ctx.log(&format!(
            "Pause bis {until}: {notice} ({})",
            detail.ticket.id
        ));
        ctx.notify(&DispatchNotice {
            kind: DispatchNoticeKind::Paused,
            ticket_id: Some(detail.ticket.id.clone()),
            title: detail.ticket.title.clone(),
            text: notice.to_string(),
            repo_path,
        })
        .await;
        Ok(())
    }

    /// Pause ohne Versuch: Kommentar, zurück nach Geplant, neue Starts für
    /// `pause_minutes_on_limit` aussetzen, Meldung
    async fn pause(
        &self,
        detail: &TicketDetail,
        repo_path: Option<String>,
        comment: &str,
        notice: &str,
    ) -> Result<()> {
        let ctx = self.ctx;
        ctx.comment(&detail.ticket.id, comment).await?;
        ctx.services()
            .tickets
            .release(&detail.ticket.id, ctx.actor(), Some(TicketStatus::Planned))
            .await?;
        let until = iso(ctx.now() + chrono::Duration::minutes(ctx.cfg.pause_minutes_on_limit));
        let stored = until.clone();
        ctx.stores
            .state
            .update(move |state| {
                state.paused_until = Some(stored);
            })
            .await?;
        ctx.log(&format!(
            "Pause bis {until}: {notice} ({})",
            detail.ticket.id
        ));
        ctx.notify(&DispatchNotice {
            kind: DispatchNoticeKind::Paused,
            ticket_id: Some(detail.ticket.id.clone()),
            title: detail.ticket.title.clone(),
            text: notice.to_string(),
            repo_path,
        })
        .await;
        Ok(())
    }

    /// Abbruch: Kommentar mit Grund und Ausgabe; zurück nach Geplant, beim letzten Versuch
    /// Blockiert. Liefert den Ausgang (released, blocked) mit Grund und ob es der letzte Versuch
    /// war
    pub async fn abort(
        &self,
        detail: &TicketDetail,
        reason: &str,
        output: &str,
        log_file: Option<&str>,
    ) -> Result<HandleResult> {
        let ctx = self.ctx;
        let attempt = ctx.attempts().failed(&detail.comments) as i64 + 1;
        let last = attempt >= ctx.cfg.max_attempts;
        let mut lines = vec![
            format!(
                "{} Versuch {attempt}/{}: {reason}.",
                markers::ABORT,
                ctx.cfg.max_attempts
            ),
            if last {
                "Das war der letzte automatische Versuch – das Ticket geht nach Blockiert."
                    .to_string()
            } else {
                "Das Ticket geht zurück nach Geplant.".to_string()
            },
        ];
        let shown = ctx.output.tail(output, TAIL);
        if !shown.is_empty() {
            lines.extend([
                String::new(),
                "Letzte Ausgabe:".to_string(),
                "```".to_string(),
                shown,
                "```".to_string(),
            ]);
        }
        if let Some(log_file) = log_file {
            lines.extend([String::new(), format!("Protokoll: {}", file_name(log_file))]);
        }
        ctx.comment(&detail.ticket.id, &lines.join("\n")).await?;
        ctx.log(&format!(
            "Abbruch {} ({attempt}/{}): {reason}",
            detail.ticket.id, ctx.cfg.max_attempts
        ));
        if last {
            let blocked_reason =
                format!("Automatische Umsetzung {attempt}× abgebrochen – bitte ansehen");
            ctx.services()
                .tickets
                .update(
                    &detail.ticket.id,
                    &TicketPatch {
                        status: Some(TicketStatus::Blocked),
                        blocked_reason: Some(Some(blocked_reason.clone())),
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
            let board = ctx.board(detail.ticket.board_id).await?;
            ctx.notify(&DispatchNotice {
                kind: DispatchNoticeKind::Blocked,
                ticket_id: Some(detail.ticket.id.clone()),
                title: detail.ticket.title.clone(),
                text: blocked_reason,
                repo_path: board.and_then(|board| board.repo_path),
            })
            .await;
            return Ok(HandleResult {
                outcome: HandleOutcome::Blocked,
                abort: Some(AbortInfo {
                    reason: reason.to_string(),
                    final_attempt: true,
                }),
                blocked_reason: None,
            });
        }
        ctx.services()
            .tickets
            .release(&detail.ticket.id, ctx.actor(), Some(TicketStatus::Planned))
            .await?;
        Ok(HandleResult {
            outcome: HandleOutcome::Released,
            abort: Some(AbortInfo {
                reason: reason.to_string(),
                final_attempt: false,
            }),
            blocked_reason: None,
        })
    }
}

/// So viele Zeichen Ausgabe stehen in einem Kommentar (`RunOutputReader.tail` ohne Angabe)
const TAIL: usize = 1500;

/// Mehrfache Leerzeichen zu einem, Rand weg (`replace(/\s+/g, ' ').trim()`)
fn collapse(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Text auf `max` Zeichen kürzen; gekürzt endet er auf „…“
fn shorten(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let kept: String = text.chars().take(max - 1).collect();
    format!("{kept}…")
}

/// Dateiname eines Pfads (`basename`)
fn file_name(path: &str) -> String {
    Path::new(path)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mehrfache_leerzeichen_werden_zu_einem() {
        assert_eq!(
            collapse("Einrichtung fehlgeschlagen:  .$ npm\n ci (Exit 1) "),
            "Einrichtung fehlgeschlagen: .$ npm ci (Exit 1)"
        );
    }

    #[test]
    fn zu_langer_grund_endet_auf_auslassungspunkte() {
        let long = "a".repeat(MAX_BLOCKED_REASON_LENGTH + 10);
        let short = shorten(&long, MAX_BLOCKED_REASON_LENGTH);
        assert_eq!(short.chars().count(), MAX_BLOCKED_REASON_LENGTH);
        assert!(short.ends_with('…'));
        assert_eq!(shorten("kurz", MAX_BLOCKED_REASON_LENGTH), "kurz");
    }

    #[test]
    fn nur_der_dateiname_steht_im_kommentar() {
        assert_eq!(file_name("/tmp/logs/ARC-0001-x.log"), "ARC-0001-x.log");
        assert_eq!(file_name("ARC-0001-x.log"), "ARC-0001-x.log");
    }
}
