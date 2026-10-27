import { DispatchRunBackfill } from '../dispatch/DispatchRunBackfill.js';
import type { BoardDatabase } from './BoardDatabase.js';
import { DispatchStatsCalculator } from './DispatchStatsCalculator.js';
import type { DispatchConfigVersion, DispatchRun, DispatchStats, DispatchStatsFilter } from './DispatchRunTypes.js';

// Läufe des Dispatchers erfassen und auswerten.
export class DispatchRunService {
  private readonly calculator = new DispatchStatsCalculator();

  constructor(private readonly db: BoardDatabase) {}

  // Schreibt einen ausgewerteten Lauf; false, wenn er schon erfasst ist
  record(run: DispatchRun): Promise<boolean> {
    return this.db.transaction((u) => u.dispatchRuns.insert(run));
  }

  // Merkt die Fassung der Dispatcher-Einstellung vor; false, wenn dieser Hash schon bekannt ist
  recordConfigVersion(version: DispatchConfigVersion): Promise<boolean> {
    return this.db.transaction((u) => u.dispatchRuns.recordConfigVersion(version));
  }

  // Fassungen der Einstellung, die in [from, to) zum ersten Mal auftraten, älteste zuerst
  configVersions(from: string, to: string): Promise<DispatchConfigVersion[]> {
    return this.db.transaction((u) => u.dispatchRuns.listConfigVersions(from, to));
  }

  // Läufe aller Projekte mit Start in [from, to), ältester zuerst (Optimierungs-Vorschläge)
  listAll(from: Date, to: Date): Promise<DispatchRun[]> {
    return this.db.transaction((u) => u.dispatchRuns.listAll(from.toISOString(), to.toISOString()));
  }

  // Läufe aus [dispatch]-Kommentaren nachtragen (Herkunft comments, ID c<KommentarId>); schon vorhandene bleiben unverändert,
  // daher beliebig oft ausführbar. dryRun schreibt nichts
  backfill(projectId: number, options: { dryRun?: boolean } = {}): Promise<{ found: number; created: DispatchRun[] }> {
    return this.db.transaction(async (u) => {
      const comments = await u.dispatchStats.dispatchComments(projectId);
      const ids = [...new Set(comments.map((c) => c.ticketId))];
      const runs = new DispatchRunBackfill().runs({
        projectId, comments,
        history: ids.length ? await u.dispatchStats.statusHistory(ids) : [],
        tickets: ids.length ? await u.tickets.list({ ids }) : [],
      });
      const existing = new Set(runs.length ? await u.dispatchRuns.existing(runs.map((r) => r.id)) : []);
      const created = runs.filter((r) => !existing.has(r.id));
      if (!options.dryRun) for (const run of created) await u.dispatchRuns.insert(run);
      return { found: runs.length, created };
    });
  }

  // Kennzahlen für Projekt, Board, Zeitraum und Lauf-Art (eine Lesetransaktion)
  stats(filter: DispatchStatsFilter): Promise<DispatchStats> {
    return this.db.transaction(async (u) => {
      const runs = await u.dispatchRuns.list({
        projectId: filter.projectId, from: filter.from.toISOString(), to: filter.to.toISOString(),
        ...(filter.boardId === undefined ? {} : { boardId: filter.boardId }),
      });
      // Neueste Fassung der Einstellung bis zum Ende des Zeitraums
      const versions = await u.dispatchRuns.listConfigVersions(new Date(0).toISOString(), filter.to.toISOString());
      return this.calculator.calculate({
        from: filter.from, to: filter.to, kind: filter.kind ?? 'work', runs,
        runsSince: await u.dispatchRuns.firstStarted(filter.projectId, 'dispatcher'),
        configChangedAt: versions.at(-1)?.firstSeenAt ?? null,
      });
    });
  }
}
