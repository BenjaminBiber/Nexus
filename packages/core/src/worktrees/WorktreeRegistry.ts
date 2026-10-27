import { existsSync } from 'node:fs';
import { readFile } from 'node:fs/promises';
import { FileLock } from '../locking/FileLock.js';
import { AtomicWriter } from '../locking/AtomicWriter.js';
import type { HarnessLayout } from '../project/HarnessLayout.js';

export interface WorktreeEntry {
  path: string;
  tickets: string[];
  readyToMerge: boolean;
  /** true, wenn der Ordner aufgeräumt wurde (der Branch bleibt erhalten) */
  removed?: boolean;
  /** Einrichtung des Dispatchers (worktreeSetup): pending = noch nicht gelungen, done = gelaufen; fehlt = keine vorgesehen */
  setup?: 'pending' | 'done';
}

export type WorktreeMap = Record<string, WorktreeEntry>;

// Verzeichnis der Worktrees (Branch → Pfad, Tickets, „bereit zum Merge“); Schreiben nur unter Sperre.
export class WorktreeRegistry {
  private readonly lock: FileLock;
  private readonly writer = new AtomicWriter();

  constructor(private readonly layout: HarnessLayout) {
    this.lock = new FileLock(layout.worktreesLock);
  }

  // Liest alle Einträge (leer, wenn es noch keine gibt)
  async read(): Promise<WorktreeMap> {
    if (!existsSync(this.layout.worktreesFile)) return {};
    return JSON.parse(await readFile(this.layout.worktreesFile, 'utf8')) as WorktreeMap;
  }

  // Legt einen neuen Worktree-Eintrag mit seinem ersten Ticket an
  add(branch: string, path: string, ticketId: string): Promise<void> {
    return this.change((map) => {
      map[branch] = { path, tickets: [ticketId], readyToMerge: false };
    });
  }

  // Nimmt ein weiteres Ticket in den Worktree auf (ohne Doppelung)
  addTicket(branch: string, ticketId: string): Promise<void> {
    return this.change((map) => {
      const entry = map[branch];
      if (entry && !entry.tickets.includes(ticketId)) entry.tickets.push(ticketId);
    });
  }

  // Setzt oder löscht die Markierung „bereit zum Merge“
  setReady(branch: string, ready: boolean): Promise<void> {
    return this.change((map) => {
      if (map[branch]) map[branch].readyToMerge = ready;
    });
  }

  // Merkt den Stand der Einrichtung des Worktrees
  setSetup(branch: string, state: 'pending' | 'done'): Promise<void> {
    return this.change((map) => {
      if (map[branch]) map[branch].setup = state;
    });
  }

  // Markiert den Worktree-Ordner als aufgeräumt
  markRemoved(branch: string): Promise<void> {
    return this.change((map) => {
      if (map[branch]) map[branch].removed = true;
    });
  }

  // Markiert einen wieder angelegten Worktree-Ordner als vorhanden
  markRestored(branch: string): Promise<void> {
    return this.change((map) => {
      if (map[branch]) map[branch].removed = false;
    });
  }

  // Liest, ändert und schreibt das Verzeichnis in einem Zug unter der Sperre
  private change(mutate: (map: WorktreeMap) => void): Promise<void> {
    return this.lock.withLock(async () => {
      const map = await this.read();
      mutate(map);
      await this.writer.write(this.layout.worktreesFile, JSON.stringify(map, null, 2));
    });
  }
}
