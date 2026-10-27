import { existsSync } from 'node:fs';
import { mkdir, readdir, readFile, rm } from 'node:fs/promises';
import { join } from 'node:path';
import { AtomicWriter } from '../../locking/AtomicWriter.js';
import type { HarnessHome } from '../../registry/HarnessHome.js';
import type { RunResult } from '../RunOutputReader.js';
import type { RunSettings, WorktreeSetupStep } from '../DispatchConfig.js';

export type RunKind = 'work' | 'merge' | 'research' | 'plan';

// Ein gestarteter Worker-Lauf; der Worker-Prozess liest ihn und schreibt sein Ergebnis daneben
export interface RunRecord {
  id: string;
  kind: RunKind;
  /** Ticket des Laufs; bei einer Recherche die Fragen-ID (Gegenstand des Laufs) */
  ticketId: string;
  boardId: number;
  title: string;
  agent: string;
  /** Arbeitsordner (Worktree des Tickets) */
  cwd: string;
  branch: string;
  /** Merge-Session: Ziel-Branch im Hauptordner */
  target: string | null;
  run: RunSettings;
  /** Umsetzung: Nummer des Versuchs (gescheiterte Versuche + 1) */
  attempt?: number;
  command: string;
  args: string[];
  prompt: string;
  logFile: string;
  startedAt: string;
  pid: number | null;
  /** Umsetzung in einem neu angelegten Worktree: Einrichtungsbefehle, die der Worker vor Claude ausführt, und ihr Zeitlimit */
  setup?: WorktreeSetupStep[];
  setupTimeoutMinutes?: number;
  /** Recherche: ID der Frage */
  questionId?: string;
  /** Recherche: Zahl der Recherchen der Frage beim Start (danach mehr = Ergebnis da) */
  researchBefore?: number;
  /** Planung: plannedAt des Tickets beim Start (danach ein neuerer Wert = Plan gespeichert) */
  plannedBefore?: string | null;
  /** Recherche und Planung: Hauptordner des Repos, zu dem der losgelöste Worktree gehört (zum Aufräumen) */
  repoRoot?: string;
}

export interface RunEnd extends RunResult {
  endedAt: string;
}

// Lauf-Dateien unter ~/.harness/dispatch/runs: <id>.json (Server) und <id>.result.json (Worker).
export class RunStore {
  private readonly writer = new AtomicWriter();

  constructor(private readonly home: HarnessHome) {}

  // Legt einen Lauf an
  async create(record: RunRecord): Promise<void> {
    await mkdir(this.home.dispatchRunsDir, { recursive: true });
    await this.writer.write(this.file(record.id), JSON.stringify(record, null, 2));
  }

  // Merkt die Prozess-ID des Workers
  async setPid(id: string, pid: number | null): Promise<void> {
    const record = await this.read(id);
    if (record) await this.writer.write(this.file(id), JSON.stringify({ ...record, pid }, null, 2));
  }

  // Lauf zur ID oder null
  async read(id: string): Promise<RunRecord | null> {
    return this.json<RunRecord>(this.file(id));
  }

  // Alle Läufe, ältester zuerst
  async list(): Promise<RunRecord[]> {
    if (!existsSync(this.home.dispatchRunsDir)) return [];
    const names = (await readdir(this.home.dispatchRunsDir)).filter((n) => n.endsWith('.json') && !n.endsWith('.result.json'));
    const records = (await Promise.all(names.map((n) => this.json<RunRecord>(join(this.home.dispatchRunsDir, n))))).filter((r): r is RunRecord => r !== null);
    return records.sort((a, b) => a.startedAt.localeCompare(b.startedAt));
  }

  // Ergebnis des Workers oder null, solange er läuft
  async result(id: string): Promise<RunEnd | null> {
    return this.json<RunEnd>(this.resultFile(id));
  }

  // Schreibt das Ergebnis (nur der Worker)
  async finish(id: string, end: RunEnd): Promise<void> {
    await mkdir(this.home.dispatchRunsDir, { recursive: true });
    await this.writer.write(this.resultFile(id), JSON.stringify(end, null, 2));
  }

  // Entfernt einen ausgewerteten Lauf
  async remove(id: string): Promise<void> {
    await rm(this.file(id), { force: true });
    await rm(this.resultFile(id), { force: true });
  }

  // Merkt eine Lauf-Zeile für die Auswertung vor (Unterordner stats), bis sie in der Datenbank steht
  async keepStats(id: string, row: unknown): Promise<void> {
    await mkdir(this.statsDir, { recursive: true });
    await this.writer.write(join(this.statsDir, `${id}.json`), JSON.stringify(row, null, 2));
  }

  // Vorgemerkte Lauf-Zeilen, nach ID
  async keptStats<T>(): Promise<Array<{ id: string; row: T }>> {
    if (!existsSync(this.statsDir)) return [];
    const names = (await readdir(this.statsDir)).filter((n) => n.endsWith('.json')).sort();
    const out: Array<{ id: string; row: T }> = [];
    for (const n of names) {
      const row = await this.json<T>(join(this.statsDir, n));
      if (row !== null) out.push({ id: n.slice(0, -'.json'.length), row });
    }
    return out;
  }

  // Entfernt eine vorgemerkte Lauf-Zeile
  async forgetStats(id: string): Promise<void> {
    await rm(join(this.statsDir, `${id}.json`), { force: true });
  }

  private get statsDir(): string {
    return join(this.home.dispatchRunsDir, 'stats');
  }

  // Liest JSON; fehlende (ENOENT) oder halb geschriebene Dateien gelten als nicht vorhanden, andere Lesefehler werfen
  private async json<T>(path: string): Promise<T | null> {
    let text: string;
    try {
      text = await readFile(path, 'utf8');
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code === 'ENOENT') return null;
      throw error;
    }
    try {
      return JSON.parse(text) as T;
    } catch {
      return null;
    }
  }

  private file(id: string): string {
    return join(this.home.dispatchRunsDir, `${id}.json`);
  }

  private resultFile(id: string): string {
    return join(this.home.dispatchRunsDir, `${id}.result.json`);
  }
}
