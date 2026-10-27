import { basename } from 'node:path';
import {
  DISPATCH_MARKERS, HarnessError, QuestionConcern, ResearchWorktree,
  type Board, type BoardQuestionActor, type BoardQuestionDetail, type BoardQuestionView, type RunEnd, type RunRecord,
} from '@harness/core';
import { BoardRuntimeHolder } from '../dbboard/BoardRuntimeHolder.js';
import type { DispatchContext } from './DispatchContext.js';

/** Ergebnis der Auswertung eines Recherche-Laufs */
export type ResearchOutcome = 'researched' | 'empty' | 'limit' | 'gone';

/** Bericht des automatischen Anstoßes eines Ticks */
export interface ResearchAutoReport {
  started: string[];
  errors: string[];
}

/** Fehler, die an der Frage selbst liegen: die Automatik merkt die Frage als erledigt und versucht sie nicht wieder */
const QUESTION_ERRORS = new Set(['research-no-repo', 'question-not-open', 'question-not-found']);
/** Nach einem Umgebungs- oder Repo-Fehler setzt die Automatik so lange aus */
export const RESEARCH_RETRY_MINUTES = 30;
/** Spielraum zwischen der Uhr des Dispatchers (Start des Laufs) und der Uhr der Datenbank (Zeitpunkt der Recherche) */
const CLOCK_SKEW_MS = 60_000;

/** Eine Frage in der Form, die die Wahl des Boards braucht */
type Locatable = Pick<BoardQuestionView, 'id' | 'projectId' | 'projectKey' | 'tickets'>;

// Recherche-Läufe zu offenen Fragen: Board mit Repo finden, losgelösten Worktree anlegen, `claude -p` nur lesend starten,
// danach auswerten (neue Recherche des Agents da? sonst Kommentar an der Frage) und den Worktree immer wieder entfernen.
export class ResearchRunner {
  private readonly worktrees = new ResearchWorktree();
  private readonly concern = new QuestionConcern();

  constructor(private readonly ctx: DispatchContext) {}

  // Name des Recherche-Agents einer Frage
  static agentName(questionId: string): string {
    return `research-${questionId}`;
  }

  // Kalendertag (Ortszeit) als JJJJ-MM-TT – Schlüssel des Tageszählers der Automatik
  static day(at: Date): string {
    return `${at.getFullYear()}-${String(at.getMonth() + 1).padStart(2, '0')}-${String(at.getDate()).padStart(2, '0')}`;
  }

  // Startet die Recherche einer offenen Frage (von Hand oder automatisch); Fehler als HarnessError
  async start(questionId: string, mode: 'manual' | 'auto'): Promise<RunRecord> {
    const { ctx } = this;
    const question = await ctx.services.questions.get(questionId);
    if (question.state !== 'open') {
      throw new HarnessError('question-not-open', `Frage ${question.id} ist nicht offen – recherchiert werden nur offene Fragen.`);
    }
    if (await this.running(question.id)) {
      throw new HarnessError('research-running', `Für Frage ${question.id} läuft schon eine Recherche.`);
    }
    const board = await this.locate(question);
    const work = await ctx.repo(board);
    const run = ctx.chooser.research();
    const agent = ResearchRunner.agentName(question.id);
    const worktree = await this.worktrees.create(work.root, question.id);
    if (worktree.warning) ctx.log(`Hinweis ${question.id}: ${worktree.warning}`);
    let record: RunRecord;
    try {
      record = await ctx.launch({
        kind: 'research', ticketId: question.id, questionId: question.id, boardId: board.id, title: question.title, agent,
        cwd: worktree.path, branch: '', target: null, run, prompt: ctx.prompts.research({ questionId: question.id, title: question.title, agent, cfg: ctx.cfg }),
        researchBefore: question.research.length, repoRoot: work.root,
      });
    } catch (error) {
      const left = this.worktrees.remove(work.root, worktree.path);
      if (left) ctx.log(`FEHLER ${question.id}: ${left}`);
      throw error;
    }
    const at = ctx.now().toISOString();
    await ctx.stores.state.update((s) => {
      const before = s.research[question.id];
      s.research[question.id] = {
        questionId: question.id, lastStartedAt: at, count: (before?.count ?? 0) + 1, autoAt: mode === 'auto' ? at : before?.autoAt ?? null,
      };
    });
    ctx.log(`Recherche gestartet ${question.id} (${mode === 'auto' ? 'automatisch' : 'von Hand'}) → ${agent} in ${worktree.path} [${run.profile}: ${run.model ?? '-'}/${run.effort ?? '-'}]`);
    return record;
  }

  // Wertet einen beendeten Recherche-Lauf aus; der Worktree wird in jedem Fall entfernt
  async evaluate(record: RunRecord, end: RunEnd, reason?: string): Promise<ResearchOutcome> {
    try {
      return await this.outcome(record, end, reason);
    } finally {
      if (record.repoRoot) {
        const left = this.worktrees.remove(record.repoRoot, record.cwd);
        if (left) this.ctx.log(`FEHLER ${record.ticketId}: ${left}`);
      }
    }
  }

  // Stößt neue offene Fragen ohne Recherche an, die den eigenen Benutzer betreffen (entstanden seit dem Einschalten der
  // Automatik, älteste zuerst, je Frage höchstens einmal, nie nach einem Lauf von Hand), bis research.wip gleichzeitige Läufe
  // und research.maxPerDay Starts am Tag. Fehler der Frage (kein Repo, nicht offen) merken sie als erledigt; Umgebungs- und
  // Repo-Fehler brechen ab, ohne zu merken, und setzen die Automatik RESEARCH_RETRY_MINUTES aus (je Fehlertext ein Bericht)
  async startAuto(): Promise<ResearchAutoReport> {
    const { ctx } = this;
    const report: ResearchAutoReport = { started: [], errors: [] };
    const now = ctx.now();
    const state = await ctx.stores.state.read();
    const auto = state.researchAuto;
    if (!auto.since || (auto.retryAt && new Date(auto.retryAt) > now)) return report;
    const today = ResearchRunner.day(now);
    let used = auto.day === today ? auto.count : 0;
    let free = ctx.cfg.research.wip - (await ctx.stores.runs.list()).filter((r) => r.kind === 'research').length;
    if (free <= 0 || used >= ctx.cfg.research.maxPerDay) return report;
    for (const question of await this.candidates(auto.since)) {
      if (free <= 0 || used >= ctx.cfg.research.maxPerDay) break;
      // Schon einmal angestoßen (automatisch oder von Hand): kein automatischer Neuversuch
      if (state.research[question.id] || (await this.running(question.id))) continue;
      try {
        await this.start(question.id, 'auto');
        report.started.push(question.id);
        free -= 1;
        used += 1;
        await ctx.stores.state.update((s) => {
          s.researchAuto = { ...s.researchAuto, day: today, count: used, retryAt: null, lastError: null };
        });
      } catch (error) {
        if (BoardRuntimeHolder.isConnectionFailure(error)) throw error;
        const message = (error as Error).message;
        if (error instanceof HarnessError && QUESTION_ERRORS.has(error.code)) {
          await this.markTried(question.id);
          report.errors.push(`${question.id}: ${message}`);
          continue;
        }
        const retryAt = new Date(now.getTime() + RESEARCH_RETRY_MINUTES * 60_000);
        await ctx.stores.state.update((s) => {
          s.researchAuto = { ...s.researchAuto, retryAt: retryAt.toISOString(), lastError: message };
        });
        if (auto.lastError !== message) {
          report.errors.push(`Recherche-Automatik ausgesetzt bis ${this.hhmm(retryAt)} (bei ${question.id}): ${message}`);
        }
        break;
      }
    }
    return report;
  }

  // Board, in dessen Repo recherchiert wird (siehe pick), sonst HarnessError „research-no-repo“
  async locate(question: Locatable): Promise<Board> {
    const board = this.pick(question, await this.ownBoards());
    if (board) return board;
    throw new HarnessError('research-no-repo',
      `Für Frage ${question.id} ist kein eigenes Board mit Repo-Pfad bekannt – die Frage mit einem Ticket eines eigenen Boards verknüpfen ` +
      `oder bei einem eigenen Board des Projekts ${question.projectKey} den Repo-Pfad setzen.`);
  }

  // Wahl des Boards aus den eigenen Boards: 1. Board eines verknüpften Tickets mit Repo-Pfad, 2. das aktive Board, wenn es zum
  // Projekt der Frage gehört und einen Repo-Pfad hat, 3. sonst das eigene Board des Projekts mit Repo-Pfad mit der kleinsten ID
  private pick(question: Locatable, own: Board[]): Board | null {
    const withRepo = own.filter((b) => b.repoPath);
    for (const ticket of question.tickets) {
      const board = withRepo.find((b) => b.id === ticket.boardId);
      if (board) return board;
    }
    const project = withRepo.filter((b) => b.projectId === question.projectId).sort((a, b) => a.id - b.id);
    return project.find((b) => b.id === this.ctx.me.activeBoard) ?? project[0] ?? null;
  }

  // Betrifft die Frage den eigenen Benutzer? Gemeinsame Regel „Eigene“ aus core (QuestionConcern)
  private concernsMe(question: BoardQuestionView, own: Board[]): boolean {
    const subject = { createdById: question.createdById, boardIds: question.tickets.map((t) => t.boardId) };
    return this.concern.concerns(subject, this.ctx.me.userId, new Set(own.map((b) => b.id)));
  }

  // Eigene Boards mit eigenem (wirksamem) Repo-Pfad
  private ownBoards(): Promise<Board[]> {
    return this.ctx.services.boards.owned(this.ctx.me.userId);
  }

  // Läuft für die Frage schon ein Recherche-Lauf?
  private async running(questionId: string): Promise<boolean> {
    return (await this.ctx.stores.runs.list()).some((r) => r.kind === 'research' && (r.questionId ?? r.ticketId) === questionId);
  }

  // Merkt eine nicht startbare Frage als angestoßen, damit die Automatik sie nicht jede Minute neu versucht
  private async markTried(questionId: string): Promise<void> {
    const at = this.ctx.now().toISOString();
    await this.ctx.stores.state.update((s) => {
      const before = s.research[questionId];
      s.research[questionId] = { questionId, lastStartedAt: before?.lastStartedAt ?? at, count: before?.count ?? 0, autoAt: at };
    });
  }

  // Kandidaten der Automatik: offene Fragen ohne Recherche, entstanden seit `since`, die den eigenen Benutzer betreffen und für
  // die es ein Board mit Repo gibt (dieselbe Wahl wie locate), älteste zuerst
  private async candidates(since: string): Promise<BoardQuestionView[]> {
    const { ctx } = this;
    const own = await this.ownBoards();
    const projectIds = new Set(own.filter((b) => b.repoPath).map((b) => b.projectId));
    if (!projectIds.size) return [];
    const projects = await ctx.services.db.transaction((u) => u.projects.list());
    const from = Date.parse(since);
    const found: BoardQuestionView[] = [];
    for (const project of projects.filter((p) => projectIds.has(p.id))) {
      const { questions } = await ctx.services.questions.list(project.key, { state: 'open' });
      found.push(...questions.filter((q) => !q.researched && Date.parse(q.createdAt) >= from && this.concernsMe(q, own) && this.pick(q, own)));
    }
    return found.sort((a, b) => a.createdAt.localeCompare(b.createdAt) || a.id.localeCompare(b.id));
  }

  // Hat der Lauf eine Recherche gespeichert? Zählt nur eine Recherche mit dem Agentnamen des Laufs als Autor ab dem Start des
  // Laufs; nur neue Recherchen ganz ohne Autor zählen über die Zahl der Recherchen beim Start
  private researched(question: BoardQuestionDetail, record: RunRecord): boolean {
    const started = Date.parse(record.startedAt) - CLOCK_SKEW_MS;
    if (question.research.some((r) => r.byName === record.agent && Date.parse(r.at) >= started)) return true;
    return question.research.slice(record.researchBefore ?? 0).some((r) => !r.byName?.trim());
  }

  // Steht der Kommentar „Recherche ohne Ergebnis“ dieses Laufs schon an der Frage (z. B. Verbindungsfehler danach)?
  private commented(question: BoardQuestionDetail, record: RunRecord): boolean {
    const started = Date.parse(record.startedAt) - CLOCK_SKEW_MS;
    return question.comments.some((c) => c.authorName === record.agent && c.body.startsWith(DISPATCH_MARKERS.RESEARCH_EMPTY) && Date.parse(c.createdAt) >= started);
  }

  // Auswertung ohne Aufräumen: neue Recherche → ok; Nutzungslimit/Claude fehlt → Pause; sonst Kommentar an der Frage.
  // Steht der Kommentar dieses Laufs schon da (Auswertung nach einem Verbindungsfehler wiederholt), wird nichts doppelt geschrieben
  private async outcome(record: RunRecord, end: RunEnd, reason?: string): Promise<ResearchOutcome> {
    const { ctx } = this;
    const id = record.questionId ?? record.ticketId;
    let question: BoardQuestionDetail;
    try {
      question = await ctx.services.questions.get(id);
    } catch (error) {
      if (error instanceof HarnessError && error.code === 'question-not-found') return 'gone';
      throw error;
    }
    if (this.researched(question, record)) {
      ctx.log(`Recherche ${id} gespeichert (${question.research.length} Recherche(n))`);
      return 'researched';
    }
    const actor: BoardQuestionActor = { ...ctx.backend.actor(record.agent), kind: 'session', session: null };
    const minutes = ctx.cfg.pauseMinutesOnLimit;
    const done = this.commented(question, record);
    if (!reason && (ctx.output.claudeMissing(end) || ctx.output.hitUsageLimit(end))) {
      if (done) return 'limit';
      const missing = ctx.output.claudeMissing(end);
      const until = new Date(ctx.now().getTime() + minutes * 60_000).toISOString();
      await ctx.stores.state.update((s) => { s.pausedUntil = until; });
      const why = missing ? `Claude Code wurde nicht gefunden (Befehl „${record.command}“)` : 'Nutzungslimit erreicht';
      await ctx.services.questions.comment(id,
        `${DISPATCH_MARKERS.RESEARCH_EMPTY}: ${why}. Der Dispatcher startet ${minutes} min lang nichts Neues; die Recherche danach bitte erneut starten.`, actor);
      ctx.log(`Pause bis ${until}: ${why} (Recherche ${id})`);
      await ctx.notify({ kind: 'paused', ticketId: null, title: question.title, text: `${why} – Pause ${minutes} min`, repoPath: null });
      return 'limit';
    }
    if (done) return 'empty';
    const summary = ctx.output.summarize(end.output);
    const why = reason ?? ctx.output.abortReason(end, summary, record.run, 'Lauf endete, ohne eine Recherche an der Frage zu speichern');
    const lines = [`${DISPATCH_MARKERS.RESEARCH_EMPTY}: ${why}.`, 'Kein automatischer Neuversuch – „Recherche starten“ auf der Seite der Frage stößt eine neue an.'];
    const shown = ctx.output.tail(ctx.output.shown(end.output, summary));
    if (shown) lines.push('', 'Letzte Ausgabe:', '```', shown, '```');
    lines.push('', `Protokoll: ${basename(record.logFile)}`);
    await ctx.services.questions.comment(id, lines.join('\n'), actor);
    ctx.log(`Recherche ${id} ohne Ergebnis: ${why}`);
    return 'empty';
  }

  // HH:MM Ortszeit
  private hhmm(d: Date): string {
    return `${String(d.getHours()).padStart(2, '0')}:${String(d.getMinutes()).padStart(2, '0')}`;
  }
}
