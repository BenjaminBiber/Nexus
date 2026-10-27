import { HarnessError, HarnessProject } from '@harness/core';
import type { BoardRuntime } from '@harness/db';

// Welches Board ein Befehl meint: --board <id|all>, sonst das Board des Repos (wenn der Ordner in einem
// eingerichteten Repo mit zugeordnetem Board liegt), sonst das aktive Board aus der Konfig.
export class BoardSelection {
  // Genau ein Board; `repoDir` ist der Arbeitsordner, aus dem das Board des Repos ermittelt wird.
  // Ein mit --board genanntes Board muss es geben (sonst 'board-not-found')
  async one(rt: Pick<BoardRuntime, 'services' | 'config'>, option?: string, repoDir?: string): Promise<number> {
    if (option !== undefined) return (await rt.services.boards.require(this.parseId(option))).id;
    const repoBoard = repoDir === undefined ? null : await this.repoBoard(rt, repoDir);
    if (repoBoard !== null) return repoBoard;
    if (rt.config.activeBoard === null) {
      throw new HarnessError('board-not-selected', 'Kein aktives Board. Mit „harness board boards use <id>“ wählen oder --board <id> angeben.');
    }
    return this.parseId(String(rt.config.activeBoard));
  }

  // Ein Board oder mit „all“ alle Boards
  async many(rt: Pick<BoardRuntime, 'services' | 'config'>, option?: string, repoDir?: string): Promise<number[]> {
    if (option === 'all') return (await rt.services.boards.list()).map((b) => b.id);
    return [await this.one(rt, option, repoDir)];
  }

  // Board-Nummer aus Text
  parseId(raw: string): number {
    const id = Number(raw);
    if (!Number.isInteger(id) || id < 1) throw new HarnessError('board-id-invalid', `„${raw}“ ist keine Board-Nummer.`);
    return id;
  }

  // Board, das dem Hauptordner des eingerichteten Repos zugeordnet ist; außerhalb eines Repos oder ohne Zuordnung null
  private async repoBoard(rt: Pick<BoardRuntime, 'services' | 'config'>, dir: string): Promise<number | null> {
    let root: string;
    try {
      root = (await HarnessProject.open(dir)).root;
    } catch {
      return null;
    }
    return (await rt.services.boards.forRepo(root, rt.config))?.id ?? null;
  }
}
