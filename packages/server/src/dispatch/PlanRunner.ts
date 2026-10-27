import { basename } from 'node:path';
import {
  DISPATCH_MARKERS, HarnessError, PRIORITY_RANK, PlanningCandidate, ResearchWorktree,
  type Board, type BoardTicketDetail, type BoardTicketView, type RunEnd, type RunRecord, type BoardTicketStatus,
} from '@harness/core';
import { BoardRuntimeHolder } from '../dbboard/BoardRuntimeHolder.js';
import type { DispatchContext } from './DispatchContext.js';

/** Ergebnis der Auswertung eines Planungslaufs */
export type PlanOutcome = 'planned' | 'question' | 'empty' | 'limit' | 'gone';

/** Bericht des automatischen Anstoßes eines Ticks */
export interface PlanAutoReport {
  started: string[];
  errors: string[];
}

/** Fehler, die am Ticket selbst liegen: die Automatik merkt das Ticket als erledigt und versucht es nicht wieder */
const TICKET_ERRORS = new Set(['plan-no-repo', 'plan-not-possible', 'plan-waiting-question', 'plan-claimed', 'plan-running', 'ticket-not-found']);
/** Nach einem Umgebungs- oder Repo-Fehler setzt die Automatik so lange aus */
export const PLANNING_RETRY_MINUTES = 30;

/** Status, in denen ein Ticket nicht (mehr) geplant wird */
const NOT_PLANNABLE: ReadonlySet<BoardTicketStatus> = new Set<BoardTicketStatus>(['in_progress', 'done', 'not_needed']);
/** Spielraum zwischen der Uhr des Dispatchers (Start des Laufs) und der Uhr der Datenbank */
const CLOCK_SKEW_MS = 60_000;

// Planungsläufe an Tickets („Planen“ im Ticket): eigenes Board mit Repo prüfen, Planungs-Claim setzen, losgelösten Worktree
// anlegen, `claude -p` nur lesend starten, danach auswerten (Plan gespeichert? Frage angelegt? sonst Kommentar am Ticket) und
// den Worktree immer wieder entfernen.
export class PlanRunner {
  private readonly worktrees = new ResearchWorktree('plan');

  constructor(private readonly ctx: DispatchContext) {}

  // Name des Planungs-Agents eines Tickets
  static agentName(ticketId: string): string {
    return `planner-${ticketId}`;
  }

  // Startet die Planung eines Tickets (von Hand oder automatisch); Fehler als HarnessError
  async start(ticketId: string, mode: 'manual' | 'auto' = 'manual'): Promise<RunRecord> {
    const { ctx } = this;
    const ticket = await ctx.services.tickets.get(ticketId);
    const board = await this.check(ticket);
    const agent = PlanRunner.agentName(ticket.id);
    const work = await ctx.repo(board);
    const run = ctx.chooser.planning();
    await ctx.services.planning.claim(ticket.id, agent, ctx.actor);
    let worktree: { path: string; warning?: string };
    try {
      worktree = await this.worktrees.create(work.root, ticket.id);
    } catch (error) {
      await this.release(ticket.id, agent);
      throw error;
    }
    if (worktree.warning) ctx.log(`Hinweis ${ticket.id}: ${worktree.warning}`);
    let record: RunRecord;
    try {
      record = await ctx.launch({
        kind: 'plan', ticketId: ticket.id, boardId: board.id, title: ticket.title, agent,
        cwd: worktree.path, branch: '', target: null, run, prompt: ctx.prompts.plan({ ticketId: ticket.id, title: ticket.title, agent, cfg: ctx.cfg }),
        plannedBefore: ticket.plannedAt ?? null, repoRoot: work.root,
      });
    } catch (error) {
      const left = this.worktrees.remove(work.root, worktree.path);
      if (left) ctx.log(`FEHLER ${ticket.id}: ${left}`);
      await this.release(ticket.id, agent);
      throw error;
    }
    const at = ctx.now().toISOString();
    await ctx.stores.state.update((s) => {
      const before = s.planning[ticket.id];
      s.planning[ticket.id] = {
        ticketId: ticket.id, lastStartedAt: at, count: (before?.count ?? 0) + 1, autoAt: mode === 'auto' ? at : before?.autoAt ?? null,
      };
    });
    ctx.log(`Planung gestartet ${ticket.id} (${mode === 'auto' ? 'automatisch' : 'von Hand'}) → ${agent} in ${worktree.path} [${run.profile}: ${run.model ?? '-'}/${run.effort ?? '-'}]`);
    return record;
  }

  // Stößt Tickets mit Planungsbedarf auf eigenen Boards mit Repo-Pfad an (dazugekommen seit dem Einschalten der Automatik,
  // Priorität und Nummer zuerst, je Ticket höchstens einmal), solange im WIP der Tickets Platz ist, bis planning.wip
  // gleichzeitige Läufe. Fehler des Tickets (kein Repo, offene Frage, fremder Claim)
  // merken es als erledigt; Umgebungs- und Repo-Fehler brechen ab, ohne zu merken, und setzen die Automatik
  // PLANNING_RETRY_MINUTES aus (je Fehlertext ein Bericht)
  async startAuto(freeWipSlots: number): Promise<PlanAutoReport> {
    const { ctx } = this;
    const report: PlanAutoReport = { started: [], errors: [] };
    const now = ctx.now();
    const state = await ctx.stores.state.read();
    const auto = state.planningAuto;
    if (!auto.since || (auto.retryAt && new Date(auto.retryAt) > now) || freeWipSlots <= 0) return report;
    let free = ctx.cfg.planning.wip - (await ctx.stores.runs.list()).filter((r) => r.kind === 'plan').length;
    if (free <= 0) return report;
    for (const ticket of await this.candidates(auto.since, now)) {
      if (free <= 0) break;
      // Schon einmal angestoßen (automatisch oder von Hand): kein automatischer Neuversuch
      if (state.planning[ticket.id]) continue;
      try {
        await this.start(ticket.id, 'auto');
        report.started.push(ticket.id);
        free -= 1;
        await ctx.stores.state.update((s) => {
          s.planningAuto = { ...s.planningAuto, retryAt: null, lastError: null };
        });
      } catch (error) {
        if (BoardRuntimeHolder.isConnectionFailure(error)) throw error;
        const message = (error as Error).message;
        if (error instanceof HarnessError && TICKET_ERRORS.has(error.code)) {
          await this.markTried(ticket.id);
          report.errors.push(`${ticket.id}: ${message}`);
          continue;
        }
        const retryAt = new Date(now.getTime() + PLANNING_RETRY_MINUTES * 60_000);
        await ctx.stores.state.update((s) => {
          s.planningAuto = { ...s.planningAuto, retryAt: retryAt.toISOString(), lastError: message };
        });
        if (auto.lastError !== message) {
          report.errors.push(`Planungs-Automatik ausgesetzt bis ${this.hhmm(retryAt)} (bei ${ticket.id}): ${message}`);
        }
        break;
      }
    }
    return report;
  }

  // Wertet einen beendeten Planungslauf aus; der Worktree wird in jedem Fall entfernt
  async evaluate(record: RunRecord, end: RunEnd, reason?: string): Promise<PlanOutcome> {
    try {
      return await this.outcome(record, end, reason);
    } finally {
      if (record.repoRoot) {
        const left = this.worktrees.remove(record.repoRoot, record.cwd);
        if (left) this.ctx.log(`FEHLER ${record.ticketId}: ${left}`);
      }
    }
  }

  // Verwaiste Planungs-Worktrees eines Repos entfernen (`active`: Ordner laufender Planungen); Meldungen je Worktree
  removeOrphans(repoPath: string, active: string[]): string[] {
    return this.worktrees.removeOrphans(repoPath, active);
  }

  // Kandidaten der Automatik: Tickets mit Planungsbedarf auf eigenen Boards mit Repo-Pfad, die seit `since` angelegt oder
  // geändert wurden, ohne offene Frage und ohne fremden Claim (PlanningCandidate); Priorität, dann Nummer
  private async candidates(since: string, now: Date): Promise<BoardTicketView[]> {
    const { ctx } = this;
    const boards = (await ctx.services.boards.owned(ctx.me.userId)).filter((b) => b.repoPath);
    const from = Date.parse(since);
    const found: BoardTicketView[] = [];
    for (const board of boards) {
      const views = await ctx.services.tickets.list({ boardIds: [board.id], needsPlanning: true });
      found.push(...views.filter((t) =>
        PlanningCandidate.matches(t, now.getTime(), t.waitingOnQuestions.length) &&
        Math.max(Date.parse(t.createdAt), Date.parse(t.updatedAt)) >= from));
    }
    return found.sort((a, b) => PRIORITY_RANK[a.priority] - PRIORITY_RANK[b.priority] || a.number - b.number);
  }

  // Merkt ein nicht startbares Ticket als angestoßen, damit die Automatik es nicht jede Minute neu versucht
  private async markTried(ticketId: string): Promise<void> {
    const at = this.ctx.now().toISOString();
    await this.ctx.stores.state.update((s) => {
      const before = s.planning[ticketId];
      s.planning[ticketId] = { ticketId, lastStartedAt: before?.lastStartedAt ?? at, count: before?.count ?? 0, autoAt: at };
    });
  }

  // HH:MM Ortszeit
  private hhmm(d: Date): string {
    return `${String(d.getHours()).padStart(2, '0')}:${String(d.getMinutes()).padStart(2, '0')}`;
  }

  // Darf das Ticket jetzt geplant werden? Liefert das eigene Board mit Repo-Pfad, sonst HarnessError
  private async check(ticket: BoardTicketDetail): Promise<Board> {
    const { ctx } = this;
    if (NOT_PLANNABLE.has(ticket.status)) {
      throw new HarnessError('plan-not-possible', `${ticket.id} steht auf „${ticket.status}“ – geplant werden nur Tickets in Backlog, In Planung, Geplant oder Blockiert.`);
    }
    if (ticket.waitingOnQuestions.length) {
      throw new HarnessError('plan-waiting-question', `${ticket.id} wartet auf Frage ${ticket.waitingOnQuestions.map((q) => q.id).join(', ')} – erst entscheiden, dann planen.`);
    }
    const runs = (await ctx.stores.runs.list()).filter((r) => r.ticketId === ticket.id);
    if (runs.some((r) => r.kind === 'plan')) throw new HarnessError('plan-running', `Für ${ticket.id} läuft schon eine Planung.`);
    if (runs.length) throw new HarnessError('plan-not-possible', `Für ${ticket.id} läuft gerade ein anderer Lauf (${runs[0].kind}).`);
    const board = await ctx.board(ticket.boardId);
    if (!board || board.ownerId !== ctx.me.userId || !board.repoPath) {
      throw new HarnessError('plan-no-repo',
        `${ticket.id} liegt nicht auf einem eigenen Board mit Repo-Pfad – geplant wird nur auf eigenen Boards, deren Repo-Pfad gesetzt ist.`);
    }
    return board;
  }

  // Planungs-Claim des Agents freigeben; Fehler nur ins Protokoll
  private async release(ticketId: string, agent: string): Promise<void> {
    try {
      await this.ctx.services.planning.release(ticketId, agent, this.ctx.actor);
    } catch (error) {
      this.ctx.log(`FEHLER ${ticketId}: Planungs-Claim nicht freigegeben: ${(error as Error).message}`);
    }
  }

  // Hat der Lauf einen Plan gespeichert? plannedAt neuer als beim Start und vom Agent des Laufs
  private planned(ticket: BoardTicketDetail, record: RunRecord): boolean {
    if (!ticket.plannedAt || ticket.plannedBy !== record.agent) return false;
    return !record.plannedBefore || Date.parse(ticket.plannedAt) > Date.parse(record.plannedBefore);
  }

  // Steht der Kommentar „Planung ohne Ergebnis“ dieses Laufs schon am Ticket?
  private commented(ticket: BoardTicketDetail, record: RunRecord): boolean {
    const started = Date.parse(record.startedAt) - CLOCK_SKEW_MS;
    return ticket.comments.some((c) => c.body.startsWith(DISPATCH_MARKERS.PLAN_EMPTY) && c.body.includes(basename(record.logFile)) && Date.parse(c.createdAt) >= started);
  }

  // Auswertung ohne Aufräumen: Plan da → planned; neue offene Frage → question; Nutzungslimit/Claude fehlt → Pause;
  // sonst Kommentar am Ticket. Außer bei planned wird der Planungs-Claim freigegeben
  private async outcome(record: RunRecord, end: RunEnd, reason?: string): Promise<PlanOutcome> {
    const { ctx } = this;
    const id = record.ticketId;
    let ticket: BoardTicketDetail;
    try {
      ticket = await ctx.services.tickets.get(id);
    } catch (error) {
      if (error instanceof HarnessError && error.code.endsWith('-not-found')) return 'gone';
      throw error;
    }
    if (this.planned(ticket, record)) {
      ctx.log(`Planung ${id} gespeichert`);
      return 'planned';
    }
    await this.release(id, record.agent);
    if (ticket.waitingOnQuestions.length) {
      ctx.log(`Planung ${id} wartet auf Frage ${ticket.waitingOnQuestions.map((q) => q.id).join(', ')}`);
      return 'question';
    }
    const done = this.commented(ticket, record);
    const minutes = ctx.cfg.pauseMinutesOnLimit;
    if (!reason && (ctx.output.claudeMissing(end) || ctx.output.hitUsageLimit(end))) {
      if (done) return 'limit';
      const missing = ctx.output.claudeMissing(end);
      const until = new Date(ctx.now().getTime() + minutes * 60_000).toISOString();
      await ctx.stores.state.update((s) => { s.pausedUntil = until; });
      const why = missing ? `Claude Code wurde nicht gefunden (Befehl „${record.command}“)` : 'Nutzungslimit erreicht';
      await ctx.comment(id, `${DISPATCH_MARKERS.PLAN_EMPTY}: ${why}. Der Dispatcher startet ${minutes} min lang nichts Neues; „Planen“ danach erneut klicken.\n\nProtokoll: ${basename(record.logFile)}`);
      ctx.log(`Pause bis ${until}: ${why} (Planung ${id})`);
      await ctx.notify({ kind: 'paused', ticketId: id, title: ticket.title, text: `${why} – Pause ${minutes} min`, repoPath: null });
      return 'limit';
    }
    if (done) return 'empty';
    const summary = ctx.output.summarize(end.output);
    const why = reason ?? ctx.output.abortReason(end, summary, record.run, 'Lauf endete, ohne einen Plan oder eine Frage zu speichern');
    const lines = [`${DISPATCH_MARKERS.PLAN_EMPTY}: ${why}.`, 'Kein automatischer Neuversuch – „Planen“ im Ticket stößt eine neue Planung an.'];
    const shown = ctx.output.tail(ctx.output.shown(end.output, summary));
    if (shown) lines.push('', 'Letzte Ausgabe:', '```', shown, '```');
    lines.push('', `Protokoll: ${basename(record.logFile)}`);
    await ctx.comment(id, lines.join('\n'));
    ctx.log(`Planung ${id} ohne Ergebnis: ${why}`);
    return 'empty';
  }
}
