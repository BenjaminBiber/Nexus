import { readFile } from 'node:fs/promises';
import { AtomicWriter } from '../locking/AtomicWriter.js';
import { FileLock } from '../locking/FileLock.js';
import type { HarnessLayout } from '../project/HarnessLayout.js';

const KEEP_MS = 30 * 24 * 60 * 60 * 1000;

type Marks = Record<string, string>;

// Merkt sich je Session, wann zuletzt offene Fragen eingespeist wurden, damit sich der Text nicht bei jedem Prompt wiederholt.
export class QuestionContextMarks {
  private readonly lock: FileLock;
  private readonly writer = new AtomicWriter();

  constructor(
    private readonly layout: HarnessLayout,
    private readonly now: () => number = () => Date.now(),
  ) {
    this.lock = new FileLock(layout.questionContextLock);
  }

  // Zeitpunkt der letzten Einspeisung für diese Session oder null; eine fehlende oder beschädigte Datei zählt als „nie“
  async get(sessionId: string): Promise<string | null> {
    return (await this.read())[sessionId] ?? null;
  }

  // Setzt den Zeitpunkt und räumt Einträge auf, die älter als 30 Tage sind
  async set(sessionId: string, iso: string): Promise<void> {
    await this.lock.withLock(async () => {
      const marks = await this.read();
      marks[sessionId] = iso;
      const fresh = Object.fromEntries(Object.entries(marks).filter(([, at]) => this.now() - Date.parse(at) < KEEP_MS));
      await this.writer.write(this.layout.questionContextFile, JSON.stringify(fresh));
    });
  }

  // Liest die Datei; Fehler ergeben eine leere Zuordnung
  private async read(): Promise<Marks> {
    try {
      const data = JSON.parse(await readFile(this.layout.questionContextFile, 'utf8'));
      return data && typeof data === 'object' && !Array.isArray(data) ? (data as Marks) : {};
    } catch {
      return {};
    }
  }
}
