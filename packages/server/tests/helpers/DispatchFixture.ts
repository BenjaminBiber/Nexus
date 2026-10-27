import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import {
  BoardConfigStore, DispatchConfigNormalizer, DispatchConfigStore, HarnessHome, HarnessProject, WorktreeManager,
  type BoardActor, type BoardTicketPatch, type RawDispatchConfig, type RunRecord,
} from '@harness/core';
import { BoardRuntimeHolder } from '../../src/dbboard/BoardRuntimeHolder.js';
import type { DispatchNotice } from '../../src/dispatch/DispatchContext.js';
import { DispatchService } from '../../src/dispatch/DispatchService.js';
import { TempRepo } from '../../../core/tests/unit/helpers/TempRepo.js';
import { FakeLauncher } from './FakeLauncher.js';
import { MEMORY_CONFIG, MEMORY_USER, MemoryBoardBackend } from './MemoryBoardBackend.js';

export const OK_OUTPUT = JSON.stringify({ type: 'result', subtype: 'success', is_error: false, result: 'fertig' });

// Testumgebung des Dispatchers: eigenes HARNESS_HOME, Git-Repo als Board-Repo, Board im Speicher, Attrappe statt Worker.
export class DispatchFixture {
  now = new Date();
  alive = true;
  readonly notices: DispatchNotice[] = [];
  readonly launcher = new FakeLauncher();
  service!: DispatchService;

  private constructor(
    readonly dir: string,
    readonly home: HarnessHome,
    readonly repo: TempRepo,
    readonly backend: MemoryBoardBackend,
    readonly boardId: number,
  ) {}

  // Legt alles an; die Einstellung ist eingeschaltet, Profile wie im Beispiel, ohne Freigabe vor dem Merge, sonst `raw`
  static async create(raw: RawDispatchConfig = {}): Promise<DispatchFixture> {
    const dir = mkdtempSync(join(tmpdir(), 'harness-dispatch-'));
    const home = new HarnessHome({ HARNESS_HOME: dir });
    const repo = new TempRepo();
    repo.git('config', 'user.name', 't');
    repo.git('config', 'user.email', 't@t');
    const backend = await MemoryBoardBackend.create();
    const actor = backend.actor();
    await backend.services.projects.create('ARC', 'Arches', actor);
    const board = await backend.services.boards.create('ARC', 'Sprint', actor);
    await backend.services.boards.setRepoPath(board.id, repo.path);
    await new BoardConfigStore(home).write({ ...MEMORY_CONFIG, activeBoard: board.id });
    await new DispatchConfigStore(home).write({ ...new DispatchConfigNormalizer().example(), enabled: true, reviewBeforeMerge: false, ...raw });
    const f = new DispatchFixture(dir, home, repo, backend, board.id);
    f.service = new DispatchService({
      home, boards: new BoardRuntimeHolder(async () => backend), launcher: f.launcher,
      notify: (n) => { f.notices.push(n); }, clock: () => f.now, isAlive: () => f.alive,
    });
    return f;
  }

  get services() {
    return this.backend.services;
  }

  // Handelnder als Junior-Agent des Tickets
  agent(ticketId: string): BoardActor {
    return { userId: MEMORY_USER.id, name: `junior-${ticketId}` };
  }

  // Legt ein bereites Ticket an (Geplant, Label auto, Plan, Handbuch, Abnahme) und liefert die ID
  async ready(title: string, patch: BoardTicketPatch = {}): Promise<string> {
    const actor = this.backend.actor();
    const created = await this.services.tickets.create(this.boardId, { title, acceptance: 'Tests grün', labels: ['auto'], needsPlanning: false }, actor);
    await this.services.tickets.update(created.id, { plan: '## Plan\nUmsetzen', handbook: 'npm test', status: 'planned', ...patch }, actor);
    return created.id;
  }

  // Jüngster Lauf zum Ticket
  async run(ticketId: string): Promise<RunRecord> {
    const runs = (await this.service.stores.runs.list()).filter((r) => r.ticketId === ticketId);
    if (!runs.length) throw new Error(`kein Lauf für ${ticketId}`);
    return runs[runs.length - 1];
  }

  // Schreibt das Ergebnis des jüngsten Laufs, als hätte der Worker es geschrieben
  async end(ticketId: string, output = OK_OUTPUT, code: number | null = 0): Promise<RunRecord> {
    const record = await this.run(ticketId);
    await this.service.stores.runs.finish(record.id, { code, timedOut: false, output, endedAt: new Date().toISOString() });
    return record;
  }

  // WorktreeManager des Board-Repos (wie die CLI: mit joinWorktrees aus der Einstellung des Dispatchers)
  async manager(): Promise<WorktreeManager> {
    const project = await HarnessProject.init(this.repo.path);
    const { joinWorktrees } = (await new DispatchConfigStore(this.home).read()).config;
    return new WorktreeManager(project.layout, this.services, project.root, { joinWorktrees });
  }

  // Agent committet eine Datei im Worktree und meldet „erledigt“ wie `harness board done`
  async finishAsAgent(ticketId: string, file = 'feature.txt', content = 'neu'): Promise<void> {
    const ticket = await this.services.tickets.get(ticketId);
    writeFileSync(join(ticket.worktree!, file), content);
    this.repo.git('-C', ticket.worktree!, 'add', file);
    this.repo.git('-C', ticket.worktree!, 'commit', '-q', '-m', `${ticketId}: ${file}`);
    await (await this.manager()).finish(ticketId, {}, ticket.worktree!, this.agent(ticketId));
  }

  // Texte aller Kommentare des Tickets
  async comments(ticketId: string): Promise<string[]> {
    return (await this.services.tickets.get(ticketId)).comments.map((c) => c.body);
  }

  // Stellt die Uhr des Dispatchers vor
  advance(minutes: number): void {
    this.now = new Date(this.now.getTime() + minutes * 60_000);
  }

  // Räumt auf
  dispose(): void {
    this.repo.dispose();
    rmSync(this.dir, { recursive: true, force: true });
  }
}
