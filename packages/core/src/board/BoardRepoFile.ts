import { existsSync } from 'node:fs';
import { mkdir, readFile } from 'node:fs/promises';
import { parse, stringify } from 'yaml';
import { HarnessError } from '../errors/HarnessError.js';
import { AtomicWriter } from '../locking/AtomicWriter.js';
import type { HarnessHome } from '../registry/HarnessHome.js';
import type { BoardRepoStore } from './BoardRepoPaths.js';

// Liest und schreibt ~/.harness/board-repos.yaml: je Board-ID der eigene Repo-Pfad dieses Benutzers (null = kein Pfad).
export class BoardRepoFile implements BoardRepoStore {
  private readonly writer = new AtomicWriter();

  constructor(private readonly home: HarnessHome) {}

  // Pfad der Datei (für Meldungen)
  get file(): string {
    return this.home.boardReposFile;
  }

  // Liest die Zuordnung; ohne Datei leer
  async read(): Promise<Map<number, string | null>> {
    if (!existsSync(this.file)) return new Map();
    let raw: unknown;
    try {
      raw = parse(await readFile(this.file, 'utf8')) ?? {};
    } catch {
      throw new HarnessError('board-repos-invalid', `${this.file} ist kein gültiges YAML.`);
    }
    if (typeof raw !== 'object' || Array.isArray(raw)) throw new HarnessError('board-repos-invalid', `${this.file} enthält keine Zuordnung Board → Repo-Pfad.`);
    const entries = new Map<number, string | null>();
    for (const [key, value] of Object.entries(raw as Record<string, unknown>)) {
      const id = Number(key);
      if (!Number.isInteger(id) || id <= 0) throw new HarnessError('board-repos-invalid', `In ${this.file} ist „${key}“ keine Board-ID.`);
      if (value !== null && typeof value !== 'string') throw new HarnessError('board-repos-invalid', `In ${this.file} ist der Pfad von Board ${id} kein Text.`);
      entries.set(id, value && value.trim() ? value.trim() : null);
    }
    return entries;
  }

  // Schreibt die Zuordnung atomar, nach Board-ID sortiert; der Ordner wird bei Bedarf angelegt
  async write(entries: Map<number, string | null>): Promise<void> {
    const sorted = Object.fromEntries([...entries].sort(([a], [b]) => a - b));
    await mkdir(this.home.dir, { recursive: true });
    await this.writer.write(this.file, stringify(sorted));
  }
}
