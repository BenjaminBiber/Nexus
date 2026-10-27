import {
  ConfigPatcher, DispatchConfigFingerprint, DispatchConfigStore, HarnessError, OptimizationAdvisor, OptimizationFactsCollector,
  SuggestionStateStore,
  type AdvisorResult, type DispatchConfigRead, type HarnessHome, type InsufficientData, type OptimizationBasis, type OptimizationFacts,
  type StoredAnalysis, type Suggestion,
} from '@harness/core';
import type { BoardRuntimeHolder } from '../dbboard/BoardRuntimeHolder.js';
import type { DispatchConfigView } from '../routes/DispatchRoutes.js';
import { PromptCatalog } from './PromptCatalog.js';

/** Ausgewertet werden die Läufe der letzten 180 Tage */
const RUN_WINDOW_DAYS = 180;
const ID = /^[0-9a-f]{16}$/;

/** Antwort von GET /api/dispatch/suggestions */
export interface SuggestionListDto {
  /** Hash der Einstellung, auf der die Liste beruht – Pflicht beim Übernehmen */
  configHash: string;
  basis: OptimizationBasis;
  suggestions: Suggestion[];
  dismissed: Suggestion[];
  insufficient: InsufficientData[];
  analysis: { at: string; costUsd: number | null; stale: boolean; count: number; skipped: number } | null;
}

/** Grundlage einer Auswertung: Einstellung, Kennzahlen und Regel-Vorschläge */
export interface SuggestionContext {
  read: DispatchConfigRead;
  configHash: string;
  facts: OptimizationFacts;
  advice: AdvisorResult;
}

// Optimierungs-Vorschläge zur Dispatcher-Einstellung: berechnen (Läufe aller Projekte), übernehmen, ausblenden, Claude-Ergebnis merken.
export class DispatchSuggestionService {
  private readonly store: DispatchConfigStore;
  private readonly state: SuggestionStateStore;
  private readonly fingerprint = new DispatchConfigFingerprint();
  private readonly patcher = new ConfigPatcher();

  constructor(private readonly holder: BoardRuntimeHolder, home: HarnessHome, private readonly clock: () => Date = () => new Date()) {
    this.store = new DispatchConfigStore(home);
    this.state = new SuggestionStateStore(home, (message) => console.warn(message));
  }

  async list(): Promise<SuggestionListDto> {
    return this.compose(await this.context());
  }

  // Liest Einstellung, Läufe und Konfig-Fassungen und wendet den Regelkatalog an; kaputte Einstellung → dispatch-config-invalid
  async context(): Promise<SuggestionContext> {
    const read = await this.store.read();
    const now = this.clock();
    const since = new Date(now.getTime() - RUN_WINDOW_DAYS * 86_400_000);
    const until = new Date(now.getTime() + 60_000);
    const { runs, versions } = await this.holder.run(async (b) => ({
      runs: await b.services.dispatchRuns.listAll(since, until),
      versions: await b.services.dispatchRuns.configVersions(new Date(0).toISOString(), until.toISOString()),
    }));
    const facts = new OptimizationFactsCollector().collect({ runs, config: read.config, versions, since: since.toISOString() });
    const advice = new OptimizationAdvisor().advise(facts, read.config, now);
    return { read, configHash: this.fingerprint.hash(read.config), facts, advice };
  }

  // Übernimmt die Änderung eines Vorschlags; geänderte Einstellung → conflict (config-changed), unbekannte ID → suggestion-not-found
  async apply(id: string, configHash: unknown): Promise<{ view: DispatchConfigView; list: SuggestionListDto }> {
    this.requireId(id);
    if (typeof configHash !== 'string' || !configHash) throw new HarnessError('input-invalid', '„configHash“ fehlt.');
    const ctx = await this.context();
    if (ctx.configHash !== configHash) {
      throw new HarnessError('conflict', 'Die Dispatcher-Einstellung wurde inzwischen geändert – die Vorschläge wurden neu berechnet.', { reason: 'config-changed' });
    }
    const list = await this.compose(ctx);
    const found = [...list.suggestions, ...list.dismissed].find((s) => s.id === id);
    if (!found) throw new HarnessError('suggestion-not-found', 'Diesen Vorschlag gibt es nicht mehr.');
    if (!found.patch) throw new HarnessError('input-invalid', 'Dieser Vorschlag ist nur ein Hinweis und ändert nichts.');
    await this.store.write(this.patcher.apply(ctx.read.raw, ctx.read.config, found.patch));
    const read = await this.store.read();
    return {
      view: { file: this.store.file, exists: true, raw: read.raw, config: read.config, error: null, promptInfo: new PromptCatalog().list() },
      list: await this.list(),
    };
  }

  // Blendet einen Vorschlag aus (hidden) oder wieder ein
  async dismiss(id: string, hidden: boolean): Promise<SuggestionListDto> {
    this.requireId(id);
    await this.state.update((s) => ({
      ...s, dismissed: hidden ? [...new Set([...s.dismissed, id])] : s.dismissed.filter((d) => d !== id),
    }));
    return this.list();
  }

  async saveAnalysis(analysis: StoredAnalysis): Promise<void> {
    await this.state.update((s) => ({ ...s, analysis }));
  }

  // Regel- und Claude-Vorschläge zusammen; Claude-Vorschläge zu einer älteren Einstellung bleiben nur als Hinweis
  private async compose(ctx: SuggestionContext): Promise<SuggestionListDto> {
    const state = await this.state.read();
    const a = state.analysis;
    const stale = a !== null && a.configHash !== ctx.configHash;
    const claude = (a?.suggestions ?? []).map((s) => (stale ? { ...s, patch: null } : s));
    const all = OptimizationAdvisor.sort([...ctx.advice.suggestions, ...claude]);
    const hidden = new Set(state.dismissed);
    return {
      configHash: ctx.configHash, basis: ctx.facts.basis,
      suggestions: all.filter((s) => !hidden.has(s.id)), dismissed: all.filter((s) => hidden.has(s.id)),
      insufficient: ctx.advice.insufficient,
      analysis: a ? { at: a.at, costUsd: a.costUsd, stale, count: a.suggestions.length, skipped: a.skipped ?? 0 } : null,
    };
  }

  private requireId(id: string): void {
    if (!ID.test(id)) throw new HarnessError('input-invalid', `${id} ist keine gültige Vorschlags-ID.`);
  }
}
