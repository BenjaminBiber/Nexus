import { mkdir, readFile, rm } from 'node:fs/promises';
import { AtomicWriter } from '../../locking/AtomicWriter.js';
import type { HarnessHome } from '../../registry/HarnessHome.js';
import { INBOX_KEY_PATTERN, MAX_INBOX_KEY_LENGTH, MAX_INBOX_KEYS } from './InboxTypes.js';

// Speichert die manuelle Reihenfolge von „Meine Aufgaben“ je Benutzer lokal in ~/.harness/inbox-order.json.
// Eine fehlende oder kaputte Datei ist nie ein Fehler: dann gilt die Standardsortierung.
export class InboxOrderFile {
  private readonly writer = new AtomicWriter();

  constructor(private readonly home: HarnessHome) {}

  get file(): string {
    return this.home.inboxOrderFile;
  }

  // Gespeicherte Schlüssel in ihrer Reihenfolge; ungültige Einträge werden übergangen
  async read(): Promise<string[]> {
    let raw: unknown;
    try {
      raw = JSON.parse(await readFile(this.file, 'utf8'));
    } catch {
      return [];
    }
    const keys = (raw as { keys?: unknown } | null)?.keys;
    if (!Array.isArray(keys)) return [];
    return this.clean(keys.filter((k): k is string => typeof k === 'string'));
  }

  // Schreibt die Schlüssel atomar (doppelte und ungültige fallen weg); der Ordner wird bei Bedarf angelegt
  async write(keys: readonly string[]): Promise<void> {
    await mkdir(this.home.dir, { recursive: true });
    await this.writer.write(this.file, `${JSON.stringify({ version: 1, keys: this.clean(keys) }, null, 2)}\n`);
  }

  // Setzt die Reihenfolge zurück
  async clear(): Promise<void> {
    await rm(this.file, { force: true });
  }

  private clean(keys: readonly string[]): string[] {
    const valid = keys.filter((k) => k.length <= MAX_INBOX_KEY_LENGTH && INBOX_KEY_PATTERN.test(k));
    return [...new Set(valid)].slice(0, MAX_INBOX_KEYS);
  }
}
