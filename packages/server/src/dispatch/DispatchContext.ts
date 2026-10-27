import {
  AttemptCounter, ClaudeInvocation, DispatchPrompt, DispatchReadiness, GitBashLocator, HarnessError, HarnessProject, ProfileChooser, RunOutputReader, WorktreeManager,
  type Board, type BoardActor, type BoardServices, type DispatchConfig, type RepoBoardOwner, type RunSettings,
  type DispatchLog, type DispatchStateStore, type ReviewStore, type RunKind, type RunRecord, type RunStore, type WorktreeSetupStep,
} from '@harness/core';
import type { BoardBackend } from '../dbboard/BoardBackend.js';
import type { WorkerLauncher } from './WorkerLauncher.js';

// Windows-Meldung des Dispatchers
export interface DispatchNotice {
  kind: 'merged' | 'blocked' | 'question' | 'paused' | 'review';
  ticketId: string | null;
  title: string;
  text: string;
  /** Repo-Pfad des Boards (für die Adresse in der Meldung) */
  repoPath: string | null;
}

export interface DispatchStores {
  runs: RunStore;
  state: DispatchStateStore;
  log: DispatchLog;
  reviews: ReviewStore;
}

// Board mit Repo: Hauptordner und Worktree-Verwaltung
export interface RepoWork {
  board: Board;
  root: string;
  manager: WorktreeManager;
}

export interface LaunchInput {
  kind: RunKind;
  ticketId: string;
  boardId: number;
  title: string;
  agent: string;
  cwd: string;
  branch: string;
  target: string | null;
  run: RunSettings;
  prompt: string;
  /** Zusätzlich erlaubte Tools (Merge-Session); eine Recherche (kind research) bekommt immer nur RESEARCH_TOOLS */
  extraTools?: string[];
  /** Recherche: Frage, Zahl ihrer Recherchen beim Start, Hauptordner des Repos */
  questionId?: string;
  researchBefore?: number;
  repoRoot?: string;
  /** Planung: plannedAt des Tickets beim Start */
  plannedBefore?: string | null;
  /** Umsetzung in einem neu angelegten Worktree: Einrichtungsbefehle vor Claude */
  setup?: WorktreeSetupStep[];
  /** Umsetzung: Nummer des Versuchs */
  attempt?: number;
}

// Alles, was ein Tick braucht: Einstellung, eigener Benutzer, Board-Dienste, lokale Dateien, Starter der Worker.
export class DispatchContext {
  readonly actor: BoardActor;
  readonly chooser: ProfileChooser;
  readonly readiness: DispatchReadiness;
  readonly attempts: AttemptCounter;
  readonly prompts = new DispatchPrompt();
  readonly output = new RunOutputReader();
  private readonly invocation = new ClaudeInvocation();
  private readonly repos = new Map<number, Promise<RepoWork>>();

  constructor(
    readonly cfg: DispatchConfig,
    readonly me: RepoBoardOwner,
    readonly backend: BoardBackend,
    readonly stores: DispatchStores,
    private readonly launcher: WorkerLauncher,
    private readonly clock: () => Date,
    private readonly notifier: (notice: DispatchNotice) => Promise<void> | void,
  ) {
    this.actor = backend.actor(cfg.dispatcherName);
    this.chooser = new ProfileChooser(cfg);
    this.readiness = new DispatchReadiness(cfg);
    this.attempts = new AttemptCounter(cfg);
  }

  // Dienste des Boards
  get services(): BoardServices {
    return this.backend.services;
  }

  // Aktuelle Zeit des Dispatchers
  now(): Date {
    return this.clock();
  }

  // Name des Junior-Agents eines Tickets
  agentName(ticketId: string): string {
    return `${this.cfg.agentPrefix}${ticketId}`;
  }

  // Gehört die Zuweisung einem Agenten des Dispatchers?
  isAgent(assignee: string | null): boolean {
    return Boolean(assignee?.startsWith(this.cfg.agentPrefix));
  }

  // Kommentar des Dispatchers
  async comment(ticketId: string, body: string): Promise<void> {
    await this.services.comments.add(ticketId, body, this.actor);
  }

  // Windows-Meldung; Fehler halten den Tick nicht auf
  async notify(notice: DispatchNotice): Promise<void> {
    try {
      await this.notifier(notice);
    } catch {
      /* Meldungen sind optional */
    }
  }

  // Zeile in dispatch.log
  log(text: string): void {
    this.stores.log.line(text);
  }

  // Board zur ID mit dem eigenen (wirksamen) Repo-Pfad oder null
  board(id: number): Promise<Board | null> {
    return this.services.boards.find(id);
  }

  // Boards, die der Dispatcher bedient: aus der Einstellung, sonst das aktive
  scope(): number[] {
    return this.cfg.boards ?? (this.me.activeBoard ? [this.me.activeBoard] : []);
  }

  // Hauptordner und Worktree-Verwaltung des Board-Repos (je Tick einmal eingerichtet)
  repo(board: Board): Promise<RepoWork> {
    if (!board.repoPath) return Promise.reject(new HarnessError('board-repo-missing', `Board ${board.id} hat keinen Repo-Pfad.`));
    let work = this.repos.get(board.id);
    if (!work) {
      work = HarnessProject.init(board.repoPath).then((project) => ({
        board, root: project.root, manager: new WorktreeManager(project.layout, this.services, project.root, { joinWorktrees: this.cfg.joinWorktrees }),
      }));
      this.repos.set(board.id, work);
    }
    return work;
  }

  // Nie zwei Worker im selben Worktree: Grund zum Warten, wenn auf dem Branch, den das Ticket nutzen würde (Fortsetzung:
  // sein Branch, außer er gehört nicht mehr zum Ticket; Neustart: nur gebündelt der Branch, dem es automatisch beitreten würde –
  // sonst bekommt es einen eigenen neuen), gerade ein Lauf beliebiger Art aktiv ist
  async worktreeBusy(ticket: { id: string; branch: string | null }, work: RepoWork): Promise<string | null> {
    const own = ticket.branch && !(await work.manager.staleReason(ticket)) ? ticket.branch : null;
    const join = !ticket.branch && this.cfg.joinWorktrees === 'sameSpec';
    const branch = own ?? (join ? await work.manager.joinCandidate(ticket.id, { joinOnlySameSpec: true }) : undefined);
    if (!branch) return null;
    const run = (await this.stores.runs.list()).find((r) => r.branch === branch);
    return run ? `wartet – Worktree belegt durch ${run.ticketId}` : null;
  }

  // Legt den Lauf an und startet den Worker; scheitert der Start, bleibt kein Lauf zurück. Läuft der Worker,
  // bleibt der Lauf auch dann, wenn die PID nicht gespeichert werden kann (ohne PID gilt er nach 2 min als verschwunden).
  async launch(input: LaunchInput): Promise<RunRecord> {
    const at = this.now();
    const id = this.stores.log.runName(input.ticketId, input.kind, at);
    const { extraTools, ...rest } = input;
    // Unter Windows braucht der Worker die Git Bash (sonst gelten die Bash-Regeln nicht); ohne sie startet nichts
    const windows = process.platform === 'win32';
    const gitBash = windows ? this.cfg.claude.gitBashPath ?? new GitBashLocator().locate(process.env) : null;
    if (windows && !gitBash) {
      throw new HarnessError('git-bash-missing', 'Git Bash nicht gefunden – Git for Windows installieren oder claude.gitBashPath in den Dispatcher-Einstellungen setzen.');
    }
    const record: RunRecord = {
      id, ...rest, command: this.cfg.claude.command, args: this.invocation.args(this.cfg, input.run, extraTools ?? [], input.kind),
      logFile: this.stores.log.filePath(`${id}.log`), startedAt: at.toISOString(), pid: null,
      ...(input.setup?.length ? { setupTimeoutMinutes: this.cfg.worktreeSetupTimeoutMinutes } : {}),
    };
    await this.stores.runs.create(record);
    let pid: number | null;
    try {
      pid = this.launcher.launch(id, this.invocation.env(process.env, this.cfg, input.kind === 'research' ? null : input.ticketId, gitBash) as NodeJS.ProcessEnv);
    } catch (error) {
      await this.stores.runs.remove(id);
      throw error;
    }
    try {
      try {
        await this.stores.runs.setPid(id, pid);
      } catch {
        await this.stores.runs.setPid(id, pid); // ein zweiter Versuch, bevor der Lauf ohne PID bleibt
      }
    } catch (error) {
      this.log(`FEHLER ${input.ticketId}: Prozess-ID ${pid} nicht gespeichert: ${(error as Error).message}`);
      return record;
    }
    return { ...record, pid };
  }
}
