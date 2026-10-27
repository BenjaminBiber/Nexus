import { HarnessError, type DispatchRunKind, type DispatchStats } from '@harness/core';
import type { FastifyInstance } from 'fastify';
import type { BoardRuntimeHolder } from '../dbboard/BoardRuntimeHolder.js';
import { DbInput } from '../dbboard/DbInput.js';

/** Standard-Zeitraum: die letzten 8 Wochen (56 Tage inkl. heute) */
const DEFAULT_DAYS = 56;
const MAX_DAYS = 366;
const KINDS: DispatchRunKind[] = ['work', 'merge', 'research'];

/** Antwort von GET /api/db/stats/dispatch */
export interface DispatchStatsDto extends DispatchStats {
  filter: { project: string; boardId: number | null; from: string; to: string; kind: DispatchRunKind };
}

interface StatsQuery {
  project?: string;
  board?: string;
  from?: string;
  to?: string;
  kind?: string;
}

// /api/db/stats/dispatch: Auswertung der Dispatcher-Läufe eines Projekts (Board, Zeitraum in Ortszeit-Tagen, Lauf-Art).
export class DbStatsRoutes {
  private readonly input = new DbInput();

  constructor(private readonly holder: BoardRuntimeHolder, private readonly clock: () => Date = () => new Date()) {}

  // Meldet die Route an
  register(app: FastifyInstance): void {
    app.get('/api/db/stats/dispatch', async (request): Promise<DispatchStatsDto> => {
      const q = request.query as StatsQuery;
      const project = q.project?.trim();
      if (!project) throw new HarnessError('input-invalid', 'Parameter „project“ (Projekt-Key) fehlt.');
      const boardId = q.board ? this.input.id(q.board, 'Board') : undefined;
      const today = this.clock();
      const to = q.to ? this.day(q.to, 'to') : new Date(today.getFullYear(), today.getMonth(), today.getDate());
      // from=config: ab dem Tag der neuesten Fassung der Dispatcher-Einstellung (ohne Fassung: Standard-Zeitraum)
      const changed = q.from?.trim() === 'config' ? await this.configChangedAt() : null;
      const fallback = new Date(to.getFullYear(), to.getMonth(), to.getDate() - (DEFAULT_DAYS - 1));
      const earliest = new Date(to.getFullYear(), to.getMonth(), to.getDate() - (MAX_DAYS - 1));
      const from = changed ? new Date(Math.max(new Date(changed.getFullYear(), changed.getMonth(), changed.getDate()).getTime(), earliest.getTime()))
        : q.from && q.from.trim() !== 'config' ? this.day(q.from, 'from') : fallback;
      if (from > to) throw new HarnessError('input-invalid', '„from“ liegt nach „to“.');
      const days = Math.round((to.getTime() - from.getTime()) / 86_400_000) + 1;
      if (days > MAX_DAYS) throw new HarnessError('input-invalid', `Zeitraum ist zu lang (${days} Tage, höchstens ${MAX_DAYS}).`);
      const kind = (q.kind?.trim() || 'work') as DispatchRunKind;
      if (!KINDS.includes(kind)) throw new HarnessError('input-invalid', `„kind“ muss ${KINDS.join(', ')} sein.`);
      // Ende ausschließlich: Mitternacht nach dem letzten Tag (Ortszeit)
      const end = new Date(to.getFullYear(), to.getMonth(), to.getDate() + 1);
      const stats = await this.holder.run(async (b) => {
        const found = (await b.services.projects.list()).find((p) => p.key.toUpperCase() === project.toUpperCase());
        if (!found) throw new HarnessError('project-not-found', `Projekt ${project} gibt es nicht.`);
        if (boardId !== undefined) {
          const board = await b.services.boards.require(boardId);
          if (board.projectId !== found.id) throw new HarnessError('input-invalid', `Board ${boardId} gehört nicht zum Projekt ${found.key}.`);
        }
        return { key: found.key, stats: await b.services.dispatchRuns.stats({ projectId: found.id, boardId, from, to: end, kind }) };
      });
      return {
        filter: { project: stats.key, boardId: boardId ?? null, from: this.text(from), to: this.text(to), kind },
        ...stats.stats,
      };
    });
  }

  // Erstes Auftreten der neuesten Fassung der Dispatcher-Einstellung (bis jetzt)
  private async configChangedAt(): Promise<Date | null> {
    const versions = await this.holder.run((b) => b.services.dispatchRuns.configVersions(new Date(0).toISOString(), this.clock().toISOString()));
    const last = versions.at(-1);
    return last ? new Date(last.firstSeenAt) : null;
  }

  // JJJJ-MM-TT als Ortszeit-Mitternacht; ungültige Tage werden abgelehnt
  private day(raw: string, key: string): Date {
    const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(raw.trim());
    const date = m ? new Date(Number(m[1]), Number(m[2]) - 1, Number(m[3])) : null;
    if (!date || this.text(date) !== raw.trim()) throw new HarnessError('input-invalid', `„${key}“ muss ein Datum JJJJ-MM-TT sein.`);
    return date;
  }

  private text(d: Date): string {
    return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`;
  }
}
