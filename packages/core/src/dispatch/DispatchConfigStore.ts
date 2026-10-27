import { existsSync } from 'node:fs';
import { mkdir, readFile } from 'node:fs/promises';
import { parse, stringify } from 'yaml';
import { HarnessError } from '../errors/HarnessError.js';
import { AtomicWriter } from '../locking/AtomicWriter.js';
import type { HarnessHome } from '../registry/HarnessHome.js';
import type { DispatchConfig, PromptProblem, RawDispatchConfig } from './DispatchConfig.js';
import { DispatchConfigNormalizer } from './DispatchConfigNormalizer.js';

export interface DispatchConfigRead {
  /** Inhalt der Datei (ohne Datei: das Beispiel) – für die Oberfläche */
  raw: RawDispatchConfig;
  config: DispatchConfig;
  exists: boolean;
  /** Befunde zu eigenen Vorlagen in der Datei; leer = alles in Ordnung. Diese Vorlagen laufen auf dem Standard. */
  promptProblems: PromptProblem[];
}

// Liest und schreibt ~/.harness/dispatch.yaml; geschrieben wird nur, was die Prüfung besteht.
export class DispatchConfigStore {
  private readonly writer = new AtomicWriter();
  private readonly normalizer = new DispatchConfigNormalizer();

  constructor(private readonly home: HarnessHome) {}

  // Pfad der Datei (für Meldungen)
  get file(): string {
    return this.home.dispatchConfigFile;
  }

  // Liest die Einstellung; ohne Datei gilt das Beispiel (ausgeschaltet)
  async read(): Promise<DispatchConfigRead> {
    if (!existsSync(this.file)) {
      const raw = this.normalizer.example();
      return { raw, config: this.normalizer.normalize(raw), exists: false, promptProblems: this.normalizer.promptProblems(raw) };
    }
    let raw: unknown;
    try {
      raw = parse(await readFile(this.file, 'utf8')) ?? {};
    } catch {
      throw new HarnessError('dispatch-config-invalid', `${this.file} ist kein gültiges YAML.`);
    }
    if (typeof raw !== 'object' || Array.isArray(raw)) throw new HarnessError('dispatch-config-invalid', `${this.file} enthält keine Einstellungen.`);
    const input = raw as RawDispatchConfig;
    return { raw: input, config: this.normalizer.normalize(input), exists: true, promptProblems: this.normalizer.promptProblems(input) };
  }

  // Prüft und schreibt atomar; liefert die aufgelöste Einstellung
  async write(raw: RawDispatchConfig): Promise<DispatchConfig> {
    // Beim Speichern ist eine fehlerhafte Vorlage ein Fehler; beim Lesen fällt sie still auf den Standard zurück
    const problems = this.normalizer.promptProblems(raw);
    if (problems.length) throw DispatchConfigNormalizer.promptError(problems);
    const config = this.normalizer.normalize(raw);
    await mkdir(this.home.dir, { recursive: true });
    await this.writer.write(this.file, stringify(raw));
    return config;
  }
}
