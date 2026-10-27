import { existsSync } from 'node:fs';
import { mkdir, readFile } from 'node:fs/promises';
import { AtomicWriter } from '../locking/AtomicWriter.js';
import { HarnessError } from '../errors/HarnessError.js';
import type { HarnessHome } from '../registry/HarnessHome.js';
import type { Session } from './Session.js';

// Speichert die Sessions aller Projekte in `~/.harness/sessions.json` (atomar).
export class SessionPersistence {
  private readonly writer = new AtomicWriter();

  constructor(private readonly home: HarnessHome) {}

  // Liest die gespeicherten Sessions (leer, wenn es die Datei nicht gibt)
  async read(): Promise<Session[]> {
    if (!existsSync(this.home.sessionsFile)) return [];
    try {
      const data = JSON.parse(await readFile(this.home.sessionsFile, 'utf8')) as { sessions?: Session[] };
      return Array.isArray(data.sessions) ? data.sessions : [];
    } catch {
      throw new HarnessError('sessions-corrupt', `Die Sessiondatei ist beschädigt: ${this.home.sessionsFile}`);
    }
  }

  // Schreibt die Sessions; der Ordner wird bei Bedarf angelegt
  async write(sessions: Session[]): Promise<void> {
    await mkdir(this.home.dir, { recursive: true });
    await this.writer.write(this.home.sessionsFile, JSON.stringify({ sessions }));
  }
}
