import { existsSync } from 'node:fs';
import { BoardConfigStore, BranchNames, HarnessError, TargetBranches, TargetBranchRule, type BoardConfig, type BoardSummary, type HarnessHome } from '@harness/core';
import type { FastifyInstance } from 'fastify';
import { BoardRuntimeHolder } from '../dbboard/BoardRuntimeHolder.js';
import { DbInput } from '../dbboard/DbInput.js';
import type { FolderPicker } from '../dbboard/FolderPicker.js';

export interface DbActiveDto {
  user: { id: string; name: string };
  boardId: number | null;
  board: BoardSummary | null;
}

// /api/db: Projekte, Boards, aktives Board (board.yaml, gemeinsam mit CLI und Agenten) und Board-Ansicht.
export class DbBoardRoutes {
  private readonly input = new DbInput();
  private readonly store: BoardConfigStore;

  constructor(private readonly holder: BoardRuntimeHolder, home: HarnessHome, private readonly picker: FolderPicker) {
    this.store = new BoardConfigStore(home);
  }

  // Meldet die Routen an
  register(app: FastifyInstance): void {
    app.get('/api/db/projects', async () => this.holder.run((b) => b.services.projects.list()));
    app.post('/api/db/projects', async (request, reply) => {
      const body = this.input.body(request.body);
      const key = this.input.text(body, 'key');
      const name = this.input.text(body, 'name');
      return reply.code(201).send(await this.holder.run((b) => b.services.projects.create(key, name, b.actor())));
    });
    app.patch('/api/db/projects/:key', async (request) => {
      const { key } = request.params as { key: string };
      const name = this.input.text(this.input.body(request.body), 'name');
      return this.holder.run((b) => b.services.projects.rename(key, name));
    });

    app.get('/api/db/boards', async (request) => {
      const { project } = request.query as { project?: string };
      return this.holder.run((b) => b.services.boards.list(project ? { projectKey: project } : {}));
    });
    app.post('/api/db/boards', async (request, reply) => {
      const body = this.input.body(request.body);
      const project = this.input.text(body, 'project');
      const name = this.input.text(body, 'name');
      const summary = await this.holder.run(async (b) => {
        const board = await b.services.boards.create(project, name, b.actor());
        return this.summary(await b.services.boards.list(), board.id);
      });
      return reply.code(201).send(summary);
    });
    app.patch('/api/db/boards/:id', async (request) => {
      const id = this.input.id((request.params as { id: string }).id, 'Board');
      const body = this.input.body(request.body);
      const name = this.input.optionalText(body, 'name');
      const repoPath = this.input.nullableText(body, 'repoPath');
      if (name === undefined && repoPath === undefined) throw new HarnessError('nothing-changed', 'Keine Änderung angegeben.');
      return this.holder.run(async (b) => {
        if (name !== undefined) await b.services.boards.rename(id, name);
        if (repoPath !== undefined) await b.services.boards.setRepoPath(id, repoPath);
        return this.summary(await b.services.boards.list(), id);
      });
    });
    app.delete('/api/db/boards/:id', async (request) => {
      const id = this.input.id((request.params as { id: string }).id, 'Board');
      const { deleteOpen } = request.query as { deleteOpen?: string };
      // board.yaml vorher prüfen: ein Konfigurationsfehler darf nicht erst nach dem Löschen als 503 kommen
      const config = await this.config();
      const result = await this.holder.run((b) => b.services.boards.delete(id, { deleteOpen: deleteOpen === 'true' }, b.actor()));
      if (config.activeBoard === id) await this.store.write({ ...config, activeBoard: null });
      return result;
    });
    // Ordnerdialog auf dem Rechner des Servers für „Mein Repo-Pfad“; path ist null, wenn die Person abbricht
    app.post('/api/db/pick-folder', async (request) => {
      const start = this.input.optionalText(this.input.body(request.body), 'start');
      return { path: await this.picker.pick(start?.trim() || null) };
    });
    app.post('/api/db/boards/:id/activate', async (request) => {
      const id = this.input.id((request.params as { id: string }).id, 'Board');
      const config = await this.config();
      await this.holder.run(async (b) => this.summary(await b.services.boards.list(), id));
      await this.store.write({ ...config, activeBoard: id });
      return this.active();
    });
    app.get('/api/db/active', async () => this.active());
    app.get('/api/db/boards/:id/view', async (request) => {
      const id = this.input.id((request.params as { id: string }).id, 'Board');
      return this.holder.run((b) => b.services.views.get(id));
    });
    // Vorschläge für den Zielbranch; ohne Repo auf diesem Rechner kein Fehler, sondern „no-repo“ (das Feld bleibt dann reine Texteingabe)
    app.get('/api/db/boards/:id/branches', async (request) => {
      const id = this.input.id((request.params as { id: string }).id, 'Board');
      const board = await this.holder.run((b) => b.services.boards.find(id));
      if (!board) throw new HarnessError('board-not-found', `Board ${id} gibt es nicht.`);
      if (!board.repoPath || !existsSync(board.repoPath)) return { branches: [], reason: 'no-repo' };
      return { branches: new BranchNames().list(board.repoPath), reason: null };
    });
    // Legt einen neuen Zielbranch im Board-Repo lokal an (von origin/<name>, sonst vom HEAD des Hauptordners); kein Push
    app.post('/api/db/boards/:id/branches', async (request, reply) => {
      const id = this.input.id((request.params as { id: string }).id, 'Board');
      const name = new TargetBranchRule().normalize(this.input.text(this.input.body(request.body), 'name'));
      if (name === null) throw new HarnessError('target-branch-invalid', 'Kein Branch-Name angegeben.');
      const board = await this.holder.run((b) => b.services.boards.find(id));
      if (!board) throw new HarnessError('board-not-found', `Board ${id} gibt es nicht.`);
      if (!board.repoPath || !existsSync(board.repoPath)) throw new HarnessError('board-repo-missing', `Board ${id} hat auf diesem Rechner kein Repo.`);
      const ensured = new TargetBranches().ensure(board.repoPath, name);
      return reply.code(ensured.created ? 201 : 200).send({ name, ...ensured });
    });
  }

  // Benutzer aus board.yaml und das aktive Board (ein gelöschtes aktives Board gilt als keins)
  private async active(): Promise<DbActiveDto> {
    const config = await this.config();
    const boards = await this.holder.run((b) => b.services.boards.list());
    const board = boards.find((x) => x.id === config.activeBoard) ?? null;
    return { user: { id: config.userId, name: config.userName }, boardId: board?.id ?? null, board };
  }

  // board.yaml; fehlend → board-config-missing, kaputt → board-config-broken (beides 503)
  private async config(): Promise<BoardConfig> {
    try {
      return await this.store.require();
    } catch (error) {
      throw BoardRuntimeHolder.translate(error);
    }
  }

  // Board mit Projekt, Besitzer und Zahl offener Tickets oder board-not-found
  private summary(list: BoardSummary[], id: number): BoardSummary {
    const found = list.find((b) => b.id === id);
    if (!found) throw new HarnessError('board-not-found', `Board ${id} gibt es nicht.`);
    return found;
  }
}
