import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import type { FastifyInstance } from 'fastify';
import { BoardConfigStore, HarnessHome, HarnessProject, ProjectRegistry } from '@harness/core';
import type { BoardBackendFactory } from '../../src/dbboard/BoardBackend.js';
import type { FolderPicker } from '../../src/dbboard/FolderPicker.js';
import type { ServerConfig } from '../../src/config/ServerConfig.js';
import type { WorkerLauncher } from '../../src/dispatch/WorkerLauncher.js';
import { MEMORY_CONFIG, MemoryBoardBackend } from './MemoryBoardBackend.js';
import { ServerApp } from '../../src/ServerApp.js';
import { TempRepo } from '../../../core/tests/unit/helpers/TempRepo.js';

export const PORT = 4710;

export interface Reply {
  status: number;
  json: any;
  text: string;
  headers: Record<string, unknown>;
}

// Testumgebung: Server ohne Port (inject), eigenes HARNESS_HOME, beliebig viele Testprojekte.
export class TestServer {
  readonly repos: TempRepo[] = [];

  private constructor(
    readonly app: FastifyInstance,
    readonly home: HarnessHome,
    private readonly homeDir: string,
  ) {}

  // Startet den Server mit frischem HARNESS_HOME; `webDir` für Tests der statischen Auslieferung,
  // `boardBackend` ersetzt die Board-Laufzeit (sonst BoardRuntime aus board.yaml)
  static async start(options: { webDir?: string; heartbeatMs?: number; boardBackend?: BoardBackendFactory; dbPollMs?: number; dispatchLauncher?: WorkerLauncher; folderPicker?: FolderPicker; fileOpener?: ServerConfig['fileOpener']; claudeUsage?: ServerConfig['claudeUsage']; suggestionClaude?: ServerConfig['suggestionClaude']; ticketDraftClaude?: ServerConfig['ticketDraftClaude']; reviseClaude?: ServerConfig['reviseClaude'] } = {}): Promise<TestServer> {
    const homeDir = mkdtempSync(join(tmpdir(), 'harness-srv-'));
    const home = new HarnessHome({ HARNESS_HOME: homeDir });
    const app = await ServerApp.create({
      port: PORT, host: '127.0.0.1', home, webDir: options.webDir ?? null, version: 'test',
      heartbeatMs: options.heartbeatMs, boardBackend: options.boardBackend, dbPollMs: options.dbPollMs,
      dispatchTickMs: 0, dispatchLauncher: options.dispatchLauncher ?? { launch: () => null },
      // nie einen echten Ordnerdialog öffnen
      folderPicker: options.folderPicker ?? { pick: async () => null },
      // nie Explorer oder VS Code starten
      fileOpener: options.fileOpener ?? { open: async () => undefined },
      // nie die echte Anmeldung lesen oder Anthropic fragen
      claudeUsage: options.claudeUsage ?? { current: async () => ({ available: false, reason: 'no-login', plan: null, fiveHour: null, sevenDay: null, fetchedAt: null }) },
      // nie das echte Claude für die Optimierungs-Analyse starten
      suggestionClaude: options.suggestionClaude ?? {
        run: async () => ({ code: 0, timedOut: false, output: JSON.stringify({ type: 'result', subtype: 'success', is_error: false, result: '[]', total_cost_usd: 0 }) }),
      },
      // nie das echte Claude für die Ticket-Entwürfe starten
      ticketDraftClaude: options.ticketDraftClaude ?? {
        run: async () => ({ code: 0, timedOut: false, output: JSON.stringify({ type: 'result', subtype: 'success', is_error: false, result: '[]', total_cost_usd: 0 }) }),
      },
      // nie das echte Claude für das Umschreiben eines Dokument-Bereichs starten
      reviseClaude: options.reviseClaude ?? {
        run: async () => ({ code: 0, timedOut: false, output: JSON.stringify({ type: 'result', subtype: 'success', is_error: false, result: 'Ersatztext', total_cost_usd: 0 }) }),
      },
    });
    return new TestServer(app, home, homeDir);
  }

  // Startet den Server mit Board-Laufzeit im Speicher und einer Platzhalter-board.yaml (Benutzer, aktives Board)
  static async startWithBoard(options: Parameters<typeof TestServer.start>[0] = {}): Promise<{ server: TestServer; backend: MemoryBoardBackend }> {
    const backend = await MemoryBoardBackend.create();
    const server = await TestServer.start({ ...options, boardBackend: async () => backend });
    await new BoardConfigStore(server.home).write(MEMORY_CONFIG);
    return { server, backend };
  }

  // Legt ein eingerichtetes Testprojekt an, meldet es an und liefert Projekt, ID und Ordner
  async addProject(): Promise<{ repo: TempRepo; project: HarnessProject; id: string }> {
    const repo = new TempRepo();
    this.repos.push(repo);
    const project = await HarnessProject.init(repo.path);
    const { project: entry } = await new ProjectRegistry(this.home).add(repo.path);
    return { repo, project, id: entry.id };
  }

  // Anfrage mit den Standardköpfen eines erlaubten Browsers
  async request(method: string, url: string, body?: unknown, headers: Record<string, string> = {}): Promise<Reply> {
    const base: Record<string, string> = { host: `127.0.0.1:${PORT}` };
    if (method !== 'GET' && method !== 'HEAD') {
      base['content-type'] = 'application/json';
      base['x-harness'] = '1';
    }
    const res = await this.app.inject({
      method: method as 'GET',
      url,
      headers: { ...base, ...headers },
      payload: body === undefined ? undefined : JSON.stringify(body),
    });
    let json: unknown = null;
    try {
      json = JSON.parse(res.body);
    } catch {
      /* kein JSON */
    }
    return { status: res.statusCode, json, text: res.body, headers: res.headers };
  }

  get = (url: string, headers?: Record<string, string>) => this.request('GET', url, undefined, headers);
  post = (url: string, body?: unknown, headers?: Record<string, string>) => this.request('POST', url, body ?? {}, headers);
  patch = (url: string, body: unknown) => this.request('PATCH', url, body);
  put = (url: string, body?: unknown) => this.request('PUT', url, body ?? {});
  delete = (url: string) => this.request('DELETE', url, {});

  // Räumt alles auf
  async dispose(): Promise<void> {
    await this.app.close();
    this.repos.forEach((r) => r.dispose());
    rmSync(this.homeDir, { recursive: true, force: true });
  }
}
