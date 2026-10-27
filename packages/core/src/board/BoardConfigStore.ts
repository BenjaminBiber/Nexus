import { existsSync } from 'node:fs';
import { mkdir, readFile } from 'node:fs/promises';
import { isAbsolute, resolve } from 'node:path';
import { parse, stringify } from 'yaml';
import { HarnessError } from '../errors/HarnessError.js';
import { AtomicWriter } from '../locking/AtomicWriter.js';
import type { HarnessHome } from '../registry/HarnessHome.js';
import { BOARD_PROVIDERS, GUID_PATTERN, type BoardConfig, type BoardProvider } from './BoardConfig.js';

// Pflichtfelder für SQL Server (Reihenfolge = Reihenfolge in der Meldung)
const MSSQL_REQUIRED = ['server', 'database', 'user', 'password'] as const;
// Pflichtfelder für jeden Anbieter
const COMMON_REQUIRED = ['userId', 'userName'] as const;
// Felder, die als Text gelten, auch wenn YAML sie als Zahl liest
const TEXT_FIELDS = ['server', 'database', 'user', 'password', 'userName'] as const;

// Liest und schreibt ~/.harness/board.yaml und prüft die Pflichtfelder.
export class BoardConfigStore {
  private readonly writer = new AtomicWriter();

  constructor(private readonly home: HarnessHome) {}

  // Pfad der Datei (für Meldungen)
  get file(): string {
    return this.home.boardConfigFile;
  }

  // Liest die Konfig; null, wenn die Datei fehlt
  async read(): Promise<BoardConfig | null> {
    if (!existsSync(this.file)) return null;
    let raw: unknown;
    try {
      raw = parse(await readFile(this.file, 'utf8'));
    } catch {
      throw new HarnessError('board-config-invalid', `${this.file} ist kein gültiges YAML.`);
    }
    return this.validate(raw);
  }

  // Liest die Konfig oder meldet, wie sie angelegt wird
  async require(): Promise<BoardConfig> {
    const config = await this.read();
    if (!config) {
      throw new HarnessError('board-config-missing', `${this.file} fehlt – mit „harness board“ die Oberfläche öffnen und unter Einstellungen → Board-Datenbank anlegen.`);
    }
    return config;
  }

  // Schreibt die Konfig atomar; der Ordner wird bei Bedarf angelegt
  async write(config: BoardConfig): Promise<void> {
    await mkdir(this.home.dir, { recursive: true });
    await this.writer.write(this.file, stringify(this.validate(config)));
  }

  // Prüft Pflichtfelder und Typen und ergänzt Standardwerte
  validate(raw: unknown): BoardConfig {
    const data = { ...((raw ?? {}) as Record<string, unknown>) };
    // Unquotierte Zahlen im YAML (z. B. password: 1234) gelten als Text
    for (const key of TEXT_FIELDS) {
      if (typeof data[key] === 'number' && Number.isFinite(data[key])) data[key] = String(data[key]);
    }
    const provider = data.provider === undefined || data.provider === null ? 'mssql' : data.provider;
    if (!BOARD_PROVIDERS.includes(provider as BoardProvider)) {
      throw new HarnessError('board-config-invalid', `provider in ${this.file} ist ungültig (erlaubt: ${BOARD_PROVIDERS.join(', ')}).`);
    }
    const required = provider === 'mssql' ? [...MSSQL_REQUIRED, ...COMMON_REQUIRED] : [...COMMON_REQUIRED];
    const missing = required.filter((key) => typeof data[key] !== 'string' || !(data[key] as string).trim());
    if (missing.length) {
      const label = provider === 'mssql' ? ' für SQL Server' : '';
      throw new HarnessError('board-config-invalid', `In ${this.file} fehlen${label}: ${missing.join(', ')}.`);
    }
    if (!GUID_PATTERN.test(data.userId as string)) {
      throw new HarnessError('board-config-invalid', `userId in ${this.file} ist keine GUID.`);
    }
    let port = data.port === undefined ? 1433 : Number(data.port);
    if (provider === 'mssql') {
      if (!Number.isInteger(port) || port < 1 || port > 65535) {
        throw new HarnessError('board-config-invalid', `port in ${this.file} ist ungültig.`);
      }
    } else if (!Number.isInteger(port) || port < 1 || port > 65535) {
      port = 1433;
    }
    if (provider === 'sqlite' && data.file !== undefined && data.file !== null && (typeof data.file !== 'string' || !data.file.trim())) {
      throw new HarnessError(
        'board-config-invalid',
        `file in ${this.file} ist leer – Pfad der SQLite-Datei angeben oder die Zeile weglassen (Standard: ${this.home.boardDbFile}).`,
      );
    }
    // Fehlt file (oder ist leer), gilt <HARNESS_HOME>/board.db; relative Pfade gelten ab HARNESS_HOME, nicht ab dem Arbeitsordner
    const rawFile = typeof data.file === 'string' && data.file.trim() ? data.file.trim() : this.home.boardDbFile;
    const file = isAbsolute(rawFile) ? rawFile : resolve(this.home.dir, rawFile);
    const active = data.activeBoard === undefined || data.activeBoard === null ? null : Number(data.activeBoard);
    const text = (key: string) => (typeof data[key] === 'string' ? (data[key] as string) : '');
    return {
      provider: provider as BoardProvider,
      file,
      server: text('server'),
      port,
      database: text('database'),
      user: text('user'),
      password: text('password'),
      encrypt: data.encrypt === undefined ? true : data.encrypt === true,
      trustServerCertificate: data.trustServerCertificate === undefined ? true : data.trustServerCertificate === true,
      userId: (data.userId as string).toLowerCase(),
      userName: data.userName as string,
      activeBoard: active !== null && Number.isInteger(active) ? active : null,
    };
  }
}
