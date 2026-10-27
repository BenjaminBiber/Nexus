import type { RunSettings } from './DispatchConfig.js';
import { StreamEventFormatter } from './StreamEventFormatter.js';

/** Ergebnis eines Worker-Laufs */
export interface RunResult {
  /** Exit-Code von Claude; null, wenn der Prozess beendet wurde */
  code: number | null;
  timedOut: boolean;
  /** stdout (Ereignisse von --output-format stream-json, zuletzt das Ergebnis), notfalls samt stderr */
  output: string;
  /** Einrichtung des Worktrees: true = alle Befehle gelaufen (fehlt ohne Einrichtung) */
  setupOk?: boolean;
  /** Einrichtung gescheitert: dieser Befehl; Claude wurde dann nicht gestartet */
  setupFailed?: { cwd: string; run: string; code: number | null; timedOut: boolean; output: string };
}

export interface OutputSummary {
  subtype: string | null;
  isError: boolean;
  text: string;
  /** Von Claude Code abgelehnte Werkzeugaufrufe, z. B. „PowerShell: harness board show ARC-0010“ (höchstens 5, ohne Doppelte) */
  denied: string[];
}

/** Verbrauch eines Modells in einem Lauf */
export interface ModelUsage {
  inputTokens: number | null;
  outputTokens: number | null;
  cacheReadTokens: number | null;
  cacheWriteTokens: number | null;
  costUsd: number | null;
}

/** Verbrauch eines Laufs laut Ergebnis-Ereignis; fehlende Felder sind null */
export interface RunUsage extends ModelUsage {
  numTurns: number | null;
  durationMs: number | null;
  /** Hauptmodell: der Eintrag in modelUsage mit den höchsten Kosten */
  model: string | null;
  modelUsage: Record<string, ModelUsage>;
}

// Liest die Ausgabe von `claude -p --output-format stream-json` (früher `json`) und formuliert Abbruchgründe (Port aus dem alten Board).
export class RunOutputReader {
  private readonly events = new StreamEventFormatter();

  // Kosten, Tokens, Schritte, Dauer und Modelle aus dem letzten Ergebnis-Ereignis; null ohne Ergebnis
  usage(output: string): RunUsage | null {
    const result = this.resultObject(output);
    if (!result) return null;
    const num = (value: unknown) => (typeof value === 'number' && Number.isFinite(value) ? value : null);
    const usage = (result.usage ?? {}) as Record<string, unknown>;
    const models: Record<string, ModelUsage> = {};
    const raw = result.modelUsage;
    if (raw && typeof raw === 'object') {
      for (const [name, value] of Object.entries(raw as Record<string, Record<string, unknown>>)) {
        if (!value || typeof value !== 'object') continue;
        models[name] = {
          inputTokens: num(value.inputTokens), outputTokens: num(value.outputTokens), cacheReadTokens: num(value.cacheReadInputTokens),
          cacheWriteTokens: num(value.cacheCreationInputTokens), costUsd: num(value.costUSD),
        };
      }
    }
    const model = Object.entries(models).sort((a, b) => (b[1].costUsd ?? -1) - (a[1].costUsd ?? -1))[0]?.[0] ?? null;
    return {
      costUsd: num(result.total_cost_usd), inputTokens: num(usage.input_tokens), outputTokens: num(usage.output_tokens),
      cacheReadTokens: num(usage.cache_read_input_tokens), cacheWriteTokens: num(usage.cache_creation_input_tokens),
      numTurns: num(result.num_turns), durationMs: num(result.duration_ms), model, modelUsage: models,
    };
  }

  // Letztes Ergebnisobjekt (type: result) der Ausgabe oder null
  private resultObject(output: string): Record<string, unknown> | null {
    const text = output.trim();
    for (const candidate of [text, ...text.split(/\r?\n/).reverse()]) {
      if (!candidate.startsWith('{')) continue;
      try {
        const parsed = JSON.parse(candidate) as Record<string, unknown> | null;
        if (parsed?.type === 'result') return parsed;
      } catch {
        // kein JSON – weiter
      }
    }
    return null;
  }

  // Das Ergebnisobjekt (type: result), wenn es sich finden lässt; sonst der Text mit lesbaren statt roher Ereignisse
  summarize(output: string): OutputSummary {
    const text = output.trim();
    for (const candidate of [text, ...text.split(/\r?\n/).reverse()]) {
      if (!candidate.startsWith('{')) continue;
      try {
        const parsed = JSON.parse(candidate) as { type?: unknown; subtype?: unknown; is_error?: unknown; result?: unknown; permission_denials?: unknown };
        if (parsed?.type === 'result') {
          return {
            subtype: typeof parsed.subtype === 'string' ? parsed.subtype : null,
            isError: parsed.is_error === true,
            text: typeof parsed.result === 'string' ? parsed.result : '',
            denied: this.denials(parsed.permission_denials),
          };
        }
      } catch {
        // kein JSON – weiter
      }
    }
    return { subtype: null, isError: false, text: this.readable(text), denied: [] };
  }

  // Was von einem Lauf für einen Kommentar gezeigt wird: der Ergebnistext, sonst die Ausgabe mit lesbaren Ereignissen
  shown(output: string, summary: OutputSummary): string {
    return summary.text || this.readable(output);
  }

  // Ausgabe mit lesbaren Zeilen statt der Ereignisse von stream-json; Zeilen ohne JSON bleiben
  readable(output: string): string {
    return this.events.format(output.trim()).join('\n');
  }

  // Abgelehnte Aufrufe aus permission_denials als kurze Texte (höchstens 5, ohne Doppelte)
  private denials(value: unknown): string[] {
    if (!Array.isArray(value)) return [];
    const out: string[] = [];
    for (const item of value as Array<{ tool_name?: unknown; tool_input?: { command?: unknown } }>) {
      const tool = typeof item?.tool_name === 'string' ? item.tool_name : 'unbekannt';
      const command = item?.tool_input?.command;
      const text = typeof command === 'string' && command.trim() ? `${tool}: ${command.trim().slice(0, 120)}` : tool;
      if (!out.includes(text)) out.push(text);
      if (out.length === 5) break;
    }
    return out;
  }

  // Warum ein Lauf als abgebrochen gilt; `fallback` ersetzt den Grund für einen Lauf ohne Fehler (Standard: Ticket ohne Ausgang)
  abortReason(result: RunResult, summary: OutputSummary, run: RunSettings, fallback?: string): string {
    const base = this.baseReason(result, summary, run, fallback);
    return summary.denied.length
      ? `${base}; abgelehnte Werkzeugaufrufe: ${summary.denied.join(', ')} (erlaubte Tools in den Dispatcher-Einstellungen prüfen)`
      : base;
  }

  // Abbruchgrund ohne die abgelehnten Aufrufe
  private baseReason(result: RunResult, summary: OutputSummary, run: RunSettings, fallback?: string): string {
    if (result.timedOut) return `Zeitlimit von ${run.timeoutMinutes} min erreicht, Lauf abgebrochen`;
    if (summary.subtype === 'error_max_turns') return `Maximale Zahl an Schritten (${run.maxTurns}) erreicht`;
    if (result.code != null && result.code !== 0 || summary.isError) {
      const text = summary.text || this.tail(result.output);
      if (this.looksLikeContextOverflow(text)) return `Kontext voll: ${this.firstLine(text, 200)}`;
    }
    if (result.code != null && result.code !== 0) return `Claude endete mit Exit-Code ${result.code}`;
    if (summary.isError) return `Claude meldet einen Fehler (${summary.subtype ?? 'unbekannt'})`;
    return fallback ?? 'Lauf endete ohne Abschluss – das Ticket steht weder auf Erledigt noch auf Blockiert und hat keine offene Frage';
  }

  // Sieht der Text nach einem Nutzungslimit aus? Bewusst grob, den Wortlaut bestimmt Claude Code
  // (ältere Versionen: „usage limit reached“, neuere: „You've hit your session limit · resets …“, auch Wochen-/Modell-Limits)
  looksLikeUsageLimit(text: string): boolean {
    return /usage limit|limit reached|rate.?limit|hit (?:your|the)\b[^.\n]{0,30}\blimit/i.test(text);
  }

  // Sieht der Text nach einem vollen Kontextfenster aus? Bewusst grob, den Wortlaut bestimmt Claude Code
  // (u. a. „Prompt is too long“, „context length“, „context window“, „input is too long“, „exceeds the maximum context“)
  looksLikeContextOverflow(text: string): boolean {
    return /prompt is too long|context length|context window|input is too long|exceeds the maximum context/i.test(text);
  }

  // Erste Zeile eines Texts, auf max Zeichen gekürzt
  private firstLine(text: string, max: number): string {
    const line = text.trim().split(/\r?\n/, 1)[0] ?? '';
    return line.length > max ? `${line.slice(0, max)}…` : line;
  }

  // Endete der Lauf an einem Nutzungslimit? Nur bei fehlerhaftem Ende (Exit-Code ≠ 0 oder is_error, kein Zeitlimit) und nur
  // in stderr bzw. dem Fehlertext – der Ergebnistext eines erfolgreichen Laufs darf „rate limit“ enthalten
  hitUsageLimit(result: RunResult): boolean {
    if (result.timedOut) return false;
    const summary = this.summarize(result.output);
    if (!summary.isError && (result.code === null || result.code === 0)) return false;
    return this.looksLikeUsageLimit(this.errorText(result.output, summary));
  }

  // Wurde Claude Code nicht gefunden? (Exit-Code 127 des Workers bzw. Meldung „claude-not-found“)
  claudeMissing(result: RunResult): boolean {
    return result.code === 127 || /claude-not-found/.test(result.output);
  }

  // Fehlertext eines Laufs: alles außer den Ereignissen von stream-json (Claudes Text zählt nicht) und dem Ergebnisobjekt,
  // dazu dessen Text nur bei is_error
  private errorText(output: string, summary: OutputSummary): string {
    const text = output.trim();
    if (this.isResultObject(text)) return summary.isError ? summary.text : '';
    const rest = text.split(/\r?\n/).filter((line) => !this.isResultObject(line.trim()) && !this.events.isEvent(line));
    return [...rest, summary.isError ? summary.text : ''].join('\n');
  }

  // Ist der Text das JSON-Ergebnisobjekt (type: result)?
  private isResultObject(text: string): boolean {
    if (!text.startsWith('{')) return false;
    try {
      return (JSON.parse(text) as { type?: unknown })?.type === 'result';
    } catch {
      return false;
    }
  }

  // Ende der Ausgabe für einen Kommentar; Codezäune werden entschärft
  tail(text: string, max = 1500): string {
    const clean = text.replace(/```/g, "'''").trim();
    return clean.length > max ? `…${clean.slice(-max)}` : clean;
  }
}
