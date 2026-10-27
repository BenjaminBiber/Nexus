import { readFile } from 'node:fs/promises';
import { FileLock } from '../locking/FileLock.js';
import { AtomicWriter } from '../locking/AtomicWriter.js';
import { HarnessError } from '../errors/HarnessError.js';
import type { HarnessLayout } from '../project/HarnessLayout.js';

export type CounterKind = 'session';
// Stände je Art; ältere Schlüssel (spec, ticket, question) bleiben unverändert erhalten
type CounterState = Record<string, number> & Record<CounterKind, number>;

// Zähler für die Session-Nummern eines Repos; nur unter Sperre gelesen und geschrieben.
export class Counter {
  private readonly lock: FileLock;
  private readonly writer = new AtomicWriter();

  constructor(private readonly layout: HarnessLayout) {
    this.lock = new FileLock(layout.countersLock);
  }

  // Erhöht den Zähler und liefert die neue Nummer (die erste Nummer ist 1)
  next(kind: CounterKind): Promise<number> {
    return this.lock.withLock(async () => {
      const state = await this.read();
      state[kind] += 1;
      await this.writer.write(this.layout.countersFile, JSON.stringify(state));
      return state[kind];
    });
  }

  // Liest den Stand; fehlende oder beschädigte Datei stoppt, statt bei 1 neu zu beginnen
  async read(): Promise<CounterState> {
    try {
      const data = JSON.parse(await readFile(this.layout.countersFile, 'utf8')) as Record<string, unknown>;
      if (!data || typeof data !== 'object' || Array.isArray(data)) throw new Error('Format');
      // `session` kam später hinzu: in älteren Dateien fehlt er und beginnt bei 0
      const session = data.session === undefined ? 0 : data.session;
      if (!Number.isInteger(session)) throw new Error('Format');
      // Nicht mehr genutzte Schlüssel wie `spec` bleiben erhalten, damit alte Dateien unverändert weiterleben
      const kept = Object.fromEntries(Object.entries(data).filter(([, v]) => Number.isInteger(v))) as Record<string, number>;
      return { ...kept, session: session as number };
    } catch {
      throw new HarnessError('counter-corrupt', `Der Zähler ist beschädigt: ${this.layout.countersFile}. Es wird nicht bei 1 neu begonnen.`);
    }
  }
}
