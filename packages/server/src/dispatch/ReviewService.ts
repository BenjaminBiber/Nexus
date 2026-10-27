import { basename } from 'node:path';
import {
  DISPATCH_MARKERS, HarnessError, REVIEW_REASON, ReviewDiff, TestRunExtractor, isHumanInteraction,
  type Board, type BoardActor, type BoardTicketDetail, type ReviewFile, type ReviewPatch, type RunEnd, type RunRecord, type TestRun,
} from '@harness/core';
import { BoardRuntimeHolder } from '../dbboard/BoardRuntimeHolder.js';
import type { DispatchContext } from './DispatchContext.js';

/** Höchstlänge der Rückmeldung bei „Zurück an den Agenten“ */
export const MAX_RETURN_TEXT = 10_000;

/** Review eines Tickets für das Panel */
export interface ReviewView {
  ticketId: string;
  branch: string | null;
  /** Ziel des Merges (Zielbranch des Tickets, sonst ausgecheckter Branch des Hauptordners) */
  target: string | null;
  /** Merge-Basis */
  base: string | null;
  files: ReviewFile[];
  tests: TestRun[];
  /** Zeitpunkt der Testläufe (Gate) oder null ohne Review-Eintrag */
  testsAt: string | null;
  /** Lauf-Protokoll (Dateiname) des Laufs */
  runLog: string | null;
  /** false = kein Diff (fremdes Board, kein Repo, kein Branch, Git-Fehler); Grund in `reason` */
  available: boolean;
  reason: string | null;
}

// Freigabe vor dem Merge: Gate nach dem Lauf, Ansicht (Diff, Tests) und „Zurück an den Agenten“.
export class ReviewService {
  private readonly tests = new TestRunExtractor();
  private readonly diff = new ReviewDiff();

  constructor(private readonly ctx: DispatchContext) {}

  // Gate: Tests merken, Ticket auf Blockiert „Human Interaction: Review vor dem Merge“, Kommentar und Meldung
  async hold(detail: BoardTicketDetail, record: RunRecord, end: RunEnd): Promise<'review'> {
    const { ctx } = this;
    const branch = detail.branch ?? record.branch;
    const tests = this.tests.extract(end.output);
    await ctx.stores.reviews.save({ ticketId: detail.id, branch, runId: record.id, logFile: record.logFile, at: ctx.now().toISOString(), tests });
    const board = await ctx.board(detail.boardId);
    const changes = await this.changes(board, branch, detail.targetBranch);
    await ctx.services.tickets.update(detail.id, { status: 'blocked', blockedReason: REVIEW_REASON, assignee: null }, ctx.actor, { force: true });
    await ctx.comment(detail.id,
      `${DISPATCH_MARKERS.REVIEW}: wartet auf Freigabe – ${changes}, Tests: ${this.testSummary(tests)}.\n` +
      'Im Panel „Übernehmen“ oder „Zurück an den Agenten“.');
    ctx.log(`Review ${detail.id} – wartet auf Freigabe vor dem Merge`);
    await ctx.notify({ kind: 'review', ticketId: detail.id, title: detail.title, text: `wartet auf Freigabe – ${changes}`, repoPath: board?.repoPath ?? null });
    return 'review';
  }

  // Ansicht für das Panel: Dateien aus Git (nur eigenes Board mit Repo), Tests aus dem Review-Eintrag
  async view(id: string): Promise<ReviewView> {
    const detail = await this.ctx.services.tickets.get(id);
    const entry = await this.ctx.stores.reviews.read(detail.id);
    const base: ReviewView = {
      ticketId: detail.id, branch: detail.branch, target: null, base: null, files: [],
      tests: entry?.tests ?? [], testsAt: entry?.at ?? null, runLog: entry ? basename(entry.logFile) : null, available: false, reason: null,
    };
    const problem = await this.unavailable(detail);
    if (typeof problem === 'string') return { ...base, reason: problem };
    try {
      const list = this.diff.files(problem.root, detail.branch!, detail.targetBranch);
      return { ...base, target: list.target, base: list.base, files: list.files, available: true };
    } catch (error) {
      if (BoardRuntimeHolder.isConnectionFailure(error)) throw error;
      return { ...base, reason: `Diff nicht berechenbar: ${(error as Error).message}` };
    }
  }

  // Patch einer Datei der Dateiliste
  async fileDiff(id: string, path: string): Promise<ReviewPatch & { path: string }> {
    const detail = await this.ctx.services.tickets.get(id);
    const problem = await this.unavailable(detail);
    if (typeof problem === 'string') throw new HarnessError('review-not-available', problem);
    return { path, ...this.diff.file(problem.root, detail.branch!, path, detail.targetBranch) };
  }

  // „Zurück an den Agenten“: Kommentar als Arbeitsanweisung, Ticket nach Geplant ohne Grund und Zuständigen; Branch und
  // Worktree bleiben, der nächste Start setzt dort fort. Zählt nicht als Versuch
  async returnToAgent(id: string, text: string, actor: BoardActor): Promise<{ ticketId: string; text: string }> {
    const { ctx } = this;
    const detail = await ctx.services.tickets.get(id);
    const comment = text.trim();
    const refuse = (why: string): never => { throw new HarnessError('return-not-possible', `${detail.id} kann nicht zurück an den Agenten: ${why}`); };
    if (!comment) refuse('die Rückmeldung ist leer.');
    if (comment.length > MAX_RETURN_TEXT) refuse(`die Rückmeldung ist länger als ${MAX_RETURN_TEXT} Zeichen.`);
    if (detail.status !== 'blocked' || !isHumanInteraction(detail.blockedReason) || !detail.branch) {
      refuse('es wartet nicht auf Freigabe (nur Blockiert mit Grund „Human Interaction: …“ und Branch).');
    }
    const problem = await this.unavailable(detail);
    if (typeof problem === 'string') refuse(problem);
    if ((await ctx.stores.runs.list()).some((r) => r.ticketId === detail.id)) refuse('für das Ticket läuft gerade ein Lauf.');
    await ctx.comment(detail.id, `${DISPATCH_MARKERS.REVIEW_RETURN} (von ${actor.name}): ${comment}`);
    await ctx.services.tickets.update(detail.id, { status: 'planned', blockedReason: null, assignee: null }, actor, { force: true });
    ctx.log(`zurück an den Agenten ${detail.id} (von ${actor.name})`);
    return { ticketId: detail.id, text: `${detail.id} zurück an den Agenten – der nächste Lauf setzt im selben Worktree fort.` };
  }

  // Hauptordner des eigenen Boards, sonst warum es keinen Diff gibt
  private async unavailable(detail: BoardTicketDetail): Promise<{ root: string } | string> {
    if (!detail.branch) return 'Am Ticket ist kein Branch eingetragen.';
    const board = await this.ctx.board(detail.boardId);
    if (!board || board.ownerId !== this.ctx.me.userId || !board.repoPath) {
      return 'Das Board gehört einem anderen Benutzer oder hat keinen Repo-Pfad – der Diff ist nur auf dem Rechner des Dispatchers sichtbar.';
    }
    return { root: (await this.ctx.repo(board)).root };
  }

  // „n Dateien geändert (+a/−d)“; lässt sich der Diff nicht berechnen, ein Hinweis
  private async changes(board: Board | null, branch: string, target: string | null): Promise<string> {
    try {
      if (!board?.repoPath) return 'Diff nicht berechenbar (kein Repo-Pfad)';
      const { files } = this.diff.files((await this.ctx.repo(board)).root, branch, target);
      const add = files.reduce((n, f) => n + f.additions, 0);
      const del = files.reduce((n, f) => n + f.deletions, 0);
      return `${files.length} ${files.length === 1 ? 'Datei' : 'Dateien'} geändert (+${add}/−${del})`;
    } catch (error) {
      if (BoardRuntimeHolder.isConnectionFailure(error)) throw error;
      return `Diff nicht berechenbar (${(error as Error).message})`;
    }
  }

  // „x grün, y rot[, z ohne Ergebnis]“ oder „keine erkannt“
  private testSummary(tests: TestRun[]): string {
    if (!tests.length) return 'keine erkannt';
    const green = tests.filter((t) => t.ok === true).length;
    const red = tests.filter((t) => t.ok === false).length;
    const none = tests.length - green - red;
    return `${green} grün, ${red} rot${none ? `, ${none} ohne Ergebnis` : ''}`;
  }
}
