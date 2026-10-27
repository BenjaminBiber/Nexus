import { existsSync, mkdirSync } from 'node:fs';
import { readFile } from 'node:fs/promises';
import { dirname, join } from 'node:path';
import { AtomicWriter, type HarnessHome } from '@harness/core';
import type { TicketDraftResult } from './TicketDraftService.js';

// Zuletzt erzeugte (und bearbeitete) Ticket-Entwürfe je Dokument, damit sie sich später wieder öffnen lassen.
// Schlüssel ist der volle Pfad der Datei; alles steht in einer Datei im Harness-Ordner (je Benutzer lokal).
export class DocDraftStore {
  private readonly writer = new AtomicWriter();
  // Schreibvorgänge nacheinander, damit sich zwei Änderungen nicht überschreiben
  private queue: Promise<unknown> = Promise.resolve();

  constructor(private readonly home: HarnessHome) {}

  private get file(): string {
    return join(this.home.dir, 'doc-drafts.json');
  }

  // Gespeicherte Entwürfe der Datei; null, wenn es keine gibt
  async get(fullPath: string): Promise<TicketDraftResult | null> {
    return (await this.readAll())[this.key(fullPath)] ?? null;
  }

  // Die Dateien unter `fullPaths`, zu denen Entwürfe gespeichert sind
  async savedAmong(fullPaths: string[]): Promise<Set<string>> {
    const all = await this.readAll();
    return new Set(fullPaths.filter((p) => (all[this.key(p)]?.drafts?.length ?? 0) > 0));
  }

  // Merkt sich die Entwürfe der Datei (ersetzt frühere)
  put(fullPath: string, result: TicketDraftResult): Promise<void> {
    return this.change((all) => { all[this.key(fullPath)] = result; });
  }

  // Vergisst die Entwürfe der Datei (verworfen oder als Tickets angelegt)
  remove(fullPath: string): Promise<void> {
    return this.change((all) => { delete all[this.key(fullPath)]; });
  }

  private change(apply: (all: Record<string, TicketDraftResult>) => void): Promise<void> {
    const next = this.queue.then(async () => {
      const all = await this.readAll();
      apply(all);
      mkdirSync(dirname(this.file), { recursive: true });
      await this.writer.write(this.file, JSON.stringify(all, null, 2));
    });
    this.queue = next.catch(() => {});
    return next;
  }

  private async readAll(): Promise<Record<string, TicketDraftResult>> {
    if (!existsSync(this.file)) return {};
    try {
      const raw = JSON.parse(await readFile(this.file, 'utf8')) as unknown;
      return raw && typeof raw === 'object' && !Array.isArray(raw) ? raw as Record<string, TicketDraftResult> : {};
    } catch {
      return {};
    }
  }

  // Unter Windows zählen Groß/Klein und die Richtung der Schrägstriche im Pfad nicht
  private key(fullPath: string): string {
    const path = fullPath.replace(/\\/g, '/');
    return process.platform === 'win32' ? path.toLowerCase() : path;
  }
}
