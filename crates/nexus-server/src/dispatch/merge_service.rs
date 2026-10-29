//! Mergt erledigte Tickets in ihren Zielbranch, startet bei Konflikten eine Merge-Session und
//! räumt danach auf. Portiert aus `packages/server/src/dispatch/MergeService.ts`.
//!
//! Lässt der Ordner des Merges ihn gerade nicht zu (laufende Git-Aktion, vorgemerkte oder mit
//! dem Branch überschneidende lokale Änderungen), wartet der Auftrag; andere Probleme setzen
//! das Ticket auf Blockiert mit Grund „Merge: …“. Aufträge werden je Branch verarbeitet:
//! Tickets eines gemeinsamen Worktrees teilen Phase, Merge-Session und Ergebnis.

use std::collections::BTreeSet;
use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use nexus_core::board::basics::{
    is_human_interaction, BoardActor, TicketStatus, HUMAN_INTERACTION_PREFIX,
    MAX_BLOCKED_REASON_LENGTH,
};
use nexus_core::board::ticket_service::{MoveOptions, TicketDetail, TicketPatch, WriteOptions};
use nexus_core::board::types::Board;
use nexus_core::dispatch::config::MERGE_SESSION_TOOLS;
use nexus_core::dispatch::git::MergeOutcome;
use nexus_core::dispatch::{
    markers, DispatchState, MergeEntry, MergePhase, MergeStrategy, RunEnd, RunKind, RunRecord,
    RunSettings, TimeWindowCheck, MERGE_REASON_PREFIX,
};
use nexus_core::git::PathKey;
use nexus_core::worktrees::manager::RepoBoard;
use nexus_core::{HarnessError, Result};

use crate::board_runtime::BoardRuntimeHolder;
use crate::dispatch::context::{DispatchContext, LaunchInput, RepoWork};
use crate::dispatch::merge_git::{MergeGit, RealMergeGit};
use crate::dispatch::merge_step::{AcceptOutcome, MergeStep, MergeStepReport};
use crate::dispatch::notifier::{DispatchNotice, DispatchNoticeKind};
use crate::dispatch::result_handler::MergeSessionEnd;

/// Bezeichnung des Hauptordners in Meldungen
const MAIN_LABEL: &str = "Hauptordner";

/// Offene Aufträge eines Branches mit ihren noch erledigten Tickets
struct BranchJob {
    branch: String,
    entries: Vec<MergeEntry>,
    /// Tickets der Aufträge, die noch auf Erledigt mit diesem Branch stehen (erstes = Ticket der
    /// Merge-Session)
    tickets: Vec<TicketDetail>,
}

impl BranchJob {
    /// IDs der erledigten Tickets, in der Reihenfolge der Aufträge
    fn ids(&self) -> Vec<String> {
        self.tickets
            .iter()
            .map(|detail| detail.ticket.id.clone())
            .collect()
    }
}

/// Ort des Merges: Hauptordner, ein anderer Worktree des Zielbranches oder der eigene
/// Merge-Worktree
#[derive(Debug, Clone)]
struct MergePlace {
    /// Ordner, in dem gemergt wird
    path: String,
    /// Branch, in den gemergt wird
    into: String,
    /// Zielbranch der Tickets; `None` im Fallback (ausgecheckter Branch des Hauptordners)
    target: Option<String>,
    /// Selbst geöffneter Merge-Worktree: nach dem Merge wieder schließen
    opened: bool,
    /// Bezeichnung des Ordners für Meldungen („Hauptordner“, „Worktree <Pfad>“,
    /// „Merge-Worktree <Pfad>“)
    label: String,
}

/// Art des Grundes, warum der Merge auf seinen Ordner wartet (für „schon kommentiert?“)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HoldKind {
    Busy,
    Staged,
    Overlap,
    Leftover,
}

impl HoldKind {
    /// Schreibweise im Zustand (`heldBy`), wortgleich zu TypeScript
    fn as_str(self) -> &'static str {
        match self {
            HoldKind::Busy => "busy",
            HoldKind::Staged => "staged",
            HoldKind::Overlap => "overlap",
            HoldKind::Leftover => "leftover",
        }
    }
}

/// Warum Git den Merge im Ordner gerade abweisen würde
#[derive(Debug, Clone)]
struct HoldReason {
    kind: HoldKind,
    text: String,
}

/// Entweder der Ort des Merges oder der Grund, warum dort nicht gemergt werden kann
type PlaceOrProblem = std::result::Result<MergePlace, String>;

/// Mergt erledigte Tickets in ihren Zielbranch (ohne Zielbranch in den ausgecheckten Branch des
/// Hauptordners), startet bei Konflikten eine Merge-Session im Worktree und räumt nach dem
/// Merge auf.
#[derive(Debug, Clone)]
pub struct MergeService {
    git: Arc<dyn MergeGit>,
    window: TimeWindowCheck,
    keys: PathKey,
}

impl Default for MergeService {
    fn default() -> Self {
        Self::new()
    }
}

impl MergeService {
    /// Merge-Dienst mit den gewöhnlichen Git-Aufrufern
    pub fn new() -> Self {
        Self::with_git(Arc::new(RealMergeGit::new()))
    }

    /// Merge-Dienst mit eigener Git-Seite (Tests)
    pub fn with_git(git: Arc<dyn MergeGit>) -> Self {
        Self {
            git,
            window: TimeWindowCheck,
            keys: PathKey::new(),
        }
    }

    // ------------------------------------------------------------------------------------
    // Durchlauf
    // ------------------------------------------------------------------------------------

    /// Alle offenen Aufträge, je Branch höchstens einmal (laufende Merge-Sessions warten auf ihr
    /// Ergebnis). Verbindungsfehler brechen den Durchlauf ab, fachliche Fehler landen je Branch
    /// im Bericht.
    pub async fn process_all(&self, ctx: &DispatchContext) -> Result<MergeStepReport> {
        let merges = ctx.stores.state.read()?.merges;
        let mut branches: Vec<String> = Vec::new();
        for entry in merges.values() {
            if !branches.contains(&entry.branch) {
                branches.push(entry.branch.clone());
            }
        }
        let mut report = MergeStepReport::default();
        for branch in branches {
            match self.process_branch(ctx, &branch).await {
                Ok(merged) => report.merged.extend(merged),
                Err(error) => {
                    if BoardRuntimeHolder::is_connection_error(&error) {
                        return Err(error);
                    }
                    let ids: Vec<String> = merges
                        .values()
                        .filter(|entry| entry.branch == branch)
                        .map(|entry| entry.ticket_id.clone())
                        .collect();
                    report.errors.push(format!("{}: {error}", ids.join(", ")));
                }
            }
        }
        Ok(report)
    }

    /// Nach der Merge-Session: Ziel enthalten und Worktree sauber → erneut mergen (im selben
    /// Durchlauf), sonst Blockiert. Fachliche Fehler beim Auswerten blockieren ebenfalls (sonst
    /// bliebe der Auftrag hängen); Verbindungsfehler gehen weiter.
    pub async fn after_session(
        &self,
        ctx: &DispatchContext,
        record: &RunRecord,
        end: &RunEnd,
    ) -> Result<()> {
        let Err(error) = self.evaluate_session(ctx, record, end).await else {
            return Ok(());
        };
        if BoardRuntimeHolder::is_connection_error(&error) {
            return Err(error);
        }
        let job = self.job(ctx, &record.branch).await?;
        if job.tickets.is_empty() {
            self.drop_all(ctx, &job).await?;
        } else {
            self.block(
                ctx,
                &job,
                None,
                &format!("Die Merge-Session ließ sich nicht auswerten: {error}"),
            )
            .await?;
        }
        Ok(())
    }

    /// „Erneut mergen“: nur Erledigt mit Branch oder Blockiert mit Grund „Merge: …“ auf einem
    /// eigenen Board, nicht während eines Merge-Laufs; setzt alle Aufträge des Branches zurück
    /// (auch einen hängenden in Phase „Session“), erlaubt wieder eine Merge-Session; liefert die
    /// Ticket-ID
    pub async fn retry(
        &self,
        ctx: &DispatchContext,
        id: &str,
        actor: &BoardActor,
    ) -> Result<String> {
        let detail = ctx.services().tickets.get(id).await?;
        let ticket = &detail.ticket;
        let merge_blocked = ticket.status == TicketStatus::Blocked
            && ticket
                .blocked_reason
                .as_deref()
                .unwrap_or_default()
                .starts_with(MERGE_REASON_PREFIX);
        let Some(branch) = ticket
            .branch
            .clone()
            .filter(|_| merge_blocked || ticket.status == TicketStatus::Done)
        else {
            return Err(HarnessError::new(
                "merge-not-possible",
                format!(
                    "{} wartet auf keinen Merge (nur Erledigt mit Branch oder Blockiert mit \
Grund „Merge: …“).",
                    ticket.id
                ),
            )
            .into());
        };
        let board = ctx.board(ticket.board_id).await?;
        if board.is_some_and(|board| board.owner_id != ctx.me.user_id) {
            return Err(HarnessError::new(
                "merge-not-possible",
                format!("{} liegt auf dem Board eines anderen Benutzers.", ticket.id),
            )
            .into());
        }
        let entries = entries_of(&ctx.stores.state.read()?, &branch);
        if self.session_running(ctx, &branch, &entries)? {
            return Err(HarnessError::new(
                "merge-not-possible",
                format!("{}: Die Merge-Session läuft noch.", ticket.id),
            )
            .into());
        }
        if merge_blocked {
            ctx.services()
                .tickets
                .move_to(
                    &ticket.id,
                    TicketStatus::Done,
                    actor,
                    &MoveOptions {
                        force: true,
                        ..MoveOptions::default()
                    },
                )
                .await?;
        }
        let since = ctx.now_iso();
        let id = ticket.id.clone();
        let fresh = MergeEntry {
            ticket_id: id.clone(),
            branch: branch.clone(),
            phase: MergePhase::Pending,
            session_used: false,
            wait_noted: false,
            target: None,
            conflict_seen: Some(false),
            held_by: None,
            since,
        };
        ctx.stores
            .state
            .update(move |state| {
                for entry in state.merges.values_mut() {
                    if entry.branch != branch {
                        continue;
                    }
                    entry.phase = MergePhase::Pending;
                    entry.session_used = false;
                    entry.target = None;
                    entry.conflict_seen = Some(false);
                    entry.held_by = Some(None);
                }
                state.merges.insert(id.clone(), fresh.clone());
            })
            .await?;
        ctx.comment(
            &ticket.id,
            &format!(
                "{}: erneuter Merge angestoßen von {}.",
                markers::MERGE,
                actor.name
            ),
        )
        .await?;
        Ok(ticket.id.clone())
    }

    /// „Abnehmen“ (Mensch) eines Tickets, das mit Grund „Human Interaction: …“ blockiert ist:
    /// Erledigt (Abnahme-Text Pflicht), Grund leeren, Verlauf; liegt es mit Branch auf einem
    /// eigenen Board mit Repo, hakt der Harness die Aufgabe ab und stößt den Merge an wie
    /// „Erneut mergen“ (`merge: true`). Sonst wird es nur erledigt; `text` sagt dann, warum
    /// nichts gemergt wird
    pub async fn accept(
        &self,
        ctx: &DispatchContext,
        id: &str,
        actor: &BoardActor,
    ) -> Result<AcceptOutcome> {
        let detail = ctx.services().tickets.get(id).await?;
        let ticket = detail.ticket.clone();
        if ticket.status != TicketStatus::Blocked
            || !is_human_interaction(ticket.blocked_reason.as_deref())
        {
            return Err(HarnessError::new(
                "accept-not-possible",
                format!(
                    "{} wartet auf keine Abnahme (nur Blockiert mit Grund \
„{HUMAN_INTERACTION_PREFIX} …“).",
                    ticket.id
                ),
            )
            .into());
        }
        let board = ctx.board(ticket.board_id).await?;
        let own = board
            .as_ref()
            .filter(|board| board.owner_id == ctx.me.user_id && board.repo_path.is_some());
        if let (Some(board), Some(_)) = (own, ticket.branch.as_deref()) {
            let work = ctx.repo(board).await?;
            work.manager
                .accept(
                    &ticket.id,
                    Some(&RepoBoard {
                        id: work.board.id,
                        name: work.board.name.clone(),
                    }),
                    Path::new(&work.root),
                    actor,
                )
                .await?;
            self.retry(ctx, &ticket.id, actor).await?;
            return Ok(AcceptOutcome {
                ticket_id: ticket.id,
                merge: true,
                text: String::new(),
            });
        }
        ctx.services()
            .tickets
            .accept(&ticket.id, actor, &WriteOptions::default())
            .await?;
        let why = if ticket.branch.is_none() {
            "kein Branch eingetragen, es gibt nichts zu mergen"
        } else {
            "das Board gehört einem anderen Benutzer oder hat keinen Repo-Pfad – gemergt wird \
dort („Mergen“ bzw. harness board merge)"
        };
        Ok(AcceptOutcome {
            text: format!("{} abgenommen und erledigt; {why}.", ticket.id),
            ticket_id: ticket.id,
            merge: false,
        })
    }

    // ------------------------------------------------------------------------------------
    // Merge-Session
    // ------------------------------------------------------------------------------------

    /// Auswertung einer Merge-Session; steht das Ticket nicht mehr auf Erledigt mit dem Branch
    /// des Laufs, wird nur sein Auftrag entfernt
    async fn evaluate_session(
        &self,
        ctx: &DispatchContext,
        record: &RunRecord,
        end: &RunEnd,
    ) -> Result<()> {
        if entries_of(&ctx.stores.state.read()?, &record.branch).is_empty() {
            return Ok(());
        }
        let detail = load(ctx, &record.ticket_id).await?;
        let stale = match &detail {
            Some(detail) => {
                detail.ticket.status != TicketStatus::Done
                    || detail.ticket.branch.as_deref() != Some(record.branch.as_str())
            }
            None => true,
        };
        if stale {
            self.drop_one(ctx, &record.ticket_id).await?;
            // Die übrigen Aufträge des Branches warten nicht mehr auf diese Session
            self.set_phase(ctx, &record.branch, MergePhase::Pending)
                .await?;
            return Ok(());
        }
        let detail = detail.expect("Ticket der Merge-Session");
        let job = self.job(ctx, &record.branch).await?;
        if job.tickets.is_empty() {
            return self.drop_all(ctx, &job).await;
        }
        let board = ctx.board(detail.ticket.board_id).await?;
        let work = match board.as_ref().filter(|board| board.repo_path.is_some()) {
            Some(board) => Some(ctx.repo(board).await?),
            None => None,
        };
        let target = record
            .target
            .clone()
            .or_else(|| job.entries.iter().find_map(|entry| entry.target.clone()));
        let resolved = match (&work, &target) {
            (Some(work), Some(target)) => self.resolved(work, target, record)?,
            _ => false,
        };
        if !resolved {
            let exit = if end.result.timed_out {
                "Zeitlimit".to_string()
            } else {
                format!(
                    "Exit-Code {}",
                    end.result
                        .code
                        .map(|code| code.to_string())
                        .unwrap_or_else(|| "null".to_string())
                )
            };
            self.block(
                ctx,
                &job,
                work.as_deref(),
                &format!(
                    "Die Merge-Session konnte den Konflikt nicht lösen ({exit}, Protokoll {}).",
                    file_name(&record.log_file)
                ),
            )
            .await?;
            return Ok(());
        }
        self.set_phase(ctx, &record.branch, MergePhase::Pending)
            .await
    }

    /// Hat die Merge-Session das Ziel in den Branch geholt und einen sauberen Worktree ohne
    /// laufende Git-Aktion hinterlassen? Beide als volle Branch-Referenz, damit ein
    /// gleichnamiges Tag nicht gewinnt
    fn resolved(&self, work: &RepoWork, target: &str, record: &RunRecord) -> Result<bool> {
        if !self.git.contains(
            Path::new(&work.root),
            &format!("refs/heads/{target}"),
            &format!("refs/heads/{}", record.branch),
        )? {
            return Ok(false);
        }
        // Worktree fehlt oder ist kaputt
        match self.git.inspect(Path::new(&record.cwd)) {
            Ok(state) => Ok(state.busy.is_none() && state.blocking_files.is_empty()),
            Err(_) => Ok(false),
        }
    }

    /// Merge-Session im Worktree des ersten Tickets starten; Phase, Session und Ziel gelten für
    /// alle Aufträge des Branches
    async fn start_session(
        &self,
        ctx: &DispatchContext,
        job: &BranchJob,
        work: &RepoWork,
        target: &str,
        files: &[String],
    ) -> Result<()> {
        let detail = &job.tickets[0];
        let cwd = work.manager.ensure_worktree(&detail.ticket.id).await?;
        let attempt = ctx.attempts().failed(&detail.comments) as i64 + 1;
        let chosen = ctx.chooser().choose(
            &nexus_core::dispatch::ProfileTicket::from(&detail.ticket),
            attempt,
        );
        let run = self.at_least_default(ctx, chosen);
        let agent = ctx.agent_name(&detail.ticket.id);
        let values = crate::dispatch::prompt_values::MergeValues {
            ticket_id: detail.ticket.id.clone(),
            title: detail.ticket.title.clone(),
            agent: agent.clone(),
            branch: job.branch.clone(),
            target: target.to_string(),
        };
        let prompt = ctx.prompts.merge(&values.input(&ctx.cfg));
        let profile = run.profile.clone();
        ctx.launch(LaunchInput {
            target: Some(target.to_string()),
            extra_tools: MERGE_SESSION_TOOLS.iter().map(|s| s.to_string()).collect(),
            ..LaunchInput::new(
                RunKind::Merge,
                &detail.ticket.id,
                work.board.id,
                &detail.ticket.title,
                &agent,
                cwd,
                &job.branch,
                run,
                prompt,
            )
        })
        .await?;
        let branch = job.branch.clone();
        let target_value = target.to_string();
        ctx.stores
            .state
            .update(move |state| {
                for entry in state.merges.values_mut() {
                    if entry.branch != branch {
                        continue;
                    }
                    entry.phase = MergePhase::Session;
                    entry.session_used = true;
                    entry.target = Some(target_value.clone());
                }
            })
            .await?;
        for detail in &job.tickets {
            ctx.comment(
                &detail.ticket.id,
                &format!(
                    "{} in {} beim Merge in {target} – Merge-Session gestartet (Profil {profile}).",
                    markers::MERGE_CONFLICT,
                    list(files)
                ),
            )
            .await?;
        }
        ctx.log(&format!(
            "Merge-Konflikt {} ({}) – Merge-Session gestartet",
            job.ids().join(", "),
            files.join(", ")
        ));
        Ok(())
    }

    /// Merge-Session mindestens mit dem Standardprofil (der Schattenbetrieb rechnet damit
    /// dieselbe Wahl nach)
    pub fn at_least_default(&self, ctx: &DispatchContext, run: RunSettings) -> RunSettings {
        let cfg = &ctx.cfg;
        let index = |name: &str| cfg.ladder.iter().position(|entry| entry == name);
        if index(&run.profile) >= index(&cfg.default_profile) {
            return run;
        }
        let Some(profile) = cfg.profiles.get(&cfg.default_profile) else {
            return run;
        };
        // Grund und Regelnummer bleiben die des gewählten Basisprofils – wie bei einer Eskalation
        RunSettings {
            model: profile.model.clone(),
            effort: profile.effort.clone(),
            max_turns: profile.max_turns,
            timeout_minutes: profile.timeout_minutes,
            profile: cfg.default_profile.clone(),
            why: format!(
                "{}, Merge-Session mindestens {}",
                run.why, cfg.default_profile
            ),
            reason: run.reason,
            config_hash: run.config_hash,
            rule_index: run.rule_index,
            escalated_from: None,
        }
    }

    // ------------------------------------------------------------------------------------
    // Ein Branch
    // ------------------------------------------------------------------------------------

    /// Alle Aufträge eines Branches; liefert die gemergten Tickets
    async fn process_branch(&self, ctx: &DispatchContext, branch: &str) -> Result<Vec<String>> {
        let state = ctx.stores.state.read()?;
        let entries = entries_of(&state, branch);
        if entries.is_empty() {
            return Ok(Vec::new());
        }
        if self.session_running(ctx, branch, &entries)? {
            return Ok(Vec::new());
        }
        let job = self.job(ctx, branch).await?;
        if job.tickets.is_empty() {
            self.drop_all(ctx, &job).await?;
            return Ok(Vec::new());
        }
        // Phase „Session“ ohne Merge-Lauf: die Auswertung fehlt (z. B. abgebrochen) → gilt als
        // ungelöst
        if job
            .entries
            .iter()
            .any(|entry| entry.phase == MergePhase::Session)
        {
            return self
                .block(ctx, &job, None, "Die Merge-Session endete ohne Auswertung.")
                .await;
        }
        let primary = job.tickets[0].ticket.clone();
        let board = ctx.board(primary.board_id).await?;
        let Some(board) = board.filter(|board| board.repo_path.is_some()) else {
            return self
                .block(ctx, &job, None, "Das Board hat keinen Repo-Pfad.")
                .await;
        };
        let work = match self.repo_of(ctx, &board).await? {
            Ok(work) => work,
            Err(problem) => return self.block(ctx, &job, None, &problem).await,
        };
        if !work.manager.is_ready(branch).await? {
            return self.wait(ctx, &job, &work, None).await;
        }
        // Die Bereitschaft ist eine Momentaufnahme aus „finish“: ein wieder geöffnetes Ticket des
        // Worktrees hält den Merge an
        let open = self.open_tickets(ctx, &job, &work).await?;
        if !open.is_empty() {
            return self.wait(ctx, &job, &work, Some(open)).await;
        }
        // Konflikt schon gesehen und die Merge-Session darf noch nicht starten: nicht jede Minute
        // mergen und abbrechen – mit Zielbranch schon vor dem Ort, sonst öffnete jeder Durchlauf
        // einen Merge-Worktree, nur um zu warten
        let not_now = self.session_not_allowed(ctx, &state);
        let conflict_waits = not_now.is_some()
            && job
                .entries
                .iter()
                .any(|entry| entry.conflict_seen == Some(true));
        if conflict_waits && primary.target_branch.is_some() {
            return Ok(Vec::new());
        }
        let place = match self.place_of(ctx, &job, &work).await? {
            Ok(place) => place,
            Err(problem) => return self.block(ctx, &job, Some(&work), &problem).await,
        };
        // Merge schon im Ziel, Branch gelöscht (Abbruch zwischen Merge und Aufräumen):
        // abschließen statt erneut mergen
        if let Some(earlier) = self.git.merged_earlier(
            Path::new(&work.root),
            branch,
            &format!("refs/heads/{}", place.into),
        ) {
            let warning = self.close(ctx, &work, &place);
            return self
                .finish(ctx, &job, &work, &place, &earlier, warning)
                .await;
        }
        // Lokale Arbeit im Ordner hält den Merge nur an, wenn Git ihn sonst abweisen würde; dann
        // wird gewartet, nicht blockiert
        let held = self.hold_reason(&place, branch)?;
        if held.is_some() || conflict_waits {
            // Ein Merge-Worktree mit liegengebliebenen Änderungen lässt sich nicht entfernen; er
            // bleibt, bis jemand aufräumt
            if held.as_ref().map(|held| held.kind) != Some(HoldKind::Leftover) {
                self.close(ctx, &work, &place);
            }
            return match held {
                Some(held) => self.hold(ctx, &job, &place, &held).await,
                None => Ok(Vec::new()),
            };
        }
        let message = self.merge_message(ctx, &job, &work)?;
        let outcome = match self.git.merge(
            Path::new(&place.path),
            branch,
            Some(&message),
            ctx.cfg.merge_strategy,
        ) {
            Ok(outcome) => outcome,
            // Der eigene Merge ließ sich nicht zurücksetzen: der Ordner braucht einen Menschen
            Err(error) if error.code() == Some("merge-abort-failed") => {
                let problem = abort_failure(&error, &place);
                return self.block(ctx, &job, Some(&work), &problem).await;
            }
            Err(error) => return Err(error),
        };
        // Die Dateiliste einer Abweisung braucht den Ordner noch; danach wird ein selbst
        // geöffneter Merge-Worktree geschlossen
        let refused_overlap = match &outcome {
            MergeOutcome::Refused { .. } => self.overlapping(&place.path, branch),
            _ => Vec::new(),
        };
        let close_warning = self.close(ctx, &work, &place);
        match outcome {
            MergeOutcome::Merged { commit } => {
                self.finish(ctx, &job, &work, &place, &commit, close_warning)
                    .await
            }
            MergeOutcome::Refused { message } => {
                if !refused_overlap.is_empty() {
                    return self
                        .hold(
                            ctx,
                            &job,
                            &place,
                            &HoldReason {
                                kind: HoldKind::Overlap,
                                text: overlap_text(&refused_overlap),
                            },
                        )
                        .await;
                }
                let problem = refusal(&message, &refused_overlap);
                self.block(ctx, &job, Some(&work), &problem).await
            }
            MergeOutcome::Conflict { files } => {
                if job.entries.iter().any(|entry| entry.session_used) {
                    return self
                        .block(
                            ctx,
                            &job,
                            Some(&work),
                            &format!(
                                "Merge-Konflikt in {} – die Merge-Session hat ihn nicht gelöst.",
                                list(&files)
                            ),
                        )
                        .await;
                }
                // Ausgeschaltet, Pause oder außerhalb des Zeitfensters: keine Merge-Session; der
                // Auftrag bleibt und merkt sich den Konflikt
                if let Some(not_now) = not_now {
                    let branch = branch.to_string();
                    ctx.stores
                        .state
                        .update(move |state| {
                            for entry in state.merges.values_mut() {
                                if entry.branch == branch {
                                    entry.conflict_seen = Some(true);
                                }
                            }
                        })
                        .await?;
                    ctx.log(&format!(
                        "Merge-Konflikt {} – {not_now}, Merge-Session wartet",
                        primary.id
                    ));
                    return Ok(Vec::new());
                }
                if let Err(error) = self
                    .start_session(ctx, &job, &work, &place.into, &files)
                    .await
                {
                    // Ohne Session käme der Auftrag jede Minute wieder; fachliche Fehler
                    // blockieren daher das Ticket
                    if BoardRuntimeHolder::is_connection_error(&error) {
                        return Err(error);
                    }
                    return self
                        .block(
                            ctx,
                            &job,
                            Some(&work),
                            &format!(
                                "Merge-Konflikt in {}; die Merge-Session startete nicht: {error}",
                                list(&files)
                            ),
                        )
                        .await;
                }
                Ok(Vec::new())
            }
        }
    }

    /// Ort des Merges: ohne Zielbranch der Hauptordner mit seinem ausgecheckten Branch
    /// (bisheriger Ablauf), sonst der des Zielbranches; liefert den Grund, warum dort nicht
    /// gemergt werden kann (ob Git den Merge gerade zulässt, prüft `hold_reason`)
    async fn place_of(
        &self,
        ctx: &DispatchContext,
        job: &BranchJob,
        work: &RepoWork,
    ) -> Result<PlaceOrProblem> {
        if let Some(target) = job.tickets[0].ticket.target_branch.clone() {
            return self.target_place(ctx, job, work, &target).await;
        }
        let Some(branch) = self.git.inspect(Path::new(&work.root))?.branch else {
            return Ok(Err(
                "Der Hauptordner steht auf keinem Branch (Detached HEAD).".to_string(),
            ));
        };
        let solved = job.entries.iter().find_map(|entry| entry.target.clone());
        if let Some(solved) = solved.filter(|solved| *solved != branch) {
            return Ok(Err(format!(
                "Zielbranch gewechselt (Konflikt gelöst gegen {solved}, ausgecheckt ist \
{branch}) – bitte Erneut mergen."
            )));
        }
        Ok(Ok(MergePlace {
            path: work.root.clone(),
            into: branch,
            target: None,
            opened: false,
            label: MAIN_LABEL.to_string(),
        }))
    }

    /// Zielbranch anlegen, falls er fehlt (Kommentar an alle Tickets), dann im Hauptordner, in
    /// einem anderen Worktree des Zielbranches oder im Merge-Worktree mergen
    async fn target_place(
        &self,
        ctx: &DispatchContext,
        job: &BranchJob,
        work: &RepoWork,
        target: &str,
    ) -> Result<PlaceOrProblem> {
        let root = Path::new(&work.root);
        // Git-Fehler (z. B. Ref-Konflikt beim Anlegen) blockieren, sonst käme der Auftrag jede
        // Minute wieder
        let ensured = match self.git.ensure_target(root, target) {
            Ok(ensured) => ensured,
            Err(error) if BoardRuntimeHolder::is_connection_error(&error) => return Err(error),
            Err(error) => return Ok(Err(format!("Zielbranch {target} nicht nutzbar: {error}"))),
        };
        let found = match self.git.checked_out_at(root, target) {
            Ok(found) => found,
            Err(error) if BoardRuntimeHolder::is_connection_error(&error) => return Err(error),
            Err(error) => return Ok(Err(format!("Zielbranch {target} nicht nutzbar: {error}"))),
        };
        if ensured.created {
            for detail in &job.tickets {
                ctx.comment(
                    &detail.ticket.id,
                    &format!(
                        "{}: Zielbranch {target} neu angelegt von {}.",
                        markers::MERGE,
                        ensured.from.clone().unwrap_or_default()
                    ),
                )
                .await?;
            }
        }
        // Ein eingetragener, aber verschwundener Worktree gilt als nicht ausgecheckt
        // (open_merge_worktree räumt ihn per prune weg)
        let at = found.filter(|path| Path::new(path).exists());
        let mut place = MergePlace {
            path: work.root.clone(),
            into: target.to_string(),
            target: Some(target.to_string()),
            opened: false,
            label: MAIN_LABEL.to_string(),
        };
        let merge_worktree = self.git.merge_worktree_path(root, target);
        match &at {
            Some(at) if !self.keys.same(at, &work.root) && !self.keys.same(at, &merge_worktree) => {
                place.label = format!("Worktree {at}");
                place.path = at.clone();
            }
            at if at.is_none() || !self.keys.same(at.as_ref().expect("Ordner"), &work.root) => {
                match self.git.open_merge_worktree(root, target) {
                    Ok(path) => {
                        place.label = format!("Merge-Worktree {path}");
                        place.path = path;
                        place.opened = true;
                    }
                    Err(error) if BoardRuntimeHolder::is_connection_error(&error) => {
                        return Err(error)
                    }
                    Err(error) => return Ok(Err(error.to_string())),
                }
            }
            _ => {}
        }
        Ok(Ok(place))
    }

    /// Warum Git den Merge im Ordner gerade abweisen würde (laufende Git-Aktion, vorgemerkte oder
    /// mit dem Branch überschneidende lokale Änderungen) oder ein selbst geöffneter
    /// Merge-Worktree nach dem Merge nicht entfernt werden könnte (liegengebliebene Änderungen
    /// eines früheren Versuchs), sonst `None`
    fn hold_reason(&self, place: &MergePlace, branch: &str) -> Result<Option<HoldReason>> {
        let path = Path::new(&place.path);
        let state = self.git.inspect(path)?;
        if let Some(busy) = state.busy {
            let text = if place.label == MAIN_LABEL {
                busy
            } else {
                format!(
                    "Im {} läuft gerade {}.",
                    place.label,
                    self.git
                        .running(path)?
                        .unwrap_or_else(|| "eine Git-Aktion".to_string())
                )
            };
            return Ok(Some(HoldReason {
                kind: HoldKind::Busy,
                text,
            }));
        }
        if place.opened && !state.blocking_files.is_empty() {
            return Ok(Some(HoldReason {
                kind: HoldKind::Leftover,
                text: format!(
                    "liegengebliebene Änderungen in {}",
                    list(&state.blocking_files)
                ),
            }));
        }
        if !state.staged_files.is_empty() {
            return Ok(Some(HoldReason {
                kind: HoldKind::Staged,
                text: format!(
                    "vorgemerkte Änderungen (git add) in {}",
                    list(&state.staged_files)
                ),
            }));
        }
        let overlap = self.overlapping(&place.path, branch);
        Ok(if overlap.is_empty() {
            None
        } else {
            Some(HoldReason {
                kind: HoldKind::Overlap,
                text: overlap_text(&overlap),
            })
        })
    }

    /// Schließt einen selbst geöffneten Merge-Worktree; liefert die Warnung, wenn er stehen
    /// bleibt, sonst `None`
    fn close(&self, ctx: &DispatchContext, work: &RepoWork, place: &MergePlace) -> Option<String> {
        if !place.opened {
            return None;
        }
        let failure = self
            .git
            .close_merge_worktree(Path::new(&work.root), &place.path)?;
        let warning = format!(
            "Merge-Worktree {} nicht entfernt: {}",
            place.path,
            one_line(&failure)
        );
        ctx.log(&warning);
        Some(warning)
    }

    /// Merge-Nachricht: ein Ticket „Merge <ID> „<Titel>“ (<branch>)“, mehrere Tickets des
    /// Worktrees „Merge <ID>, <ID> (<branch>)“; bei Squash „<ID> „<Titel>“ (squash von
    /// <branch>)“ bzw. „<ID>, <ID> (squash von <branch>)“
    fn merge_message(
        &self,
        ctx: &DispatchContext,
        job: &BranchJob,
        work: &RepoWork,
    ) -> Result<String> {
        let mut ids: Vec<String> = Vec::new();
        for id in work
            .manager
            .tickets_of(&job.branch)?
            .into_iter()
            .chain(job.ids())
        {
            if !ids.contains(&id) {
                ids.push(id);
            }
        }
        let head = if ids.len() == 1 {
            format!("{} „{}“", ids[0], job.tickets[0].ticket.title)
        } else {
            ids.join(", ")
        };
        Ok(if ctx.cfg.merge_strategy == MergeStrategy::Squash {
            format!("{head} (squash von {})", job.branch)
        } else {
            format!("Merge {head} ({})", job.branch)
        })
    }

    /// Hauptordner des Boards oder die Meldung, warum der Repo-Pfad nicht nutzbar ist
    /// (Verbindungsfehler gehen weiter)
    async fn repo_of(
        &self,
        ctx: &DispatchContext,
        board: &Board,
    ) -> Result<std::result::Result<Arc<RepoWork>, String>> {
        match ctx.repo(board).await {
            Ok(work) => Ok(Ok(work)),
            Err(error) if BoardRuntimeHolder::is_connection_error(&error) => Err(error),
            Err(error) => Ok(Err(format!(
                "Repo-Pfad {} nicht nutzbar: {error}",
                board.repo_path.clone().unwrap_or_default()
            ))),
        }
    }

    /// Warum gerade keine Merge-Session starten darf (ausgeschaltet, Pause, außerhalb des
    /// Zeitfensters), sonst `None`
    fn session_not_allowed(&self, ctx: &DispatchContext, state: &DispatchState) -> Option<String> {
        let now = ctx.now();
        if !ctx.cfg.enabled {
            return Some("Dispatcher ausgeschaltet".to_string());
        }
        if state
            .paused_until
            .as_deref()
            .and_then(|at| at.parse::<chrono::DateTime<chrono::Utc>>().ok())
            .is_some_and(|until| until > now)
        {
            return Some("Pause aktiv".to_string());
        }
        if !self.window.contains(
            now.with_timezone(&chrono::Local).time(),
            ctx.cfg.window.as_ref(),
        ) {
            return Some("außerhalb des Zeitfensters".to_string());
        }
        None
    }

    /// Tickets des Worktrees, die noch diesen Branch haben und wieder offen sind (nicht Erledigt,
    /// nicht „Nicht mehr nötig“, nicht wegen des Merges blockiert)
    async fn open_tickets(
        &self,
        ctx: &DispatchContext,
        job: &BranchJob,
        work: &RepoWork,
    ) -> Result<Vec<String>> {
        let mut open = Vec::new();
        for id in work.manager.tickets_of(&job.branch)? {
            if job.tickets.iter().any(|detail| detail.ticket.id == id) {
                continue;
            }
            let Some(detail) = load(ctx, &id).await? else {
                continue;
            };
            let ticket = &detail.ticket;
            if ticket.branch.as_deref() != Some(job.branch.as_str())
                || ticket.status == TicketStatus::Done
                || ticket.status == TicketStatus::NotNeeded
            {
                continue;
            }
            if ticket.status == TicketStatus::Blocked
                && ticket
                    .blocked_reason
                    .as_deref()
                    .unwrap_or_default()
                    .starts_with(MERGE_REASON_PREFIX)
            {
                continue;
            }
            open.push(ticket.id.clone());
        }
        Ok(open)
    }

    /// Dateien, an denen ein abgewiesener Merge scheitert; lässt sich das nicht ermitteln, keine
    /// (dann zählt die Git-Meldung)
    fn overlapping(&self, root: &str, branch: &str) -> Vec<String> {
        self.git
            .overlapping(Path::new(root), branch)
            .unwrap_or_default()
    }

    // ------------------------------------------------------------------------------------
    // Warten, Blockieren, Abschließen
    // ------------------------------------------------------------------------------------

    /// Worktree noch nicht bereit oder ein Ticket wieder offen: je Ticket einmal kommentieren,
    /// dann still warten
    async fn wait(
        &self,
        ctx: &DispatchContext,
        job: &BranchJob,
        work: &RepoWork,
        open: Option<Vec<String>>,
    ) -> Result<Vec<String>> {
        let in_worktree = match open {
            Some(open) => open,
            None => work.manager.tickets_of(&job.branch)?,
        };
        for entry in &job.entries {
            if entry.wait_noted
                || !job
                    .tickets
                    .iter()
                    .any(|detail| detail.ticket.id == entry.ticket_id)
            {
                continue;
            }
            let others: Vec<&String> = in_worktree
                .iter()
                .filter(|id| **id != entry.ticket_id)
                .collect();
            let names = if others.is_empty() {
                "weitere Tickets derselben Spec".to_string()
            } else {
                others
                    .iter()
                    .map(|id| id.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            ctx.comment(
                &entry.ticket_id,
                &format!(
                    "{} auf {names} – der Branch wird gemergt, sobald alle Tickets des \
Worktrees fertig sind.",
                    markers::MERGE_WAIT
                ),
            )
            .await?;
        }
        let branch = job.branch.clone();
        ctx.stores
            .state
            .update(move |state| {
                for entry in state.merges.values_mut() {
                    if entry.branch != branch {
                        continue;
                    }
                    entry.phase = MergePhase::Waiting;
                    entry.wait_noted = true;
                }
            })
            .await?;
        Ok(Vec::new())
    }

    /// Der Ordner lässt den Merge gerade nicht zu: je Art des Grundes einmal kommentieren, dann
    /// still warten – eine andere Dateiliste desselben Grundes kommentiert nicht erneut. Jeder
    /// Durchlauf prüft erneut und mergt, sobald der Grund weg ist (committet, verworfen,
    /// Git-Aktion beendet)
    async fn hold(
        &self,
        ctx: &DispatchContext,
        job: &BranchJob,
        place: &MergePlace,
        reason: &HoldReason,
    ) -> Result<Vec<String>> {
        let kind = reason.kind.as_str();
        let fresh: Vec<&MergeEntry> = job
            .entries
            .iter()
            .filter(|entry| entry.held_by.clone().flatten().as_deref() != Some(kind))
            .collect();
        if fresh.is_empty() {
            return Ok(Vec::new());
        }
        for entry in fresh {
            ctx.comment(
                &entry.ticket_id,
                &format!(
                    "{} auf den {}: {} – gemergt wird automatisch, sobald das erledigt ist.",
                    markers::MERGE_WAIT,
                    place.label,
                    reason.text
                ),
            )
            .await?;
        }
        let branch = job.branch.clone();
        let kind = kind.to_string();
        ctx.stores
            .state
            .update(move |state| {
                for entry in state.merges.values_mut() {
                    if entry.branch != branch {
                        continue;
                    }
                    entry.phase = MergePhase::Waiting;
                    entry.held_by = Some(Some(kind.clone()));
                }
            })
            .await?;
        ctx.log(&format!(
            "Merge wartet {}: {}",
            job.ids().join(", "),
            reason.text
        ));
        Ok(Vec::new())
    }

    /// Gemergt: aufräumen, Merge-blockierte Tickets des Worktrees wieder auf Erledigt, alle
    /// kommentieren, Aufträge entfernen, melden (ein nicht entfernter Merge-Worktree zählt als
    /// unvollständiges Aufräumen)
    async fn finish(
        &self,
        ctx: &DispatchContext,
        job: &BranchJob,
        work: &RepoWork,
        place: &MergePlace,
        commit: &str,
        close_warning: Option<String>,
    ) -> Result<Vec<String>> {
        let primary = job.tickets[0].ticket.clone();
        let target = place.into.clone();
        let cleaned = work
            .manager
            .remove_merged(&job.branch, ctx.actor(), place.target.as_deref())
            .await?;
        let mut warnings = cleaned.warnings.clone();
        if let Some(warning) = close_warning {
            warnings.push(warning);
        }
        let mut ids: Vec<String> = Vec::new();
        for id in cleaned.tickets.iter().cloned().chain(job.ids()) {
            if !ids.contains(&id) {
                ids.push(id);
            }
        }
        let head = format!("{} in {target} ({commit}).", markers::MERGED);
        let lines = if warnings.is_empty() {
            let removed = match &cleaned.removed {
                Some(path) => format!(", Worktree {path} entfernt"),
                None => String::new(),
            };
            vec![format!("{head} Branch {} aufgeräumt{removed}.", job.branch)]
        } else {
            let mut lines = vec![format!("{head} Aufräumen unvollständig:")];
            lines.extend(warnings);
            lines
        };
        let reopened = self.reopen_merge_blocked(ctx, &ids).await?;
        for id in &ids {
            let mut body = lines.clone();
            if reopened.contains(id) {
                body.push(
                    "Das Ticket war wegen des Merges blockiert und steht wieder auf Erledigt."
                        .to_string(),
                );
            }
            ctx.comment(id, &body.join("\n")).await?;
        }
        self.drop_all(ctx, job).await?;
        for id in &ids {
            ctx.stores.reviews.remove(id)?;
        }
        ctx.log(&format!(
            "gemergt {} in {target} ({commit})",
            ids.join(", ")
        ));
        ctx.notify(&DispatchNotice {
            kind: DispatchNoticeKind::Merged,
            ticket_id: Some(primary.id.clone()),
            title: primary.title.clone(),
            text: format!("in {target} gemergt ({commit})"),
            repo_path: work.board.repo_path.clone(),
        })
        .await;
        Ok(ids)
    }

    /// Tickets, die mit Grund „Merge: …“ auf Blockiert stehen, wieder auf Erledigt (force);
    /// liefert deren IDs
    async fn reopen_merge_blocked(
        &self,
        ctx: &DispatchContext,
        ids: &[String],
    ) -> Result<Vec<String>> {
        let mut reopened = Vec::new();
        for id in ids {
            let Some(detail) = load(ctx, id).await? else {
                continue;
            };
            if detail.ticket.status != TicketStatus::Blocked
                || !detail
                    .ticket
                    .blocked_reason
                    .as_deref()
                    .unwrap_or_default()
                    .starts_with(MERGE_REASON_PREFIX)
            {
                continue;
            }
            ctx.services()
                .tickets
                .move_to(
                    id,
                    TicketStatus::Done,
                    ctx.actor(),
                    &MoveOptions {
                        force: true,
                        ..MoveOptions::default()
                    },
                )
                .await?;
            reopened.push(id.clone());
        }
        Ok(reopened)
    }

    /// Blockiert mit Grund „Merge: …“ alle erledigten Tickets des Worktrees (ohne
    /// Worktree-Verwaltung: die der Aufträge), Kommentar je Ticket, Aufträge des Branches
    /// entfernen, melden
    async fn block(
        &self,
        ctx: &DispatchContext,
        job: &BranchJob,
        work: Option<&RepoWork>,
        problem: &str,
    ) -> Result<Vec<String>> {
        let full = one_line(&format!("{MERGE_REASON_PREFIX} {problem}"));
        let reason = shorten(&full, MAX_BLOCKED_REASON_LENGTH);
        let mut ids: Vec<String> = job.ids();
        if let Some(work) = work {
            for id in work.manager.tickets_of(&job.branch)? {
                if !ids.contains(&id) {
                    ids.push(id);
                }
            }
        }
        let mut blocked: Vec<nexus_core::board::Ticket> = Vec::new();
        for id in &ids {
            let found = match job.tickets.iter().find(|detail| detail.ticket.id == *id) {
                Some(detail) => Some(detail.clone()),
                None => load(ctx, id).await?,
            };
            let Some(detail) = found else {
                continue;
            };
            let ticket = &detail.ticket;
            if ticket.status != TicketStatus::Done
                || ticket.branch.as_deref() != Some(job.branch.as_str())
            {
                continue;
            }
            ctx.services()
                .tickets
                .update(
                    &ticket.id,
                    &TicketPatch {
                        status: Some(TicketStatus::Blocked),
                        blocked_reason: Some(Some(reason.clone())),
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
                &ticket.id,
                &format!("{} nicht möglich: {problem}", markers::MERGE),
            )
            .await?;
            blocked.push(ticket.clone());
        }
        self.drop_all(ctx, job).await?;
        let Some(first) = blocked.first().cloned() else {
            return Ok(Vec::new());
        };
        ctx.log(&format!(
            "Merge blockiert {}: {problem}",
            blocked
                .iter()
                .map(|ticket| ticket.id.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ));
        let board = ctx.board(first.board_id).await?;
        ctx.notify(&DispatchNotice {
            kind: DispatchNoticeKind::Blocked,
            ticket_id: Some(first.id.clone()),
            title: first.title.clone(),
            text: reason,
            repo_path: board.and_then(|board| board.repo_path),
        })
        .await;
        Ok(Vec::new())
    }

    // ------------------------------------------------------------------------------------
    // Aufträge im Zustand
    // ------------------------------------------------------------------------------------

    /// Aufträge des Branches und ihre Tickets; Aufträge, deren Ticket fehlt, nicht mehr erledigt
    /// ist oder den Branch wechselte, fallen weg
    async fn job(&self, ctx: &DispatchContext, branch: &str) -> Result<BranchJob> {
        let entries = entries_of(&ctx.stores.state.read()?, branch);
        let mut tickets: Vec<TicketDetail> = Vec::new();
        for entry in &entries {
            let detail = load(ctx, &entry.ticket_id).await?;
            match detail.filter(|detail| {
                detail.ticket.status == TicketStatus::Done
                    && detail.ticket.branch.as_deref() == Some(branch)
            }) {
                Some(detail) => tickets.push(detail),
                None => self.drop_one(ctx, &entry.ticket_id).await?,
            }
        }
        let kept: BTreeSet<String> = tickets
            .iter()
            .map(|detail| detail.ticket.id.clone())
            .collect();
        Ok(BranchJob {
            branch: branch.to_string(),
            entries: entries
                .into_iter()
                .filter(|entry| kept.contains(&entry.ticket_id))
                .collect(),
            tickets,
        })
    }

    /// Läuft für den Branch oder eines seiner Tickets eine Merge-Session?
    fn session_running(
        &self,
        ctx: &DispatchContext,
        branch: &str,
        entries: &[MergeEntry],
    ) -> Result<bool> {
        let ids: BTreeSet<&str> = entries
            .iter()
            .map(|entry| entry.ticket_id.as_str())
            .collect();
        Ok(ctx.stores.runs.list()?.iter().any(|run| {
            run.kind == RunKind::Merge
                && (run.branch == branch || ids.contains(run.ticket_id.as_str()))
        }))
    }

    /// Setzt die Phase aller Aufträge eines Branches
    async fn set_phase(
        &self,
        ctx: &DispatchContext,
        branch: &str,
        phase: MergePhase,
    ) -> Result<()> {
        let branch = branch.to_string();
        ctx.stores
            .state
            .update(move |state| {
                for entry in state.merges.values_mut() {
                    if entry.branch == branch {
                        entry.phase = phase;
                    }
                }
            })
            .await?;
        Ok(())
    }

    /// Entfernt den Auftrag eines Tickets
    async fn drop_one(&self, ctx: &DispatchContext, ticket_id: &str) -> Result<()> {
        let ticket_id = ticket_id.to_string();
        ctx.stores
            .state
            .update(move |state| {
                state.merges.shift_remove(&ticket_id);
            })
            .await?;
        Ok(())
    }

    /// Entfernt alle Aufträge des Branches
    async fn drop_all(&self, ctx: &DispatchContext, job: &BranchJob) -> Result<()> {
        let branch = job.branch.clone();
        ctx.stores
            .state
            .update(move |state| {
                state.merges.retain(|_, entry| entry.branch != branch);
            })
            .await?;
        Ok(())
    }
}

#[async_trait]
impl MergeSessionEnd for MergeService {
    async fn session_ended(
        &self,
        ctx: &DispatchContext,
        record: &RunRecord,
        end: &RunEnd,
    ) -> Result<()> {
        self.after_session(ctx, record, end).await
    }
}

#[async_trait]
impl MergeStep for MergeService {
    async fn process_all(&self, ctx: &DispatchContext) -> Result<MergeStepReport> {
        MergeService::process_all(self, ctx).await
    }

    async fn retry(&self, ctx: &DispatchContext, id: &str, actor: &BoardActor) -> Result<String> {
        MergeService::retry(self, ctx, id, actor).await
    }

    async fn accept(
        &self,
        ctx: &DispatchContext,
        id: &str,
        actor: &BoardActor,
    ) -> Result<AcceptOutcome> {
        MergeService::accept(self, ctx, id, actor).await
    }
}

/// Aufträge eines Branches im Zustand
fn entries_of(state: &DispatchState, branch: &str) -> Vec<MergeEntry> {
    state
        .merges
        .values()
        .filter(|entry| entry.branch == branch)
        .cloned()
        .collect()
}

/// Ticket oder `None`, wenn es gelöscht wurde; andere Fehler (Verbindung) gehen weiter
async fn load(ctx: &DispatchContext, id: &str) -> Result<Option<TicketDetail>> {
    match ctx.services().tickets.get(id).await {
        Ok(detail) => Ok(Some(detail)),
        Err(error) if error.code() == Some("ticket-not-found") => Ok(None),
        Err(error) => Err(error),
    }
}

/// Grund, wenn sich der eigene Merge nicht zurücksetzen ließ: nennt den Ordner, in dem von Hand
/// aufzuräumen ist (im Hauptordner die Meldung des Mergers)
fn abort_failure(error: &nexus_core::Error, place: &MergePlace) -> String {
    let message = error.to_string();
    if place.label == MAIN_LABEL {
        return message;
    }
    let details = error.harness().and_then(|harness| harness.details.clone());
    let field = |name: &str| -> Option<String> {
        details
            .as_ref()
            .and_then(|details| details.get(name))
            .and_then(|value| value.as_str())
            .map(str::to_string)
    };
    match (field("git"), field("command")) {
        (Some(git), Some(command)) => format!(
            "Der {} steht noch im Merge-Zustand: {git}. Bitte dort von Hand aufräumen \
({command}).",
            place.label
        ),
        _ => format!("Im {}: {message}", place.label),
    }
}

/// Grund für lokale Änderungen, die der Branch ebenfalls ändert
fn overlap_text(files: &[String]) -> String {
    format!(
        "lokale Änderungen in {}, die der Branch ebenfalls ändert",
        list(files)
    )
}

/// Grund einer Abweisung durch Git: die lokal geänderten Dateien, die der Branch ebenfalls
/// ändert; ist diese Liste leer, die Dateien aus der Git-Meldung („would be overwritten by
/// merge:“), sonst die Meldung in einer Zeile
fn refusal(message: &str, overlapping: &[String]) -> String {
    if !overlapping.is_empty() {
        return format!(
            "Git lehnt den Merge ab, er würde lokale Änderungen überschreiben: {}",
            list(overlapping)
        );
    }
    let lines: Vec<&str> = message.split('\n').map(|line| line.trim_end()).collect();
    let mut files: Vec<String> = Vec::new();
    let mut index = 0;
    while index < lines.len() {
        if !lines[index].ends_with("would be overwritten by merge:") {
            index += 1;
            continue;
        }
        index += 1;
        while index < lines.len() && indented(lines[index]) {
            files.push(lines[index].trim().to_string());
            index += 1;
        }
    }
    if !files.is_empty() {
        return format!(
            "Git lehnt den Merge ab, er würde lokale Änderungen überschreiben: {}",
            list(&files)
        );
    }
    format!("Git lehnt den Merge ab: {}", one_line(message))
}

/// Zeile einer Dateiliste: beginnt mit Leerraum, danach etwas, das keiner ist (`/^\s+\S/`)
fn indented(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.len() < line.len() && !trimmed.is_empty()
}

/// Höchstens zehn Dateien, danach „und n weitere“; ohne Dateien „unbekannten Dateien“
fn list(files: &[String]) -> String {
    if files.is_empty() {
        return "unbekannten Dateien".to_string();
    }
    if files.len() > 10 {
        return format!(
            "{} und {} weitere",
            files[..10].join(", "),
            files.len() - 10
        );
    }
    files.join(", ")
}

/// Alle Folgen von Leerraum zu einem Leerzeichen, Rand entfernt (wie `replace(/\s+/g, ' ')`)
fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Kürzt auf höchstens `max` Zeichen; das letzte wird „…“
fn shorten(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let kept: String = text.chars().take(max - 1).collect();
    format!("{kept}…")
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

    #[test]
    fn dateiliste_kurzt_nach_zehn_eintragen() {
        let files: Vec<String> = (1..=12).map(|n| format!("f{n}.txt")).collect();
        assert_eq!(list(&[]), "unbekannten Dateien");
        assert_eq!(list(&files[..2]), "f1.txt, f2.txt");
        assert!(list(&files).ends_with("und 2 weitere"));
    }

    #[test]
    fn abweisung_nennt_die_dateien_der_git_meldung() {
        let message = "error: The following untracked working tree files would be overwritten \
by merge:\n\tx.txt\nAborting";
        assert_eq!(
            refusal(message, &[]),
            "Git lehnt den Merge ab, er würde lokale Änderungen überschreiben: x.txt"
        );
    }

    #[test]
    fn abweisung_ohne_dateiliste_steht_in_einer_zeile() {
        assert_eq!(
            refusal("fatal: etwas\n  ging   schief\nHinweis: x", &[]),
            "Git lehnt den Merge ab: fatal: etwas ging schief Hinweis: x"
        );
    }
}
