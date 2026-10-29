//! Rechnet die Entscheidung eines Laufs nach, den der Node-Dispatcher gestartet hat, und
//! vergleicht sie feldweise.
//!
//! Benutzt werden nur die lesenden Teile des Dispatchers: Profilwahl, Versuchszähler,
//! Arbeitsanweisung und die Argumente von `claude -p`. Weder [`TicketStarter`] noch
//! [`MergeService::start_session`] werden aufgerufen, es entsteht also kein Claim, kein
//! Kommentar, kein Worktree und keine Lauf-Datei.
//!
//! Was Rust nach dem Start nicht mehr feststellen kann, bleibt außen vor statt als Abweichung
//! aufzufallen: der Ordner losgelöster Worktrees (Recherche und Planung legen ihn beim Start
//! an und räumen ihn danach weg) und die Frage, ob ein Worktree beim Start neu war
//! (`setup`) – die Einrichtungsbefehle werden deshalb nur geprüft, wenn Node welche
//! ausgeführt hat.

use nexus_core::dispatch::shadow::{compare, text_diff, ShadowDecision, ShadowDiff};
use nexus_core::dispatch::{
    ClaudeInvocation, ProfileTicket, RunKind, RunRecord, RunSettings, WorktreeSetupStep,
};
use nexus_core::Result;
use serde_json::{json, Value};

use crate::dispatch::context::DispatchContext;
use crate::dispatch::merge_service::MergeService;
use crate::dispatch::prompt_values::{
    plan_agent_name, research_agent_name, MergeValues, PromptValues, ResearchValues,
};

/// Felder, die bei jeder Laufart verglichen werden
const COMMON: &[&str] = &[
    "kind",
    "agent",
    "command",
    "run.profile",
    "run.model",
    "run.effort",
    "run.maxTurns",
    "run.timeoutMinutes",
    "run.why",
    "run.reason",
    "run.ruleIndex",
    "run.configHash",
    "run.escalatedFrom",
];

/// Was Rust nachgerechnet hat: die Felder eines Laufs und die Namen, die verglichen werden
struct Recomputed {
    value: Value,
    fields: Vec<&'static str>,
    prompt: Option<String>,
}

/// Rechnet Entscheidungen nach und vergleicht sie.
#[derive(Debug)]
pub struct ShadowDecisionMaker<'a> {
    ctx: &'a DispatchContext,
}

impl<'a> ShadowDecisionMaker<'a> {
    /// Vergleicher auf diesem (nur lesenden) Kontext
    pub fn new(ctx: &'a DispatchContext) -> Self {
        Self { ctx }
    }

    /// Vergleicht einen von Node gestarteten Lauf mit dem, was Rust entschieden hätte.
    /// Lässt sich nichts nachrechnen (Ticket weg, Frage weg, Board ohne Repo), bleibt die
    /// Zeile ohne Urteil und nennt den Grund
    pub async fn compare(&self, record: &RunRecord) -> ShadowDecision {
        let mut line = ShadowDecision {
            at: self.ctx.now_iso(),
            run_id: record.id.clone(),
            ticket_id: record.ticket_id.clone(),
            kind: record.kind.as_str().to_string(),
            equal: true,
            diffs: Vec::new(),
            prompt_diff: None,
            skipped: None,
        };
        let node = match serde_json::to_value(record) {
            Ok(node) => node,
            Err(error) => {
                line.skipped = Some(format!("Lauf-Datei nicht lesbar: {error}"));
                line.equal = false;
                return line;
            }
        };
        let mine = match self.recompute(record).await {
            Ok(mine) => mine,
            Err(error) => {
                line.skipped = Some(error.to_string());
                line.equal = false;
                return line;
            }
        };
        line.diffs = compare::diffs(&node, &mine.value, &mine.fields);
        if let Some(prompt) = &mine.prompt {
            if &record.prompt != prompt {
                line.diffs.push(ShadowDiff::new(
                    "prompt",
                    Some(format!("{} Zeichen", record.prompt.chars().count())),
                    Some(format!("{} Zeichen", prompt.chars().count())),
                ));
                line.prompt_diff = Some(text_diff::unified(&record.prompt, prompt));
            }
        }
        line.equal = line.diffs.is_empty();
        line
    }

    /// Die Entscheidung, wie Rust sie getroffen hätte
    async fn recompute(&self, record: &RunRecord) -> Result<Recomputed> {
        match record.kind {
            RunKind::Work => self.work(record).await,
            RunKind::Merge => self.merge(record).await,
            RunKind::Research => self.research(record).await,
            RunKind::Plan => self.plan(record).await,
        }
    }

    /// Umsetzung: Profil aus Ticket und Versuch, Arbeitsanweisung aus dem Ticket samt
    /// Antworten und Review-Rückmeldungen, Ordner aus dem Worktree-Verzeichnis
    async fn work(&self, record: &RunRecord) -> Result<Recomputed> {
        let ctx = self.ctx;
        let detail = ctx.services().tickets.get(&record.ticket_id).await?;
        let attempt = ctx.attempts().failed(&detail.comments) as i64 + 1;
        let run = ctx
            .chooser()
            .choose(&ProfileTicket::from(&detail.ticket), attempt);
        let agent = ctx.agent_name(&record.ticket_id);
        let branch = detail.ticket.branch.clone();
        let cwd = self.worktree_path(record, branch.as_deref()).await;
        let values = PromptValues::new(ctx).work(&detail).await?;
        let prompt = ctx.prompts.work(&values.input(&ctx.cfg));
        let mut fields: Vec<&'static str> = COMMON.to_vec();
        fields.extend(["branch", "target", "attempt", "args", "setupTimeoutMinutes"]);
        // Der Ordner nur, wenn das Verzeichnis ihn kennt; ein aufgeräumter Worktree wäre
        // sonst eine Abweichung, die keine ist
        if cwd.is_some() {
            fields.push("cwd");
        }
        let setup: Option<Vec<WorktreeSetupStep>> = record
            .setup
            .as_ref()
            .map(|_| ctx.cfg.worktree_setup.clone());
        if setup.is_some() {
            fields.push("setup");
        }
        let setup_timeout = record
            .setup
            .as_ref()
            .is_some_and(|steps| !steps.is_empty())
            .then_some(ctx.cfg.worktree_setup_timeout_minutes);
        Ok(Recomputed {
            value: json!({
                "kind": RunKind::Work,
                "agent": agent,
                "branch": branch,
                "cwd": cwd,
                "target": Value::Null,
                "attempt": attempt,
                "command": ctx.cfg.claude.command,
                "args": self.args(&run, &[], RunKind::Work),
                "run": run,
                "setup": setup,
                "setupTimeoutMinutes": setup_timeout,
            }),
            fields,
            prompt: Some(prompt),
        })
    }

    /// Merge-Session: Profil wie bei der Umsetzung, aber mindestens das Standardprofil;
    /// Branch und Zielbranch stehen schon fest und gehen als Werte in die Anweisung ein
    async fn merge(&self, record: &RunRecord) -> Result<Recomputed> {
        let ctx = self.ctx;
        let detail = ctx.services().tickets.get(&record.ticket_id).await?;
        let attempt = ctx.attempts().failed(&detail.comments) as i64 + 1;
        let chosen = ctx
            .chooser()
            .choose(&ProfileTicket::from(&detail.ticket), attempt);
        let run = MergeService::new().at_least_default(ctx, chosen);
        let agent = ctx.agent_name(&record.ticket_id);
        let values = MergeValues {
            ticket_id: record.ticket_id.clone(),
            title: detail.ticket.title.clone(),
            agent: agent.clone(),
            branch: record.branch.clone(),
            target: record.target.clone().unwrap_or_default(),
        };
        let prompt = ctx.prompts.merge(&values.input(&ctx.cfg));
        let extra: Vec<String> = nexus_core::dispatch::config::MERGE_SESSION_TOOLS
            .iter()
            .map(|tool| tool.to_string())
            .collect();
        let mut fields: Vec<&'static str> = COMMON.to_vec();
        fields.push("args");
        Ok(Recomputed {
            value: json!({
                "kind": RunKind::Merge,
                "agent": agent,
                "command": ctx.cfg.claude.command,
                "args": self.args(&run, &extra, RunKind::Merge),
                "run": run,
            }),
            fields,
            prompt: Some(prompt),
        })
    }

    /// Recherche: festes Recherche-Profil und die Anweisung aus der Frage
    async fn research(&self, record: &RunRecord) -> Result<Recomputed> {
        let ctx = self.ctx;
        let id = record
            .question_id
            .clone()
            .unwrap_or_else(|| record.ticket_id.clone());
        let question = ctx.services().questions.get(&id).await?;
        let run = ctx.chooser().research();
        let agent = research_agent_name(&id);
        let values = ResearchValues {
            question_id: id,
            title: question.view.question.title.clone(),
            agent: agent.clone(),
        };
        let prompt = ctx.prompts.research(&values.input(&ctx.cfg));
        let mut fields: Vec<&'static str> = COMMON.to_vec();
        fields.push("args");
        Ok(Recomputed {
            value: json!({
                "kind": RunKind::Research,
                "agent": agent,
                "command": ctx.cfg.claude.command,
                "args": self.args(&run, &[], RunKind::Research),
                "run": run,
            }),
            fields,
            prompt: Some(prompt),
        })
    }

    /// Planung: festes Planungsprofil und die Anweisung aus dem Ticket
    async fn plan(&self, record: &RunRecord) -> Result<Recomputed> {
        let ctx = self.ctx;
        let detail = ctx.services().tickets.get(&record.ticket_id).await?;
        let run = ctx.chooser().planning();
        let agent = plan_agent_name(&record.ticket_id);
        let values = PromptValues::new(ctx).plan(&detail);
        let prompt = ctx.prompts.plan(&values.input(&ctx.cfg));
        let mut fields: Vec<&'static str> = COMMON.to_vec();
        fields.push("args");
        Ok(Recomputed {
            value: json!({
                "kind": RunKind::Plan,
                "agent": agent,
                "command": ctx.cfg.claude.command,
                "args": self.args(&run, &[], RunKind::Plan),
                "run": run,
            }),
            fields,
            prompt: Some(prompt),
        })
    }

    /// Argumente von `claude -p` für diesen Lauf
    fn args(&self, run: &RunSettings, extra: &[String], kind: RunKind) -> Vec<String> {
        ClaudeInvocation::new().args(&self.ctx.cfg, run, extra, kind)
    }

    /// Ordner des Worktrees, den das Verzeichnis für diesen Branch kennt; ohne Branch, ohne
    /// Repo oder ohne Eintrag `None`
    async fn worktree_path(&self, record: &RunRecord, branch: Option<&str>) -> Option<String> {
        let branch = branch?;
        let board = self.ctx.board(record.board_id as i32).await.ok()??;
        let work = self.ctx.repo(&board).await.ok()?;
        work.manager.path_of(branch).ok().flatten()
    }
}
