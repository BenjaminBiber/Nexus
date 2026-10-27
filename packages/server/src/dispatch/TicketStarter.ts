import { existsSync } from 'node:fs';
import { DISPATCH_MARKERS, GitRunner, type BoardTicketDetail, type BoardTicketView, type RunRecord, type RunSettings } from '@harness/core';
import { BoardRuntimeHolder } from '../dbboard/BoardRuntimeHolder.js';
import type { DispatchContext, RepoWork } from './DispatchContext.js';
import { PromptValues } from './PromptValues.js';
import type { ResultHandler } from './ResultHandler.js';
import { RunRecorder } from './RunRecorder.js';

export type StartOutcome = { record: RunRecord } | { skipped: string } | { error: string };

/** Was ein gescheiterter Start für die Auswertung braucht */
interface StartInfo {
  agent: string;
  run: RunSettings;
  attempt: number;
  projectId: number;
  boardId: number;
}

/** Gestarteter Lauf; `targetCreated` meldet einen beim Start neu angelegten Zielbranch */
interface Launched {
  record: RunRecord;
  branch: string;
  cwd: string;
  warning?: string;
  targetCreated?: string;
}

// Übernimmt ein bereites Ticket für den Junior-Agent, richtet den Worktree ein und startet den Worker.
export class TicketStarter {
  private readonly git = new GitRunner();
  private readonly values: PromptValues;

  constructor(
    private readonly ctx: DispatchContext,
    private readonly results: ResultHandler,
  ) {
    this.values = new PromptValues(ctx);
  }

  // Frisch lesen und erneut prüfen (auch: Worktree frei) → claim (nur unveränderte Version) → Worktree (neu oder vorhanden) → Lauf.
  // Scheitert der Start nach dem claim, zählt das als Abbruch-Versuch; nach dem Start nie mehr zurück.
  async start(view: BoardTicketView, work: RepoWork): Promise<StartOutcome> {
    const { ctx } = this;
    const detail = await ctx.services.tickets.get(view.id);
    const notReady = ctx.readiness.reason(detail, work.board, ctx.me.userId, existsSync);
    if (notReady) return { skipped: notReady };
    // Gehört der eingetragene Worktree nicht mehr zum Ticket, startet es neu (eigener Branch) statt ihn wiederherzustellen
    const stale = await work.manager.staleReason(detail);
    if (detail.branch && !stale && !this.git.tryRun(work.root, ['rev-parse', '--verify', '--quiet', `refs/heads/${detail.branch}`]).ok) {
      return { skipped: `Branch ${detail.branch} gibt es auf diesem Rechner nicht` };
    }
    // Läuft im (künftigen) Worktree schon ein Worker, wartet das Ticket ungeclaimt auf den nächsten Tick
    const busy = await ctx.worktreeBusy(detail, work);
    if (busy) return { skipped: busy };
    const attempt = ctx.attempts.failed(detail.comments) + 1;
    const run = ctx.chooser.choose(detail, attempt);
    const agent = ctx.agentName(view.id);
    try {
      await ctx.services.tickets.claim(view.id, agent, ctx.actor, { status: 'planned', rev: detail.rev });
    } catch (error) {
      // Nur fachliche Fehler (Version geändert, schon vergeben) überspringen das Ticket; ein Verbindungsfehler bricht den Tick ab
      if (BoardRuntimeHolder.isConnectionFailure(error)) throw error;
      return { skipped: (error as Error).message };
    }
    let launched: Launched;
    try {
      launched = await this.launch(view, work, detail, agent, run, attempt);
    } catch (error) {
      return this.failed(detail, (error as Error).message, { agent, run, attempt, projectId: work.board.projectId, boardId: work.board.id });
    }
    const { record, branch, cwd, warning, targetCreated } = launched;
    try {
      await ctx.comment(view.id,
        `${DISPATCH_MARKERS.START}: an ${agent} übergeben – Branch ${branch}, Profil ${run.profile} (Modell ${run.model ?? 'Standard'}, ` +
        `Effort ${run.effort ?? 'Standard'}, max. ${run.maxTurns} Schritte, ${run.timeoutMinutes} min). Grund: ${run.why}.` +
        (warning ? `\nHinweis: ${warning}` : ''));
      ctx.log(`gestartet ${view.id} → ${agent} in ${cwd} [${run.profile}: ${run.model ?? '-'}/${run.effort ?? '-'}]`);
    } catch (error) {
      // Der Worker läuft bereits: kein Zurückgeben, sonst startet der nächste Tick ein zweites Mal
      ctx.log(`FEHLER ${view.id}: Start-Kommentar nicht geschrieben: ${(error as Error).message}`);
    }
    // Neu angelegter Zielbranch: eigener Kommentar mit dem Merge-Marker (kein zweites Start-Ereignis), Fehler nur im Log
    if (targetCreated) {
      try {
        await ctx.comment(view.id, `${DISPATCH_MARKERS.MERGE}: ${targetCreated}`);
      } catch (error) {
        ctx.log(`FEHLER ${view.id}: Kommentar zum Zielbranch nicht geschrieben: ${(error as Error).message}`);
      }
    }
    return { record };
  }

  // Worktree (neu oder vorhanden) und Lauf; Warnungen (z. B. nicht wiederhergestellter Worktree) kommen in den Start-Kommentar,
  // ein neu angelegter Zielbranch in einen eigenen
  private async launch(
    view: BoardTicketView, work: RepoWork, detail: BoardTicketDetail, agent: string, run: RunSettings, attempt: number,
  ): Promise<Launched> {
    const { ctx } = this;
    let branch: string;
    let cwd: string;
    const warnings: string[] = [];
    let created = false;
    let targetCreated: string | undefined;
    const resumed = detail.branch ? await work.manager.resume(view.id, ctx.actor) : null;
    if (resumed && 'path' in resumed) {
      // Fortsetzung nach Frage oder Abbruch: vorhandener Branch, Worktree notfalls neu
      cwd = resumed.path;
      await ctx.services.tickets.move(view.id, 'in_progress', ctx.actor);
      branch = detail.branch!;
    } else {
      if (resumed) warnings.push(resumed.reset);
      // Ein Worktree je Ticket: eigener Branch vom Zielbranch des Tickets bzw. vom ausgecheckten Branch des Hauptordners; gebündelt: Beitritt derselben Spec
      const join = ctx.cfg.joinWorktrees === 'sameSpec';
      const started = await work.manager.start(view.id, { join, joinOnlySameSpec: true, repoBoard: work.board }, work.root, ctx.actor);
      if (started.warning) warnings.push(started.warning);
      branch = started.branch;
      cwd = started.path;
      created = !started.joined;
      targetCreated = started.targetCreated;
    }
    // Einrichtung (worktreeSetup) nur für einen neu angelegten Worktree – oder wenn sie dort noch aussteht (gescheitert, abgebrochen)
    const steps = ctx.cfg.worktreeSetup;
    if (steps.length && created) await work.manager.markSetup(branch, 'pending');
    const setup = steps.length && (created || (await work.manager.setupPending(branch))) ? steps : undefined;
    const feedback = this.values.pending(detail.comments);
    const prompt = ctx.prompts.work({ ticketId: view.id, title: view.title, agent, branch, answers: await this.values.answers(view.id), feedback, cfg: ctx.cfg });
    const record = await ctx.launch({ kind: 'work', ticketId: view.id, boardId: work.board.id, title: view.title, agent, cwd, branch, target: null, run, prompt, attempt, ...(setup ? { setup } : {}) });
    return { record, branch, cwd, warning: warnings.join(' ') || undefined, targetCreated };
  }

  // Start gescheitert: zählt als Abbruch-Versuch (zurück nach Geplant, beim letzten Versuch Blockiert) und kommt als Lauf mit
  // Kategorie start-failed in die Auswertung
  private async failed(detail: BoardTicketDetail, message: string, launch: StartInfo): Promise<StartOutcome> {
    const { ctx } = this;
    ctx.log(`FEHLER ${detail.id}: ${message}`);
    try {
      const reason = `Worker konnte nicht gestartet werden: ${message}`;
      const { abort } = await this.results.abort(detail, reason, '', null);
      await new RunRecorder(ctx).keepStartFailed({ ticketId: detail.id, ...launch, reason, final: abort?.final ?? false });
    } catch (error) {
      if (BoardRuntimeHolder.isConnectionFailure(error)) throw error;
      ctx.log(`FEHLER ${detail.id}: Zurückgeben nach gescheitertem Start: ${(error as Error).message}`);
    }
    return { error: message };
  }
}
