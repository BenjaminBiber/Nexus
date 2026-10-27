import { appendFileSync, createWriteStream, existsSync, mkdirSync, statSync, truncateSync, type WriteStream } from 'node:fs';
import { open, readdir } from 'node:fs/promises';
import { dirname, join } from 'node:path';
import { HarnessError } from '../../errors/HarnessError.js';
import type { HarnessHome } from '../../registry/HarnessHome.js';
import type { RunKind } from './RunStore.js';

const MAX_LOG_BYTES = 1024 * 1024;
const MAX_READ_CHARS = 200_000;
const NAME = /^[A-Za-z0-9._-]+\.log$/;

/** Lauf-Protokoll eines Tickets oder einer Frage */
export interface RunLogEntry {
  /** Dateiname, z. B. ARC-0001-20261016-091500-merge.log */
  name: string;
  kind: RunKind;
  /** Start laut Namen als ISO-Zeit */
  started: string | null;
}

// Protokolle des Dispatchers: dispatch.log (Ereignisse) und je Lauf eine Datei mit Anweisung und Ausgabe.
export class DispatchLog {
  constructor(private readonly home: HarnessHome, private readonly clock: () => Date = () => new Date()) {}

  // Hängt eine Zeile an dispatch.log an; über 1 MB wird neu begonnen. Fehler sind kein Grund zum Abbruch
  line(text: string): void {
    try {
      mkdirSync(this.home.dispatchLogsDir, { recursive: true });
      const file = this.filePath('dispatch.log');
      if ((statSync(file, { throwIfNoEntry: false })?.size ?? 0) > MAX_LOG_BYTES) truncateSync(file, 0);
      appendFileSync(file, `${this.stamp(this.clock())}  ${text}\n`);
    } catch {
      /* Protokoll ist nur Hilfe */
    }
  }

  // Öffnet (und legt bei Bedarf den Ordner an) eine Lauf-Protokolldatei zum Anhängen
  openRunLog(file: string): WriteStream {
    mkdirSync(dirname(file), { recursive: true });
    return createWriteStream(file, { flags: 'a' });
  }

  // Name eines Lauf-Protokolls (ohne .log): <ID>-<JJJJMMTT-HHMMSS>[-merge|-research|-plan]
  runName(ticketId: string, kind: RunKind, at: Date): string {
    const p = (n: number) => String(n).padStart(2, '0');
    const stamp = `${at.getFullYear()}${p(at.getMonth() + 1)}${p(at.getDate())}-${p(at.getHours())}${p(at.getMinutes())}${p(at.getSeconds())}`;
    return `${ticketId}-${stamp}${kind === 'work' ? '' : `-${kind}`}`;
  }

  // Voller Pfad einer Protokolldatei
  filePath(name: string): string {
    return join(this.home.dispatchLogsDir, name);
  }

  // Protokolldateien: dispatch.log zuerst, danach die neuesten Läufe
  async list(): Promise<string[]> {
    if (!existsSync(this.home.dispatchLogsDir)) return [];
    const names = (await readdir(this.home.dispatchLogsDir)).filter((n) => NAME.test(n));
    const runs = names.filter((n) => n !== 'dispatch.log').sort((a, b) => this.stampOf(b).localeCompare(this.stampOf(a)) || a.localeCompare(b));
    return [...(names.includes('dispatch.log') ? ['dispatch.log'] : []), ...runs];
  }

  // Lauf-Protokolle eines Tickets oder einer Frage, neueste zuerst. Der Name beginnt genau mit `<ID>-` und dem Zeitstempel
  // (ARC-00010-… gehört nicht zu ARC-0001); `started` ist der Zeitstempel des Namens (Ortszeit) als ISO-Zeit
  async runs(id: string): Promise<RunLogEntry[]> {
    const ticket = id.trim().toUpperCase();
    if (!/^[A-Z][A-Z0-9]*-[A-Z]?\d+$/.test(ticket)) throw new HarnessError('id-invalid', `„${id}“ ist keine Ticket- oder Fragen-ID.`);
    if (!existsSync(this.home.dispatchLogsDir)) return [];
    const own = new RegExp(`^${ticket}-(\\d{8}-\\d{6})(?:-(merge|research|plan))?\\.log$`);
    const out: RunLogEntry[] = [];
    for (const name of await readdir(this.home.dispatchLogsDir)) {
      const m = own.exec(name);
      if (m) out.push({ name, kind: (m[2] as RunLogEntry['kind'] | undefined) ?? 'work', started: this.startedAt(m[1]) });
    }
    return out.sort((a, b) => this.stampOf(b.name).localeCompare(this.stampOf(a.name)) || a.name.localeCompare(b.name));
  }

  // Inhalt (Ende) einer Protokolldatei; nur einfache Dateinamen, die tatsächlich im Protokollordner liegen
  // (reservierte Gerätenamen wie CON.log fallen so heraus). Gelesen wird nur das Ende der Datei
  async read(name: string): Promise<string> {
    if (!NAME.test(name) || name.includes('..')) throw new HarnessError('log-name-invalid', `„${name}“ ist kein Protokoll des Dispatchers.`);
    const names = existsSync(this.home.dispatchLogsDir) ? await readdir(this.home.dispatchLogsDir) : [];
    if (!names.includes(name)) throw new HarnessError('log-not-found', `Das Protokoll ${name} gibt es nicht.`);
    const handle = await open(this.filePath(name), 'r');
    try {
      const size = (await handle.stat()).size;
      const start = Math.max(0, size - MAX_READ_CHARS * 4);
      const buffer = Buffer.alloc(size - start);
      const { bytesRead } = await handle.read(buffer, 0, buffer.length, start);
      const text = buffer.subarray(0, bytesRead).toString('utf8');
      return start > 0 || text.length > MAX_READ_CHARS ? `…${text.slice(-MAX_READ_CHARS)}` : text;
    } finally {
      await handle.close();
    }
  }

  // Zeitstempel im Namen für die Sortierung (JJJJMMTT-HHMMSS)
  private stampOf(name: string): string {
    return /(\d{8}-\d{6})/.exec(name)?.[1] ?? '';
  }

  // Zeitstempel JJJJMMTT-HHMMSS (Ortszeit) als ISO-Zeit; ungültig → null
  private startedAt(stamp: string): string | null {
    const n = (from: number, to: number) => Number(stamp.slice(from, to));
    const d = new Date(n(0, 4), n(4, 6) - 1, n(6, 8), n(9, 11), n(11, 13), n(13, 15));
    return Number.isNaN(d.getTime()) ? null : d.toISOString();
  }

  // Ortszeit JJJJ-MM-TT HH:MM:SS
  private stamp(d: Date): string {
    const p = (n: number) => String(n).padStart(2, '0');
    return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())} ${p(d.getHours())}:${p(d.getMinutes())}:${p(d.getSeconds())}`;
  }
}
