import { basename } from 'node:path';
import {
  DISPATCH_MARKERS, HarnessError, LIMIT_PAUSE, MAX_BLOCKED_REASON_LENGTH, SETUP_REASON_PREFIX,
  type Board, type BoardTicketDetail, type RunEnd, type RunRecord, type RunResult,
} from '@harness/core';
import { BoardRuntimeHolder } from '../dbboard/BoardRuntimeHolder.js';
import type { DispatchContext } from './DispatchContext.js';
import type { HandleResult } from './RunRecorder.js';
import { ReviewService } from './ReviewService.js';

/** So viele Limit-Pausen in Folge zählen nicht als Versuch; die nächste schon */
const LIMIT_PAUSES_FREE = 2;

// Wertet einen beendeten Lauf aus (Port von finishRun/abortTicket des alten Boards, erweitert um Fragen und Merge).
export class ResultHandler {
  private readonly reviews: ReviewService;

  constructor(
    private readonly ctx: DispatchContext,
    private readonly mergeSessionEnded: (record: RunRecord, end: RunEnd) => Promise<void>,
  ) {
    this.reviews = new ReviewService(ctx);
  }

  // Ausgang (done, review, blocked, question, limit, claude-missing, released, taken, other, gone, merge-session, setup-failed)
  // samt Abbruch- bzw. Blockiert-Grund für die Auswertung
  async handle(record: RunRecord, end: RunEnd, reason?: string): Promise<HandleResult> {
    if (record.kind === 'merge') {
      await this.mergeSessionEnded(record, end);
      return { outcome: 'merge-session' };
    }
    const result = await this.evaluate(record, end, reason);
    return typeof result === 'string' ? { outcome: result } : result;
  }

  // Auswertung eines Umsetzungslaufs
  private async evaluate(record: RunRecord, end: RunEnd, reason?: string): Promise<string | HandleResult> {
    const { ctx } = this;
    let detail: BoardTicketDetail;
    try {
      detail = await ctx.services.tickets.get(record.ticketId);
    } catch (error) {
      // Nur ein gelöschtes Ticket ist weg; Verbindungsfehler gehen an den Tick (der Lauf bleibt für später)
      if (error instanceof HarnessError && error.code === 'ticket-not-found') return 'gone';
      throw error;
    }
    const board = await ctx.board(detail.boardId);
    if (record.kind === 'work' && record.setup?.length) {
      if (end.setupOk) await this.setupDone(record, board);
      if (end.setupFailed) return detail.assignee === record.agent ? this.setupFailed(detail, record, end.setupFailed, board) : 'taken';
    }
    if (detail.status === 'done') {
      // Hat ein Mensch das Ticket übernommen und selbst abgeschlossen, merged der Dispatcher nicht
      if (detail.assignee && detail.assignee !== record.agent) return 'taken';
      // Freigabe vor dem Merge: wartet als Human Interaction auf „Übernehmen“ oder „Zurück an den Agenten“
      if (ctx.cfg.reviewBeforeMerge && detail.branch) return this.reviews.hold(detail, record, end);
      await ctx.stores.state.update((s) => {
        s.merges[detail.id] = {
          ticketId: detail.id, branch: detail.branch ?? record.branch, phase: 'pending', sessionUsed: false, waitNoted: false, target: null,
          since: ctx.now().toISOString(),
        };
      });
      ctx.log(`erledigt ${detail.id} – Merge vorgemerkt`);
      return 'done';
    }
    if (detail.assignee !== record.agent) return 'taken';
    if (detail.status === 'blocked') {
      await ctx.notify({ kind: 'blocked', ticketId: detail.id, title: detail.title, text: detail.blockedReason ?? 'Blockiert', repoPath: board?.repoPath ?? null });
      return { outcome: 'blocked', ...(detail.blockedReason ? { blockedReason: detail.blockedReason } : {}) };
    }
    if (detail.status !== 'in_progress') return 'other';
    if (detail.waitingOnQuestions.length) {
      const ids = detail.waitingOnQuestions.map((q) => q.id).join(', ');
      await ctx.services.tickets.release(detail.id, ctx.actor, { status: 'planned' });
      await ctx.comment(detail.id,
        `${DISPATCH_MARKERS.QUESTION}: wartet auf ${ids} – Ticket zurück nach Geplant, Branch ${detail.branch ?? record.branch} und Worktree bleiben. ` +
        'Nach der Entscheidung geht es dort weiter.');
      await ctx.notify({ kind: 'question', ticketId: detail.id, title: detail.title, text: `wartet auf ${ids}`, repoPath: board?.repoPath ?? null });
      return 'question';
    }
    const minutes = ctx.cfg.pauseMinutesOnLimit;
    if (!reason && ctx.output.claudeMissing(end)) {
      // Kein Fehler des Tickets: nicht als Versuch zählen, sondern pausieren, bis Claude Code wieder da ist
      await this.pause(detail, board?.repoPath ?? null,
        `${DISPATCH_MARKERS.PAUSE}: Claude Code wurde nicht gefunden (Befehl „${record.command}“), Ticket zurück nach Geplant. ` +
        `Der Dispatcher startet ${minutes} min lang nichts Neues. Bitte Claude Code installieren oder den Befehl in der Einstellung prüfen. Zählt nicht als Versuch.`,
        `Claude Code nicht gefunden – Pause ${minutes} min`);
      return 'claude-missing';
    }
    const summary = ctx.output.summarize(end.output);
    if (!reason && ctx.output.hitUsageLimit(end)) {
      // Ab der dritten Limit-Pause in Folge liegt es vermutlich am Ticket selbst: dann zählt es als Versuch
      if (ctx.attempts.limitPausesInRow(detail.comments) < LIMIT_PAUSES_FREE) {
        await this.pause(detail, board?.repoPath ?? null,
          `${LIMIT_PAUSE} erreicht, Ticket zurück nach Geplant. Der Dispatcher startet ${minutes} min lang nichts Neues. Zählt nicht als Versuch.`,
          `Nutzungslimit – Pause ${minutes} min`);
        return 'limit';
      }
      return this.abort(detail, `Nutzungslimit zum ${LIMIT_PAUSES_FREE + 1}. Mal in Folge`, ctx.output.shown(end.output, summary), record.logFile);
    }
    return this.abort(detail, reason ?? ctx.output.abortReason(end, summary, record.run), ctx.output.shown(end.output, summary), record.logFile);
  }

  // Einrichtung gelungen: im Verzeichnis der Worktrees merken, damit ein Fortsetzen nicht erneut installiert
  private async setupDone(record: RunRecord, board: Board | null): Promise<void> {
    if (!board?.repoPath) return;
    try {
      await (await this.ctx.repo(board)).manager.markSetup(record.branch, 'done');
    } catch (error) {
      if (BoardRuntimeHolder.isConnectionFailure(error)) throw error;
      this.ctx.log(`FEHLER ${record.ticketId}: Einrichtung nicht vermerkt: ${(error as Error).message}`);
    }
  }

  // Einrichtung gescheitert (Claude lief nicht): Blockiert mit Grund „Einrichtung fehlgeschlagen: …“, Ausgabe als Kommentar,
  // Meldung. Zählt nicht als Versuch – es liegt an der Umgebung, ein größeres Profil hilft nicht; beim nächsten Start wird
  // die Einrichtung erneut versucht
  private async setupFailed(detail: BoardTicketDetail, record: RunRecord, failed: NonNullable<RunResult['setupFailed']>, board: Board | null): Promise<'setup-failed'> {
    const { ctx } = this;
    const step = `${failed.cwd}$ ${failed.run} (${failed.timedOut ? 'Zeitlimit' : `Exit ${failed.code}`})`;
    const full = `${SETUP_REASON_PREFIX} ${step}`.replace(/\s+/g, ' ').trim();
    const reason = full.length > MAX_BLOCKED_REASON_LENGTH ? `${full.slice(0, MAX_BLOCKED_REASON_LENGTH - 1)}…` : full;
    const lines = [`${DISPATCH_MARKERS.SETUP} fehlgeschlagen: ${step}. Claude wurde nicht gestartet; das zählt nicht als Versuch.`,
      'Nach dem Beheben das Ticket wieder auf Geplant setzen – beim nächsten Start läuft die Einrichtung erneut.'];
    const shown = ctx.output.tail(failed.output);
    if (shown) lines.push('', 'Letzte Ausgabe:', '```', shown, '```');
    lines.push('', `Protokoll: ${basename(record.logFile)}`);
    await ctx.comment(detail.id, lines.join('\n'));
    await ctx.services.tickets.update(detail.id, { status: 'blocked', blockedReason: reason, assignee: null }, ctx.actor, { force: true });
    ctx.log(`Einrichtung fehlgeschlagen ${detail.id}: ${step}`);
    await ctx.notify({ kind: 'blocked', ticketId: detail.id, title: detail.title, text: reason, repoPath: board?.repoPath ?? null });
    return 'setup-failed';
  }

  // Pause ohne Versuch: Kommentar, zurück nach Geplant, neue Starts für pauseMinutesOnLimit aussetzen, Meldung
  private async pause(detail: BoardTicketDetail, repoPath: string | null, comment: string, notice: string): Promise<void> {
    const { ctx } = this;
    await ctx.comment(detail.id, comment);
    await ctx.services.tickets.release(detail.id, ctx.actor, { status: 'planned' });
    const until = new Date(ctx.now().getTime() + ctx.cfg.pauseMinutesOnLimit * 60_000).toISOString();
    await ctx.stores.state.update((s) => { s.pausedUntil = until; });
    ctx.log(`Pause bis ${until}: ${notice} (${detail.id})`);
    await ctx.notify({ kind: 'paused', ticketId: detail.id, title: detail.title, text: notice, repoPath });
  }

  // Abbruch: Kommentar mit Grund und Ausgabe; zurück nach Geplant, beim letzten Versuch Blockiert. Liefert den Ausgang
  // (released, blocked) mit Grund und ob es der letzte Versuch war
  async abort(detail: BoardTicketDetail, reason: string, output: string, logFile: string | null): Promise<HandleResult & { outcome: 'released' | 'blocked' }> {
    const { ctx } = this;
    const attempt = ctx.attempts.failed(detail.comments) + 1;
    const last = attempt >= ctx.cfg.maxAttempts;
    const lines = [
      `${DISPATCH_MARKERS.ABORT} Versuch ${attempt}/${ctx.cfg.maxAttempts}: ${reason}.`,
      last ? 'Das war der letzte automatische Versuch – das Ticket geht nach Blockiert.' : 'Das Ticket geht zurück nach Geplant.',
    ];
    const shown = ctx.output.tail(output);
    if (shown) lines.push('', 'Letzte Ausgabe:', '```', shown, '```');
    if (logFile) lines.push('', `Protokoll: ${basename(logFile)}`);
    await ctx.comment(detail.id, lines.join('\n'));
    ctx.log(`Abbruch ${detail.id} (${attempt}/${ctx.cfg.maxAttempts}): ${reason}`);
    if (last) {
      const blockedReason = `Automatische Umsetzung ${attempt}× abgebrochen – bitte ansehen`;
      await ctx.services.tickets.update(detail.id, { status: 'blocked', blockedReason, assignee: null }, ctx.actor, { force: true });
      const board = await ctx.board(detail.boardId);
      await ctx.notify({ kind: 'blocked', ticketId: detail.id, title: detail.title, text: blockedReason, repoPath: board?.repoPath ?? null });
      return { outcome: 'blocked', abort: { reason, final: true } };
    }
    await ctx.services.tickets.release(detail.id, ctx.actor, { status: 'planned' });
    return { outcome: 'released', abort: { reason, final: false } };
  }
}
