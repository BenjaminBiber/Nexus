import { existsSync } from 'node:fs';
import {
  BoardConfigStore, DispatchConfigNormalizer, DispatchConfigStore, DispatchLog, DispatchPrompt, DispatchStateStore, HarnessError, PRIORITY_RANK, QuestionIds, ResearchWorktree, ReviewStore, RunLogSplitter, RunStore, TimeWindowCheck,
  type Board, type BoardTicketView, type DispatchPromptKind, type MergeEntry, type RunEnd, type RunLogParts, type RunRecord, type DispatchConfig, type HarnessHome, type RepoBoardOwner,
} from '@harness/core';
import type { BoardBackend } from '../dbboard/BoardBackend.js';
import { BoardRuntimeHolder } from '../dbboard/BoardRuntimeHolder.js';
import { DispatchContext, type DispatchNotice, type DispatchStores, type RepoWork } from './DispatchContext.js';
import type { DispatchMergeInfo, DispatchRunInfo, DispatchRunLog, DispatchStatus, DispatchTicketRow, QuestionResearchStatus, TicketPlanningStatus } from './DispatchStatus.js';
import { MergeResultReader, type MergeRetryResult } from './MergeResultReader.js';
import { MergeService } from './MergeService.js';
import { PlanRunner } from './PlanRunner.js';
import { PromptValues } from './PromptValues.js';
import { ResearchRunner } from './ResearchRunner.js';
import { ResultHandler } from './ResultHandler.js';
import { RunRecorder } from './RunRecorder.js';
import { ReviewService, type ReviewView } from './ReviewService.js';
import { TicketStarter } from './TicketStarter.js';
import type { WorkerLauncher } from './WorkerLauncher.js';

/** Nach so vielen Minuten über dem Zeitlimit gilt ein Lauf ohne Ergebnis als verschwunden, auch wenn die PID lebt */
const OVERDUE_MINUTES = 10;
/** Nach so vielen Minuten gilt eine Übernahme (Geplant mit Agent, ohne Lauf) als hängengeblieben */
const CLAIM_STALE_MINUTES = 5;

export interface DispatchServiceDeps {
  home: HarnessHome;
  boards: BoardRuntimeHolder;
  launcher: WorkerLauncher;
  notify?: (notice: DispatchNotice) => Promise<void> | void;
  clock?: () => Date;
  /** Läuft der Prozess noch? (Standard: process.kill(pid, 0)) */
  isAlive?: (pid: number) => boolean;
}

/** Eingabe der Vorschau: Laufart, zu prüfende Vorlage (leer = Standard) und optional Ticket bzw. Frage */
export interface PromptPreviewInput {
  kind: DispatchPromptKind;
  template?: string | null;
  id?: string | null;
}

export interface PromptPreview {
  kind: DispatchPromptKind;
  /** Ticket- bzw. Fragen-ID der echten Werte; null = Beispielwerte */
  id: string | null;
  sample: boolean;
  prompt: string;
}

export interface DispatchReport {
  at: string;
  /** Warum nichts Neues gestartet wurde, sonst null */
  idle: string | null;
  started: string[];
  queued: string[];
  evaluated: Array<{ ticketId: string; outcome: string }>;
  merged: string[];
  /** Automatisch gestartete Recherchen (Fragen-IDs) */
  research: string[];
  /** Automatisch gestartete Planungen (Ticket-IDs) */
  planned: string[];
  errors: string[];
}

/** Bericht von „Erneut mergen“ mit dem Ergebnis für das angefragte Ticket */
export interface MergeRetryReport extends DispatchReport {
  result: MergeRetryResult;
}

// Herz des Dispatchers: ein Tick wertet Läufe aus, gibt Verwaiste zurück, mergt Erledigtes und startet bereite Tickets bis zum WIP-Limit.
export class DispatchService {
  readonly stores: DispatchStores;
  lastReport: DispatchReport | null = null;
  private running: Promise<DispatchReport> | null = null;
  /** Sperre: Ticks und „Erneut mergen“ laufen nacheinander */
  private lock: Promise<unknown> = Promise.resolve();
  private readonly clock: () => Date;
  private readonly isAlive: (pid: number) => boolean;
  private readonly window = new TimeWindowCheck();
  private readonly researchWorktrees = new ResearchWorktree();
  private readonly configs: DispatchConfigStore;

  constructor(private readonly deps: DispatchServiceDeps) {
    this.clock = deps.clock ?? (() => new Date());
    this.isAlive = deps.isAlive ?? DispatchService.processAlive;
    this.stores = { runs: new RunStore(deps.home), state: new DispatchStateStore(deps.home), log: new DispatchLog(deps.home, this.clock), reviews: new ReviewStore(deps.home) };
    this.configs = new DispatchConfigStore(deps.home);
  }

  // Zählt der Lauf zum WIP der Tickets? Nur Umsetzung und Merge-Session; Recherchen und Planungen laufen nebenher
  static countsForWip(run: RunRecord): boolean {
    return run.kind === 'work' || run.kind === 'merge';
  }

  // Prüft per Signal 0, ob ein Prozess existiert
  static processAlive(pid: number): boolean {
    try {
      process.kill(pid, 0);
      return true;
    } catch (error) {
      return (error as NodeJS.ErrnoException).code === 'EPERM';
    }
  }

  // Ein Durchlauf; steht schon einer an, teilen sich beide Aufrufe dessen Ergebnis. Läuft gerade „Erneut mergen“,
  // wartet der Durchlauf auf dessen Ende (gemeinsame Sperre)
  tick(): Promise<DispatchReport> {
    if (!this.running) {
      this.running = this.exclusive(() => this.runTick()).finally(() => {
        this.running = null;
      });
    }
    return this.running;
  }

  // Führt `work` aus, sobald alle vorher angemeldeten Arbeiten (Ticks, „Erneut mergen“) fertig sind
  private exclusive<T>(work: () => Promise<T>): Promise<T> {
    const next = this.lock.then(work);
    this.lock = next.catch(() => undefined);
    return next;
  }

  // Pausiert neue Starts für `minutes` Minuten
  async pause(minutes = 60): Promise<void> {
    const until = new Date(this.clock().getTime() + minutes * 60_000).toISOString();
    await this.stores.state.update((s) => { s.pausedUntil = until; });
    this.stores.log.line(`Pause bis ${until} (von Hand)`);
  }

  // Hebt die Pause auf
  async resume(): Promise<void> {
    await this.stores.state.update((s) => { s.pausedUntil = null; });
    this.stores.log.line('Pause aufgehoben');
  }

  // Schaltet den Dispatcher ein oder aus (schreibt dispatch.yaml)
  async setEnabled(on: boolean): Promise<void> {
    const { raw } = await this.configs.read();
    await this.configs.write({ ...raw, enabled: on });
    this.stores.log.line(on ? 'eingeschaltet' : 'ausgeschaltet');
  }

  // „Erneut mergen“ (Mensch): unter der Sperre der Ticks Auftrag setzen und sofort einen Tick ausführen (kein Timer-Tick
  // dazwischen); liefert den Bericht samt Ergebnis für dieses Ticket. Fehler (kein Merge möglich, fremdes Board,
  // Merge-Session läuft noch) werden geworfen
  retryMerge(id: string): Promise<MergeRetryReport> {
    return this.exclusive(async () => {
      const cfg = (await this.configs.read()).config;
      const board = await new BoardConfigStore(this.deps.home).require();
      const ticketId = await this.deps.boards.run(async (backend) => {
        const ctx = new DispatchContext(cfg, { userId: board.userId, activeBoard: board.activeBoard }, backend, this.stores, this.deps.launcher, this.clock, this.deps.notify ?? (() => undefined));
        return new MergeService(ctx).retry(id, backend.actor());
      });
      const report = await this.runTick();
      const detail = await this.deps.boards.run(async (backend) => backend.services.tickets.get(ticketId)).catch(() => null);
      const entry = (await this.stores.state.read()).merges[ticketId];
      return { ...report, result: new MergeResultReader().read(ticketId, report.merged, report.errors, detail, entry) };
    });
  }

  // „Abnehmen“ (Mensch) eines Human-Interaction-Tickets: unter der Sperre der Ticks erledigen, auf eigenem Board den Merge
  // anstoßen und sofort einen Tick ausführen; liefert den Bericht samt Ergebnis (gemergt, wartet … oder nur erledigt)
  accept(id: string, by?: string): Promise<MergeRetryReport> {
    return this.exclusive(async () => {
      const cfg = (await this.configs.read()).config;
      const board = await new BoardConfigStore(this.deps.home).require();
      const accepted = await this.deps.boards.run(async (backend) => {
        const ctx = new DispatchContext(cfg, { userId: board.userId, activeBoard: board.activeBoard }, backend, this.stores, this.deps.launcher, this.clock, this.deps.notify ?? (() => undefined));
        return new MergeService(ctx).accept(id, backend.actor(by?.trim() || undefined));
      });
      const report = await this.runTick();
      if (!accepted.merge) return { ...report, result: { outcome: 'accepted', text: accepted.text } };
      const detail = await this.deps.boards.run(async (backend) => backend.services.tickets.get(accepted.ticketId)).catch(() => null);
      const entry = (await this.stores.state.read()).merges[accepted.ticketId];
      return { ...report, result: new MergeResultReader().read(accepted.ticketId, report.merged, report.errors, detail, entry) };
    });
  }

  // Review vor dem Merge: Dateien des Ticket-Branches gegen das Ziel und Testläufe des Agenten
  async review(id: string): Promise<ReviewView> {
    return this.withContext((ctx) => new ReviewService(ctx).view(id));
  }

  // Patch einer Datei aus der Dateiliste des Reviews
  async reviewFile(id: string, path: string): Promise<{ path: string; patch: string; truncated: boolean }> {
    return this.withContext((ctx) => new ReviewService(ctx).fileDiff(id, path));
  }

  // „Zurück an den Agenten“ (Mensch) mit Rückmeldung: unter der Sperre der Ticks zurück nach Geplant, ohne sofortigen Tick;
  // der nächste Durchlauf setzt im selben Worktree fort
  returnToAgent(id: string, comment: string, by?: string): Promise<MergeRetryReport> {
    return this.exclusive(async () => {
      const returned = await this.deps.boards.run(async (backend) => {
        const ctx = await this.contextFor(backend);
        return new ReviewService(ctx).returnToAgent(id, comment, backend.actor(by?.trim() || undefined));
      });
      const report: DispatchReport = { at: this.clock().toISOString(), idle: null, started: [], queued: [], evaluated: [], merged: [], research: [], planned: [], errors: [] };
      return { ...report, result: { outcome: 'returned', text: returned.text } };
    });
  }

  // Vorschau einer (noch ungespeicherten) Vorlage: rendert sie mit den echten Werten eines Tickets bzw. einer Frage
  // oder mit Beispielwerten. Schreibt nichts und läuft bewusst ohne Sperre, damit sie keinen Tick blockiert.
  async previewPrompt(input: PromptPreviewInput): Promise<PromptPreview> {
    const { kind } = input;
    // Leer gilt wie in der Datei als „Standardvorlage“; Zeilenenden wie beim Speichern vereinheitlichen
    const template = typeof input.template === 'string' && input.template.trim() ? input.template.replace(/\r\n/g, '\n') : null;
    if (template) {
      const problems = new DispatchConfigNormalizer().promptProblems({ prompts: { [kind]: template } });
      if (problems.length) throw DispatchConfigNormalizer.promptError(problems);
    }
    const base = (await this.configs.read()).config;
    // Über die Einstellung rendern, nicht an ihr vorbei: so gilt dieselbe Fallback-Logik wie im Lauf
    const cfg: DispatchConfig = { ...base, prompts: { ...base.prompts, [kind]: template } };
    const id = typeof input.id === 'string' && input.id.trim() ? input.id.trim() : null;
    const prompts = new DispatchPrompt();
    if (!id) {
      const sample = PromptValues.sample(cfg);
      const prompt = kind === 'work' ? prompts.work(sample.work)
        : kind === 'plan' ? prompts.plan(sample.plan)
          : kind === 'research' ? prompts.research(sample.research) : prompts.merge(sample.merge);
      return { kind, id: null, sample: true, prompt };
    }
    return this.previewWithBoard(cfg, async (ctx) => {
      const values = new PromptValues(ctx);
      if (kind === 'research') {
        const question = await ctx.services.questions.get(new QuestionIds().canonical(id));
        return { kind, id: question.id, sample: false, prompt: prompts.research(values.research(question)) };
      }
      const detail = await ctx.services.tickets.get(id.toUpperCase());
      const prompt = kind === 'work' ? prompts.work(await values.work(detail))
        : kind === 'plan' ? prompts.plan(values.plan(detail)) : prompts.merge(values.merge(detail));
      return { kind, id: detail.id, sample: false, prompt };
    });
  }

  // Kontext des eigenen Benutzers mit einer abweichenden Einstellung (für die Vorschau; nur lesend, ohne Sperre)
  private previewWithBoard<T>(cfg: DispatchConfig, work: (ctx: DispatchContext) => Promise<T>): Promise<T> {
    return this.deps.boards.run(async (backend) => {
      const board = await new BoardConfigStore(this.deps.home).require();
      return work(new DispatchContext(cfg, { userId: board.userId, activeBoard: board.activeBoard }, backend, this.stores, this.deps.launcher, this.clock, this.deps.notify ?? (() => undefined)));
    });
  }

  // Führt `work` mit einem Kontext des eigenen Benutzers aus (ohne Sperre, nur lesend)
  private withContext<T>(work: (ctx: DispatchContext) => Promise<T>): Promise<T> {
    return this.deps.boards.run(async (backend) => work(await this.contextFor(backend)));
  }

  // Kontext aus aktueller Einstellung und eigenem Benutzer
  private async contextFor(backend: BoardBackend): Promise<DispatchContext> {
    const cfg = (await this.configs.read()).config;
    const board = await new BoardConfigStore(this.deps.home).require();
    return new DispatchContext(cfg, { userId: board.userId, activeBoard: board.activeBoard }, backend, this.stores, this.deps.launcher, this.clock, this.deps.notify ?? (() => undefined));
  }

  // „Recherche starten“ (von Hand): startet sofort, unabhängig von Ein/Aus, Zeitfenster und Opt-in; abgelehnt bei Pause
  // (z. B. Nutzungslimit), nicht offener Frage, schon laufender Recherche oder fehlendem Repo. Läuft unter der Sperre der Ticks
  startResearch(questionId: string): Promise<DispatchRunInfo> {
    return this.exclusive(async () => {
      const cfg = (await this.configs.read()).config;
      const paused = (await this.stores.state.read()).pausedUntil;
      if (paused && new Date(paused) > this.clock()) {
        throw new HarnessError('dispatch-paused',
          `Der Dispatcher pausiert bis ${this.hhmm(new Date(paused))} (z. B. wegen Nutzungslimit) – die Recherche danach starten oder die Pause aufheben.`);
      }
      const board = await new BoardConfigStore(this.deps.home).require();
      const record = await this.deps.boards.run(async (backend) => {
        const ctx = new DispatchContext(cfg, { userId: board.userId, activeBoard: board.activeBoard }, backend, this.stores, this.deps.launcher, this.clock, this.deps.notify ?? (() => undefined));
        return new ResearchRunner(ctx).start(questionId, 'manual');
      });
      return this.runInfo(record);
    });
  }

  // Läuft für die Frage gerade eine Recherche? Dazu Pause und Zähler (ohne Datenbank)
  async researchStatus(questionId: string): Promise<QuestionResearchStatus> {
    const id = new QuestionIds().canonical(questionId);
    const run = (await this.stores.runs.list()).find((r) => r.kind === 'research' && (r.questionId ?? r.ticketId) === id);
    const state = await this.stores.state.read();
    const paused = state.pausedUntil && new Date(state.pausedUntil) > this.clock() ? state.pausedUntil : null;
    const entry = state.research[id];
    return { questionId: id, run: run ? this.runInfo(run) : null, pausedUntil: paused, lastStartedAt: entry?.lastStartedAt ?? null, count: entry?.count ?? 0 };
  }

  // „Planen“ (von Hand): startet sofort, unabhängig von Ein/Aus, Zeitfenster und Opt-in; abgelehnt bei Pause (z. B.
  // Nutzungslimit), fremdem Board, falschem Status, offener Frage oder schon laufender Planung. Läuft unter der Sperre der Ticks
  startPlanning(ticketId: string): Promise<DispatchRunInfo> {
    return this.exclusive(async () => {
      const cfg = (await this.configs.read()).config;
      const paused = (await this.stores.state.read()).pausedUntil;
      if (paused && new Date(paused) > this.clock()) {
        throw new HarnessError('dispatch-paused',
          `Der Dispatcher pausiert bis ${this.hhmm(new Date(paused))} (z. B. wegen Nutzungslimit) – die Planung danach starten oder die Pause aufheben.`);
      }
      const board = await new BoardConfigStore(this.deps.home).require();
      const record = await this.deps.boards.run(async (backend) => {
        const ctx = new DispatchContext(cfg, { userId: board.userId, activeBoard: board.activeBoard }, backend, this.stores, this.deps.launcher, this.clock, this.deps.notify ?? (() => undefined));
        return new PlanRunner(ctx).start(ticketId);
      });
      return this.runInfo(record);
    });
  }

  // Läuft für das Ticket gerade eine Planung? Dazu die Pause (ohne Datenbank)
  async planningStatus(ticketId: string): Promise<TicketPlanningStatus> {
    const id = ticketId.trim().toUpperCase();
    const run = (await this.stores.runs.list()).find((r) => r.kind === 'plan' && r.ticketId === id);
    const state = await this.stores.state.read();
    const paused = state.pausedUntil && new Date(state.pausedUntil) > this.clock() ? state.pausedUntil : null;
    return { ticketId: id, run: run ? this.runInfo(run) : null, pausedUntil: paused };
  }

  // Läufe eines Tickets oder einer Frage aus den Protokollen, neueste zuerst; `running`: der Lauf steht im RunStore und
  // der Worker hat noch kein Ergebnis geschrieben
  async ticketRuns(id: string): Promise<DispatchRunLog[]> {
    const logs = await this.stores.log.runs(id);
    const names = new Set(logs.map((l) => l.name));
    const active = new Set<string>();
    for (const r of await this.stores.runs.list()) {
      if (names.has(`${r.id}.log`) && !(await this.stores.runs.result(r.id))) active.add(`${r.id}.log`);
    }
    return logs.map((l) => ({ ...l, running: active.has(l.name) }));
  }

  // Ein Lauf-Protokoll (Ende der Datei), zerlegt in Kopf, Arbeitsanweisung und Ausgabe
  async runLog(name: string): Promise<RunLogParts & { name: string }> {
    return { name, ...new RunLogSplitter().split(await this.stores.log.read(name)) };
  }

  // Lauf für Oberfläche und CLI
  private runInfo(r: RunRecord): DispatchRunInfo {
    return {
      id: r.id, kind: r.kind, ticketId: r.ticketId, title: r.title, profile: r.run.profile, model: r.run.model, effort: r.run.effort,
      startedAt: r.startedAt, log: `${r.id}.log`, ...(r.questionId ? { questionId: r.questionId } : {}),
    };
  }

  // Ein ausdrücklich genanntes Board muss es geben ('board-not-found' wie bei den anderen Befehlen);
  // ist die DB nicht erreichbar, prüft erst der Status (Meldung in ticketsError)
  async requireBoard(boardId?: number): Promise<void> {
    if (boardId === undefined) return;
    try {
      await this.deps.boards.run((backend) => backend.services.boards.require(boardId));
    } catch (error) {
      if ((error as { code?: string }).code === 'board-not-found') throw error;
    }
  }

  // Zustand für Oberfläche und CLI; mit Board (Standard: aktives Board) auch die Bereitschaft seiner Geplant-Tickets
  async status(requested?: number): Promise<DispatchStatus> {
    const boardId = requested ?? (await this.activeBoard());
    let cfg: DispatchConfig | null = null;
    let configError: string | null = null;
    try {
      cfg = (await this.configs.read()).config;
    } catch (error) {
      configError = (error as Error).message;
    }
    const now = this.clock();
    const state = await this.stores.state.read();
    const all = await this.stores.runs.list();
    const runs = all.filter((r) => DispatchService.countsForWip(r));
    const researching = all.filter((r) => r.kind === 'research').length;
    const planning = all.filter((r) => r.kind === 'plan').length;
    const paused = state.pausedUntil && new Date(state.pausedUntil) > now ? state.pausedUntil : null;
    const wip = cfg?.wip ?? 0;
    let tickets: DispatchTicketRow[] = [];
    let boardInScope = false;
    let ticketsError: string | null = null;
    if (boardId && cfg) {
      try {
        ({ tickets, boardInScope } = await this.rows(cfg, boardId, Math.max(0, wip - runs.length)));
      } catch (error) {
        ticketsError = (error as Error).message;
      }
    }
    const kind = configError ? 'error' : !cfg!.enabled ? 'off' : paused ? 'paused' : !this.window.contains(now, cfg!.window) ? 'outside' : runs.length ? 'running' : 'idle';
    const message = {
      error: `Einstellung fehlerhaft: ${configError}`,
      off: 'Dispatcher ist ausgeschaltet',
      paused: `pausiert bis ${this.hhmm(new Date(paused ?? now))}`,
      outside: cfg?.window ? `außerhalb ${cfg.window.from}–${cfg.window.to}` : '',
      running: `läuft · ${runs.length}/${wip}`,
      idle: `bereit · 0/${wip}`,
    }[kind];
    return {
      enabled: cfg?.enabled ?? false, state: kind, message, pausedUntil: paused, wip, active: runs.length, planning,
      runs: all.map((r) => this.runInfo(r)),
      research: {
        auto: cfg?.research.auto ?? false, wip: cfg?.research.wip ?? 1, active: researching, maxPerDay: cfg?.research.maxPerDay ?? 10,
        today: state.researchAuto.day === ResearchRunner.day(now) ? state.researchAuto.count : 0,
        since: state.researchAuto.since,
        retryAt: state.researchAuto.retryAt && new Date(state.researchAuto.retryAt) > now ? state.researchAuto.retryAt : null,
        error: state.researchAuto.lastError,
      },
      planningAuto: {
        auto: cfg?.planning.auto ?? false, wip: cfg?.planning.wip ?? 1, active: planning,
        since: state.planningAuto.since,
        retryAt: state.planningAuto.retryAt && new Date(state.planningAuto.retryAt) > now ? state.planningAuto.retryAt : null,
        error: state.planningAuto.lastError,
      },
      merges: await this.mergeInfos(Object.values(state.merges)),
      boardId: boardId ?? null, boardInScope, tickets, ticketsError,
      lastTick: this.lastReport?.at ?? null, lastErrors: this.lastReport?.errors ?? [],
      configFile: this.configs.file, configError, agentPrefix: cfg?.agentPrefix ?? 'junior-',
    };
  }

  // Ablauf eines Ticks; Fehler landen im Bericht und in dispatch.log
  private async runTick(): Promise<DispatchReport> {
    const report: DispatchReport = { at: this.clock().toISOString(), idle: null, started: [], queued: [], evaluated: [], merged: [], research: [], planned: [], errors: [] };
    try {
      const cfg = (await this.configs.read()).config;
      // Ausgeschaltet (auch die Recherche-Automatik) und nichts offen: keine Verbindung zur Datenbank nötig (kein Fehler je Minute ohne Board-Konfig)
      if (!cfg.enabled && !cfg.research.auto && !cfg.planning.auto && !(await this.stores.runs.list()).length && !Object.keys((await this.stores.state.read()).merges).length) {
        await this.followResearchAuto(false);
        await this.followPlanningAuto(false);
        report.idle = 'Dispatcher ist ausgeschaltet';
        this.lastReport = report;
        return report;
      }
      await this.followResearchAuto(cfg.research.auto);
      await this.followPlanningAuto(cfg.planning.auto);
      const board = await new BoardConfigStore(this.deps.home).require();
      const me: RepoBoardOwner = { userId: board.userId, activeBoard: board.activeBoard };
      await this.deps.boards.run(async (backend) => {
        const ctx = new DispatchContext(cfg, me, backend, this.stores, this.deps.launcher, this.clock, this.deps.notify ?? (() => undefined));
        const merges = new MergeService(ctx);
        const results = new ResultHandler(ctx, (record, end) => merges.afterSession(record, end));
        const research = new ResearchRunner(ctx);
        await this.evaluateRuns(ctx, results, research, report);
        await this.removeResearchOrphans(ctx);
        await this.releaseStale(ctx, results, report);
        await merges.processAll(report);
        await this.startReady(ctx, results, research, report);
        // Vorgemerkte Läufe (ausgewertet oder gescheiterter Start) in die Auswertung schreiben
        await new RunRecorder(ctx).flush();
      });
    } catch (error) {
      report.errors.push((error as Error).message);
    }
    for (const e of report.errors) this.stores.log.line(`FEHLER ${e}`);
    this.lastReport = report;
    return report;
  }

  // Beendete Läufe auswerten; Worker ohne Ergebnis, deren Prozess fehlt oder die weit über dem Zeitlimit liegen
  // (Prozess-ID vermutlich neu vergeben), gelten als abgestürzt. Verbindungsfehler brechen den Tick ab, der Lauf bleibt.
  private async evaluateRuns(ctx: DispatchContext, results: ResultHandler, research: ResearchRunner, report: DispatchReport): Promise<void> {
    const recorder = new RunRecorder(ctx);
    for (const record of await this.stores.runs.list()) {
      let end: RunEnd | null = await this.stores.runs.result(record.id);
      let reason: string | undefined;
      if (!end) {
        reason = this.vanished(record);
        if (!reason) continue;
        end = { code: null, timedOut: false, output: '', endedAt: this.clock().toISOString() };
      }
      try {
        const result = record.kind === 'research' ? { outcome: await research.evaluate(record, end, reason) }
          : record.kind === 'plan' ? { outcome: await new PlanRunner(ctx).evaluate(record, end, reason) }
            : await results.handle(record, end, reason);
        report.evaluated.push({ ticketId: record.ticketId, outcome: result.outcome });
        ctx.log(`beendet ${record.ticketId} (${record.kind}): ${result.outcome}`);
        // Vor dem Entfernen vormerken: so kommt jeder ausgewertete Lauf genau einmal in die Auswertung (Planungen lässt der Recorder aus)
        await recorder.keep(record, end, result);
      } catch (error) {
        if (BoardRuntimeHolder.isConnectionFailure(error)) throw error;
        report.errors.push(`${record.ticketId}: ${(error as Error).message}`);
      }
      await this.stores.runs.remove(record.id);
    }
  }

  // Recherche-Automatik nachführen: an ohne Zeitpunkt → ab jetzt (nur neuere Fragen); aus → Zeitpunkt vergessen, damit
  // erneutes Einschalten neu beginnt
  private async followResearchAuto(on: boolean): Promise<void> {
    const since = (await this.stores.state.read()).researchAuto.since;
    if (on === Boolean(since)) return;
    await this.researchAutoSwitched(on);
  }

  // Die Recherche-Automatik wurde ein- oder ausgeschaltet (Einstellung gespeichert oder vom Tick bemerkt): beim Einschalten
  // gilt sie ab jetzt, Aussetzen nach Fehlern und Fehlertext beginnen neu; beim Ausschalten wird der Zeitpunkt vergessen
  async researchAutoSwitched(on: boolean): Promise<void> {
    const at = this.clock().toISOString();
    await this.stores.state.update((s) => {
      s.researchAuto = on ? { ...s.researchAuto, since: at, retryAt: null, lastError: null } : { ...s.researchAuto, since: null };
    });
    this.stores.log.line(on ? `Recherche-Automatik an (für Fragen ab ${at})` : 'Recherche-Automatik aus');
  }

  // Planungs-Automatik nachführen: an ohne Zeitpunkt → ab jetzt (nur neuere Tickets); aus → Zeitpunkt vergessen
  private async followPlanningAuto(on: boolean): Promise<void> {
    const since = (await this.stores.state.read()).planningAuto.since;
    if (on === Boolean(since)) return;
    await this.planningAutoSwitched(on);
  }

  // Die Planungs-Automatik wurde ein- oder ausgeschaltet (Einstellung gespeichert oder vom Tick bemerkt): beim Einschalten
  // gilt sie ab jetzt, Aussetzen nach Fehlern und Fehlertext beginnen neu; beim Ausschalten wird der Zeitpunkt vergessen
  async planningAutoSwitched(on: boolean): Promise<void> {
    const at = this.clock().toISOString();
    await this.stores.state.update((s) => {
      s.planningAuto = on ? { ...s.planningAuto, since: at, retryAt: null, lastError: null } : { ...s.planningAuto, since: null };
    });
    this.stores.log.line(on ? `Planungs-Automatik an (für Tickets ab ${at})` : 'Planungs-Automatik aus');
  }

  // Verwaiste Recherche- und Planungs-Worktrees (ohne Lauf) in den Repos der eigenen Boards entfernen; Fehler nur ins Protokoll
  private async removeResearchOrphans(ctx: DispatchContext): Promise<void> {
    const runs = await this.stores.runs.list();
    const active = runs.filter((r) => r.kind === 'research').map((r) => r.cwd);
    const planning = runs.filter((r) => r.kind === 'plan').map((r) => r.cwd);
    const plans = new PlanRunner(ctx);
    const boards = await ctx.services.boards.owned(ctx.me.userId);
    const repos = new Set(boards.map((b) => b.repoPath).filter((p): p is string => Boolean(p)));
    for (const repo of repos) {
      for (const note of this.researchWorktrees.removeOrphans(repo, active)) ctx.log(note);
      for (const note of plans.removeOrphans(repo, planning)) ctx.log(note);
    }
  }

  // Grund, warum ein Lauf ohne Ergebnis als verschwunden gilt, sonst undefined
  private vanished(record: RunRecord): string | undefined {
    const age = this.clock().getTime() - Date.parse(record.startedAt);
    // Die Einrichtung (worktreeSetup) läuft vor Claude und hat ein eigenes Zeitlimit je Befehl
    const setupMinutes = (record.setup?.length ?? 0) * (record.setupTimeoutMinutes ?? 0);
    if (age > (record.run.timeoutMinutes + setupMinutes + OVERDUE_MINUTES) * 60_000) {
      return `Worker lieferte ${OVERDUE_MINUTES} min nach dem Zeitlimit noch kein Ergebnis (Prozess hängt oder Prozess-ID neu vergeben)`;
    }
    const gone = record.pid === null ? age > 120_000 : !this.isAlive(record.pid);
    return gone ? 'Worker-Prozess endete ohne Ergebnis (abgestürzt oder Rechner neu gestartet)' : undefined;
  }

  // Boards im Bereich, die dem eigenen Benutzer gehören (fremde Boards bedient deren Besitzer)
  private async ownBoards(ctx: DispatchContext): Promise<Board[]> {
    const boards: Board[] = [];
    for (const boardId of ctx.scope()) {
      const board = await ctx.board(boardId);
      if (board && board.ownerId === ctx.me.userId) boards.push(board);
    }
    return boards;
  }

  // Agent-Tickets ohne Lauf zurückgeben: In Arbeit ohne Änderung seit staleMinutes (zählt als Versuch),
  // Geplant mit Agent seit CLAIM_STALE_MINUTES (hängende Übernahme, zählt nicht)
  private async releaseStale(ctx: DispatchContext, results: ResultHandler, report: DispatchReport): Promise<void> {
    const active = new Set((await this.stores.runs.list()).map((r) => r.ticketId));
    const limit = ctx.cfg.staleMinutes * 60_000;
    for (const board of await this.ownBoards(ctx)) {
      let views: BoardTicketView[];
      try {
        views = await ctx.services.tickets.list({ boardIds: [board.id] });
      } catch (error) {
        if (BoardRuntimeHolder.isConnectionFailure(error)) throw error;
        report.errors.push(`Board ${board.id}: ${(error as Error).message}`);
        continue;
      }
      for (const view of views) {
        if (!ctx.isAgent(view.assignee) || active.has(view.id)) continue;
        const idle = ctx.now().getTime() - Date.parse(view.updatedAt);
        if (view.status === 'planned' && idle > CLAIM_STALE_MINUTES * 60_000) {
          try {
            await ctx.services.tickets.release(view.id, ctx.actor, { status: 'planned' });
            ctx.log(`Übernahme ${view.id} durch ${view.assignee} hing seit mehr als ${CLAIM_STALE_MINUTES} min – freigegeben`);
          } catch (error) {
            if (BoardRuntimeHolder.isConnectionFailure(error)) throw error;
            report.errors.push(`${view.id}: ${(error as Error).message}`);
          }
          continue;
        }
        if (view.status !== 'in_progress' || idle <= limit) continue;
        try {
          const { outcome } = await results.abort(await ctx.services.tickets.get(view.id),
            `Kein Lebenszeichen seit mehr als ${ctx.cfg.staleMinutes} min (Worker vermutlich abgestürzt oder Rechner neu gestartet)`, '', null);
          report.evaluated.push({ ticketId: view.id, outcome });
        } catch (error) {
          if (BoardRuntimeHolder.isConnectionFailure(error)) throw error;
          report.errors.push(`${view.id}: ${(error as Error).message}`);
        }
      }
    }
  }

  // Bereite Tickets bis zum WIP-Limit starten (nur eingeschaltet, ohne Pause, im Zeitfenster), danach – unabhängig vom Schalter
  // für Tickets – die Recherche-Automatik. Fachliche Fehler eines Boards (z. B. Repo-Pfad kein Git-Repo) oder Tickets landen im
  // Bericht, die übrigen werden trotzdem bedient; Verbindungsfehler brechen ab
  private async startReady(ctx: DispatchContext, results: ResultHandler, research: ResearchRunner, report: DispatchReport): Promise<void> {
    const now = this.clock();
    const paused = (await this.stores.state.read()).pausedUntil;
    if (!ctx.cfg.enabled) report.idle = 'Dispatcher ist ausgeschaltet';
    else if (paused && new Date(paused) > now) report.idle = `Pausiert bis ${this.hhmm(new Date(paused))}`;
    else if (!this.window.contains(now, ctx.cfg.window)) report.idle = `Außerhalb des Zeitfensters ${ctx.cfg.window!.from}–${ctx.cfg.window!.to}`;
    if (!report.idle) await this.startTickets(ctx, results, report);
    const blocked = (paused && new Date(paused) > now) || !this.window.contains(now, ctx.cfg.window);
    if (ctx.cfg.research.auto && !blocked) {
      const auto = await research.startAuto();
      report.research.push(...auto.started);
      report.errors.push(...auto.errors);
    }
    if (ctx.cfg.planning.auto && !blocked) {
      // Geplant wird nur, solange im WIP der Tickets noch Platz ist (Planungen zählen selbst nicht mit)
      const free = ctx.cfg.wip - (await this.stores.runs.list()).filter((r) => DispatchService.countsForWip(r)).length;
      const auto = await new PlanRunner(ctx).startAuto(free);
      report.planned.push(...auto.started);
      report.errors.push(...auto.errors);
    }
  }

  // Bereite Tickets der eigenen Boards bis zum WIP-Limit starten (Recherche-Läufe zählen nicht mit)
  private async startTickets(ctx: DispatchContext, results: ResultHandler, report: DispatchReport): Promise<void> {
    const all = await this.stores.runs.list();
    let free = ctx.cfg.wip - all.filter((r) => DispatchService.countsForWip(r)).length;
    // Ein Ticket, das gerade geplant wird, startet nicht (der Plan ändert sich noch)
    const planning = new Set(all.filter((r) => r.kind === 'plan').map((r) => r.ticketId));
    const starter = new TicketStarter(ctx, results);
    for (const board of await this.ownBoards(ctx)) {
      try {
        let work: RepoWork | undefined;
        for (const view of this.planned(await ctx.services.tickets.list({ boardIds: [board.id] }))) {
          if (planning.has(view.id) || ctx.readiness.reason(view, board, ctx.me.userId, existsSync)) continue;
          if (free <= 0) {
            report.queued.push(view.id);
            continue;
          }
          work ??= await ctx.repo(board);
          try {
            const outcome = await starter.start(view, work);
            if ('record' in outcome) {
              report.started.push(view.id);
              free -= 1;
            } else if ('error' in outcome) {
              report.errors.push(`${view.id}: ${outcome.error}`);
            }
          } catch (error) {
            if (BoardRuntimeHolder.isConnectionFailure(error)) throw error;
            report.errors.push(`${view.id}: ${(error as Error).message}`);
          }
        }
      } catch (error) {
        if (BoardRuntimeHolder.isConnectionFailure(error)) throw error;
        report.errors.push(`Board ${board.id}: ${(error as Error).message}`);
      }
    }
  }

  // Bereitschaft der Geplant-Tickets eines Boards für den Status
  private rows(cfg: DispatchConfig, boardId: number, free: number): Promise<{ tickets: DispatchTicketRow[]; boardInScope: boolean }> {
    return this.deps.boards.run(async (backend) => {
      const board = await new BoardConfigStore(this.deps.home).require();
      const me: RepoBoardOwner = { userId: board.userId, activeBoard: board.activeBoard };
      const ctx = new DispatchContext(cfg, me, backend, this.stores, this.deps.launcher, this.clock, () => undefined);
      const target = await ctx.board(boardId);
      if (!target) return { tickets: [], boardInScope: false };
      let left = free;
      let work: Promise<RepoWork | null> | undefined;
      const tickets: DispatchTicketRow[] = [];
      for (const view of this.planned(await ctx.services.tickets.list({ boardIds: [boardId], withoutTexts: true }))) {
        let reason = ctx.readiness.reason(view, target, me.userId, existsSync);
        if (!reason) {
          // Läuft im Worktree des Tickets schon ein Worker, wartet es, ohne einen freien Platz zu belegen
          work ??= ctx.repo(target).catch(() => null);
          const repo = await work;
          reason = repo ? await ctx.worktreeBusy(view, repo).catch(() => null) : null;
        }
        if (!reason) {
          reason = left > 0 ? 'bereit' : 'bereit – wartet auf freien Platz';
          left -= 1;
        }
        tickets.push({ id: view.id, title: view.title, reason });
      }
      return { tickets, boardInScope: ctx.scope().includes(boardId) };
    });
  }

  // Merge-Aufträge für den Status mit dem Zielbranch ihres Tickets (nicht MergeEntry.target, das erst die Merge-Session setzt);
  // ohne Aufträge keine DB-Abfrage, ist das Ticket oder die DB nicht lesbar, steht kein Ziel
  private async mergeInfos(entries: MergeEntry[]): Promise<DispatchMergeInfo[]> {
    const targets = new Map<string, string | null>();
    if (entries.length) {
      await this.deps.boards.run(async (backend) => {
        for (const e of entries) {
          const detail = await backend.services.tickets.get(e.ticketId).catch(() => null);
          targets.set(e.ticketId, detail?.targetBranch ?? null);
        }
      }).catch(() => undefined);
    }
    return entries.map((m) => ({ ticketId: m.ticketId, phase: m.phase, branch: m.branch, target: targets.get(m.ticketId) ?? null }));
  }

  // Aktives Board aus board.yaml (ohne oder mit kaputter Datei: keines)
  private async activeBoard(): Promise<number | undefined> {
    try {
      return (await new BoardConfigStore(this.deps.home).read())?.activeBoard ?? undefined;
    } catch {
      return undefined;
    }
  }

  // Geplant-Tickets in Board-Reihenfolge: Priorität, dann Nummer
  private planned(views: BoardTicketView[]): BoardTicketView[] {
    return views.filter((v) => v.status === 'planned').sort((a, b) => PRIORITY_RANK[a.priority] - PRIORITY_RANK[b.priority] || a.number - b.number);
  }

  // HH:MM Ortszeit
  private hhmm(d: Date): string {
    return `${String(d.getHours()).padStart(2, '0')}:${String(d.getMinutes()).padStart(2, '0')}`;
  }
}
