import {
  ContextBuilder,
  DEFAULT_SERVER_PORT,
  EnvFileWriter,
  HarnessHome,
  HarnessProject,
  HookEventFactory,
  QuestionContextMarks,
  SessionTitle,
  TicketAssigner,
  TokenStore,
  WorkflowSource,
  type HookEvent,
  type HookKind,
} from '@harness/core';
import { BoardRuntime } from '@harness/db';
import { ServerLauncher } from '../board/ServerLauncher.js';
import type { BoardAccess, BoardOpener } from './BoardAccess.js';
import { ServerClient } from './ServerClient.js';

const DEFAULT_SERVER_START_MS = 2500;
/** Zeit, die Zuordnung, Kontext und das Markieren der Kommentare aus der Board-DB zusammen höchstens brauchen dürfen */
const DEFAULT_BOARD_BUDGET_MS = 2000;
const OUTPUT_NAMES: Partial<Record<HookKind, string>> = { 'session-start': 'SessionStart', 'user-prompt': 'UserPromptSubmit' };

interface Launcher {
  probe(port: number): Promise<'ours' | 'free' | 'foreign'>;
  start(port: number, waitMs?: number): Promise<void>;
}

/** Was die Board-DB zum Hook beiträgt; `markRead` erst nach der Ausgabe aufrufen */
interface BoardPart {
  tickets: string[];
  text: string | null;
  markRead: () => Promise<void>;
}

const NO_BOARD: BoardPart = { tickets: [], text: null, markRead: async () => undefined };

// Öffnet die Board-Laufzeit aus board.yaml; fehlende Konfig oder nicht erreichbare DB ergeben null
const openRuntime: BoardOpener = async (env) => {
  try {
    return await BoardRuntime.open(new HarnessHome(env));
  } catch {
    return null;
  }
};

// Ablauf eines Hooks: zuordnen, Kontext einspeisen, Umgebung setzen, an den Server melden. Wirft nie.
// Zuordnung und Kontext kommen aus der Board-DB; ohne DB, ohne Board des Repos oder wenn die DB nicht innerhalb der
// Zeitvorgabe antwortet, bleiben sie leer. Umgebung, Session-Name und Meldung an den Server hängen nicht von der DB ab.
export class HookRunner {
  private readonly factory = new HookEventFactory();
  private readonly openBoard: BoardOpener;
  private readonly launcher: Launcher;
  private readonly boardBudgetMs: number;

  constructor(deps: { openBoard?: BoardOpener; launcher?: Launcher; boardBudgetMs?: number } = {}) {
    this.openBoard = deps.openBoard ?? openRuntime;
    this.launcher = deps.launcher ?? new ServerLauncher();
    this.boardBudgetMs = deps.boardBudgetMs ?? DEFAULT_BOARD_BUDGET_MS;
  }

  // Führt den Hook aus. Die Ausgabe für Claude Code (JSON-Text) geht über `emit`, bevor der Server gemeldet wird,
  // damit sie nicht von dessen Erreichbarkeit abhängt.
  async run(kind: HookKind, payload: unknown, env: Record<string, string | undefined>, emit: (output: string) => void): Promise<void> {
    try {
      await this.execute(kind, payload, env, emit);
    } catch (error) {
      if (env.HARNESS_DEBUG === '1') console.error(`Hook ${kind}: ${(error as Error).message}`);
    }
  }

  // Die eigentliche Arbeit; Fehler fängt `run`
  private async execute(kind: HookKind, payload: unknown, env: Record<string, string | undefined>, emit: (output: string) => void): Promise<void> {
    if (typeof payload !== 'object' || payload === null) return;
    const raw = payload as { cwd?: unknown; session_id?: unknown };
    const cwd = typeof raw.cwd === 'string' && raw.cwd ? raw.cwd : process.cwd();
    const sessionId = typeof raw.session_id === 'string' ? raw.session_id : '';
    if (!sessionId) return;

    const deadline = Date.now() + this.boardBudgetMs;
    const project = await HarnessProject.open(cwd); // ohne `harness init` Fehler → still beenden
    const board = await this.open(env, deadline);
    let event: HookEvent | null = null;
    try {
      const part = board ? await this.fromBoard(board, deadline, kind, project, cwd, sessionId, env) : NO_BOARD;
      event = this.factory.create(kind, payload, cwd, part.tickets);
      if (!event) return;
      const sessionTitle = await this.nameSession(kind, project, sessionId, event, payload);
      if (kind === 'session-start') {
        const vars: Record<string, string> = { HARNESS_SESSION: sessionId };
        if (part.tickets[0]) vars.HARNESS_TICKET = part.tickets[0];
        await new EnvFileWriter().append(env.CLAUDE_ENV_FILE, vars, env);
      }
      const name = OUTPUT_NAMES[kind];
      if (name && (part.text || sessionTitle)) {
        const output: Record<string, string> = { hookEventName: name };
        if (part.text) output.additionalContext = part.text;
        if (sessionTitle) output.sessionTitle = sessionTitle;
        emit(JSON.stringify({ hookSpecificOutput: output }));
      }
      // Erst nach der Ausgabe als gelesen markieren; scheitert das oder läuft die Zeit ab, kommen die Kommentare beim nächsten Mal
      await this.markRead(kind, part, deadline, env);
    } finally {
      if (board) await this.within(this.close(board), deadline);
    }
    await this.notify(kind, project, event, env);
  }

  // Öffnet die Board-DB innerhalb der Zeitvorgabe; eine erst später geöffnete Laufzeit wird gleich wieder geschlossen
  private async open(env: Record<string, string | undefined>, deadline: number): Promise<BoardAccess | null> {
    const opening = this.openBoard(env).catch(() => null);
    const opened = await this.within(opening, deadline);
    if (opened) return opened.value;
    if (env.HARNESS_DEBUG === '1') console.error('Hook: Board-DB antwortet nicht rechtzeitig');
    void opening.then((late) => (late ? this.close(late) : undefined));
    return null;
  }

  // Wartet auf `work` höchstens bis `deadline`; null, wenn die Zeit vorher abläuft. Späte Fehler bleiben ohne Folgen.
  private async within<T>(work: Promise<T>, deadline: number): Promise<{ value: T } | null> {
    work.catch(() => undefined);
    const remaining = deadline - Date.now();
    if (remaining <= 0) return null;
    let timer: NodeJS.Timeout | undefined;
    const timeout = new Promise<null>((done) => {
      timer = setTimeout(() => done(null), remaining);
    });
    try {
      return await Promise.race([work.then((value) => ({ value })), timeout]);
    } finally {
      clearTimeout(timer);
    }
  }

  // Tickets der Session und Kontext aus der Board-DB innerhalb der Zeitvorgabe.
  // Fehler oder Zeitablauf bei der Zuordnung ergeben „nichts zugeordnet, kein Kontext“; beim Kontext bleibt die Zuordnung erhalten.
  private async fromBoard(
    board: BoardAccess, deadline: number, kind: HookKind, project: HarnessProject, cwd: string, sessionId: string,
    env: Record<string, string | undefined>,
  ): Promise<BoardPart> {
    const debug = (message: string) => {
      if (env.HARNESS_DEBUG === '1') console.error(`Hook ${kind}: Board-DB: ${message}`);
    };
    let tickets: string[];
    try {
      const assign = async () => {
        const repoBoard = await board.services.boards.forRepo(project.root, board.config);
        if (!repoBoard) return null;
        return { tickets: await new TicketAssigner(board.services).resolve({ cwd, sessionId, env, boardId: repoBoard.id }) };
      };
      const assigned = await this.within(assign(), deadline);
      if (!assigned) debug('Zuordnung nicht rechtzeitig');
      if (!assigned?.value) return NO_BOARD;
      tickets = assigned.value.tickets;
    } catch (error) {
      debug((error as Error).message);
      return NO_BOARD;
    }
    if (kind !== 'session-start' && kind !== 'user-prompt') return { ...NO_BOARD, tickets };
    // Angewendeter Workflow (Anpassungen), sonst der Standard
    const phases = new WorkflowSource(new HarnessHome(env)).read().phases;
    const builder = new ContextBuilder(board.services, new QuestionContextMarks(project.layout), undefined, phases);
    try {
      const context = await this.within(builder.build(kind, tickets, sessionId), deadline);
      if (!context) {
        debug('Kontext nicht rechtzeitig');
        return { ...NO_BOARD, tickets };
      }
      const { text, commentIds } = context.value;
      return { tickets, text, markRead: () => builder.markRead(commentIds) };
    } catch (error) {
      debug((error as Error).message);
      return { ...NO_BOARD, tickets };
    }
  }

  // Markiert die eingespeisten Kommentare innerhalb der Zeitvorgabe als gelesen; Fehler bleiben ohne Folgen
  private async markRead(kind: HookKind, part: BoardPart, deadline: number, env: Record<string, string | undefined>): Promise<void> {
    try {
      if (!(await this.within(part.markRead(), deadline))) throw new Error('nicht rechtzeitig');
    } catch (error) {
      if (env.HARNESS_DEBUG === '1') console.error(`Hook ${kind}: Kommentare nicht als gelesen markiert: ${(error as Error).message}`);
    }
  }

  // Schließt die Laufzeit; Fehler beim Schließen sind ohne Folgen
  private async close(board: BoardAccess): Promise<void> {
    try {
      await board.close();
    } catch {
      // Verbindung war ohnehin weg
    }
  }

  // Nummeriert die Session („SE-001: Titel“). Liefert den Namen, der an Claude Code zurückgeht, sonst undefined.
  // Den automatisch erzeugten Titel von Claude Code sehen Hooks nicht: Beim Start wird nur ein vorhandener eigener
  // Titel ergänzt, beim ersten Prompt wird der Titel aus dem Prompt abgeleitet. Die Oberfläche zeigt immer mindestens „SE-001“.
  private async nameSession(kind: HookKind, project: HarnessProject, sessionId: string, event: HookEvent, payload: unknown): Promise<string | undefined> {
    if (kind !== 'session-start' && kind !== 'user-prompt') return undefined;
    const number = await project.sessionNumbers.assign(sessionId);
    const title = new SessionTitle();
    const existing = event.title;
    let name: string | undefined;
    if (existing) {
      name = title.format(number, existing);
    } else if (kind === 'user-prompt') {
      const prompt = (payload as { prompt?: unknown }).prompt;
      const derived = title.fromPrompt(typeof prompt === 'string' ? prompt : undefined);
      if (derived) name = title.format(number, derived);
    }
    event.title = name ?? title.format(number);
    return name !== existing ? name : undefined; // unverändert → Claude Code nicht erneut umbenennen
  }

  // Meldet das Ereignis an den Server; nur beim Sessionbeginn wird er bei Bedarf gestartet
  private async notify(kind: HookKind, project: HarnessProject, event: HookEvent | null, env: Record<string, string | undefined>): Promise<void> {
    if (!event) return;
    const port = Number(env.HARNESS_PORT) || DEFAULT_SERVER_PORT;
    const state = await this.launcher.probe(port);
    if (state === 'foreign') return;
    if (state === 'free') {
      if (kind !== 'session-start') return;
      await this.launcher.start(port, Number(env.HARNESS_HOOK_SERVER_WAIT_MS) || DEFAULT_SERVER_START_MS);
    }
    const client = new ServerClient(port);
    await client.registerProject(project.root);
    await client.sendEvent(project.root, await new TokenStore(project.layout).read(), event);
  }
}
