import { existsSync } from 'node:fs';
import { mkdir, readFile } from 'node:fs/promises';
import { AtomicWriter } from '../../locking/AtomicWriter.js';
import type { HarnessHome } from '../../registry/HarnessHome.js';
import type { Suggestion } from './SuggestionTypes.js';

/** Letztes Ergebnis der Claude-Analyse */
export interface StoredAnalysis {
  at: string;
  /** Hash der Einstellung, gegen die analysiert wurde (DispatchConfigFingerprint) */
  configHash: string;
  costUsd: number | null;
  suggestions: Suggestion[];
  /** Antworten, die Claude geliefert hat, die aber nicht als Vorschlag taugten (kein Titel); fehlt in älteren Dateien */
  skipped?: number;
}

export interface SuggestionState {
  /** IDs ausgeblendeter Vorschläge */
  dismissed: string[];
  analysis: StoredAnalysis | null;
}

const empty = (): SuggestionState => ({ dismissed: [], analysis: null });

// Liest und schreibt ~/.harness/dispatch-suggestions.json (Ausblendungen und letzte Claude-Analyse); eine kaputte Datei gilt als leer.
export class SuggestionStateStore {
  private readonly writer = new AtomicWriter();

  constructor(private readonly home: HarnessHome, private readonly warn: (message: string) => void = () => {}) {}

  get file(): string {
    return this.home.dispatchSuggestionsFile;
  }

  async read(): Promise<SuggestionState> {
    if (!existsSync(this.file)) return empty();
    try {
      const parsed = JSON.parse(await readFile(this.file, 'utf8')) as Partial<SuggestionState> | null;
      return {
        dismissed: Array.isArray(parsed?.dismissed) ? parsed.dismissed.filter((id): id is string => typeof id === 'string') : [],
        analysis: parsed?.analysis && typeof parsed.analysis === 'object' ? parsed.analysis : null,
      };
    } catch {
      this.warn(`${this.file} ist kein gültiges JSON und wird als leer behandelt.`);
      return empty();
    }
  }

  async write(state: SuggestionState): Promise<void> {
    await mkdir(this.home.dir, { recursive: true });
    await this.writer.write(this.file, JSON.stringify(state, null, 2));
  }

  // Liest, ändert und schreibt; liefert den neuen Zustand
  async update(change: (state: SuggestionState) => SuggestionState): Promise<SuggestionState> {
    const next = change(await this.read());
    await this.write(next);
    return next;
  }
}
