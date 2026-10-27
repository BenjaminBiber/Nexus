import { readFile } from 'node:fs/promises';
import { AtomicWriter } from '../locking/AtomicWriter.js';
import { FileLock } from '../locking/FileLock.js';
import type { Counter } from '../counter/Counter.js';
import type { HarnessLayout } from '../project/HarnessLayout.js';

type NumberMap = Record<string, number>;

// Vergibt jeder Session eines Projekts dauerhaft eine fortlaufende Nummer (dieselbe Session behält ihre Nummer).
export class SessionNumbers {
  private readonly lock: FileLock;
  private readonly writer = new AtomicWriter();

  constructor(
    private readonly layout: HarnessLayout,
    private readonly counter: Counter,
  ) {
    this.lock = new FileLock(layout.sessionNumbersLock);
  }

  // Liefert die Nummer der Session; vergibt eine neue, falls sie noch keine hat
  assign(sessionId: string): Promise<number> {
    return this.lock.withLock(async () => {
      const map = await this.read();
      if (map[sessionId] !== undefined) return map[sessionId];
      map[sessionId] = await this.counter.next('session');
      await this.writer.write(this.layout.sessionNumbersFile, JSON.stringify(map));
      return map[sessionId];
    });
  }

  // Liest die Zuordnung; fehlende Datei = noch keine Sessions
  private async read(): Promise<NumberMap> {
    try {
      return JSON.parse(await readFile(this.layout.sessionNumbersFile, 'utf8')) as NumberMap;
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code === 'ENOENT') return {};
      throw error; // beschädigt: nicht still neu beginnen, sonst würden Nummern doppelt vergeben
    }
  }
}
