import { mkdir, readFile, rm } from 'node:fs/promises';
import { join } from 'node:path';
import { AtomicWriter } from '../../locking/AtomicWriter.js';
import type { HarnessHome } from '../../registry/HarnessHome.js';
import type { TestRun } from '../TestRunExtractor.js';

/** Review eines Tickets vor dem Merge: Lauf und Testläufe des Agenten */
export interface ReviewEntry {
  ticketId: string;
  branch: string;
  runId: string;
  /** Lauf-Protokoll des Laufs */
  logFile: string;
  /** Zeitpunkt des Gates (ISO) */
  at: string;
  tests: TestRun[];
}

// Review-Einträge unter ~/.harness/dispatch/reviews/<ID>.json.
export class ReviewStore {
  private readonly writer = new AtomicWriter();

  constructor(private readonly home: HarnessHome) {}

  // Speichert den Eintrag (ersetzt einen früheren desselben Tickets)
  async save(entry: ReviewEntry): Promise<void> {
    await mkdir(this.home.dispatchReviewsDir, { recursive: true });
    await this.writer.write(this.file(entry.ticketId), JSON.stringify(entry, null, 2));
  }

  // Eintrag zum Ticket oder null; fehlende oder kaputte Dateien gelten als nicht vorhanden
  async read(ticketId: string): Promise<ReviewEntry | null> {
    let text: string;
    try {
      text = await readFile(this.file(ticketId), 'utf8');
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code === 'ENOENT') return null;
      throw error;
    }
    try {
      return JSON.parse(text) as ReviewEntry;
    } catch {
      return null;
    }
  }

  // Entfernt den Eintrag
  async remove(ticketId: string): Promise<void> {
    await rm(this.file(ticketId), { force: true });
  }

  private file(ticketId: string): string {
    return join(this.home.dispatchReviewsDir, `${ticketId.replace(/[^A-Za-z0-9_-]/g, '_')}.json`);
  }
}
