import {
  AnalysisWorkspace, ClaudeInvocation, ConfigPatcher, DispatchConfigNormalizer, HarnessError, suggestionId,
  type ConfigPatchOp, type DispatchConfig, type HarnessHome, type RunResult, type Suggestion,
} from '@harness/core';
import { ClaudeResultEnvelope, excerpt } from './ClaudeResultEnvelope.js';
import type { ClaudeRunInput } from './ClaudeProcess.js';
import type { DispatchSuggestionService, SuggestionContext, SuggestionListDto } from './DispatchSuggestionService.js';

/** Startet Claude; in Tests ersetzt */
export interface ClaudeRunner {
  run(input: ClaudeRunInput): Promise<RunResult>;
}

const TIMEOUT_MS = 5 * 60_000;
const MAX_SUGGESTIONS = 10;
/** Die Analyse liest nur den Prompt – keine Shell, keine Dateien, kein Web */
const DENIED_TOOLS = ['Bash', 'Edit', 'Write', 'MultiEdit', 'NotebookEdit', 'WebFetch', 'WebSearch', 'Task'];

// Claude-Analyse der Optimierungs-Vorschläge: verdichtete Kennzahlen an `claude -p`, Antwort als JSON-Array prüfen und merken.
export class SuggestionAnalyzer {
  private running = false;
  private readonly invocation = new ClaudeInvocation();
  private readonly envelope = new ClaudeResultEnvelope();
  private readonly patcher = new ConfigPatcher();
  private readonly normalizer = new DispatchConfigNormalizer();
  private readonly workspace: AnalysisWorkspace;

  constructor(
    private readonly service: DispatchSuggestionService,
    private readonly claude: ClaudeRunner,
    home: HarnessHome,
    private readonly clock: () => Date = () => new Date(),
  ) {
    this.workspace = new AnalysisWorkspace(home);
  }

  // Startet eine Analyse (nur eine gleichzeitig) und liefert die neu berechnete Liste
  async analyze(): Promise<SuggestionListDto> {
    if (this.running) throw new HarnessError('conflict', 'Es läuft schon eine Analyse.', { reason: 'analysis-running' });
    this.running = true;
    try {
      const ctx = await this.service.context();
      const cfg = ctx.read.config;
      const cwd = await this.workspace.dir();
      const result = await this.claude.run({
        command: cfg.claude.command, args: this.args(cfg), prompt: this.prompt(ctx), cwd,
        env: this.invocation.env(process.env, cfg, null) as NodeJS.ProcessEnv, timeoutMs: TIMEOUT_MS, onOutput: () => {},
      });
      if (result.code === 127) throw new HarnessError('analysis-failed', `Claude-Befehl „${cfg.claude.command}“ nicht gefunden.`);
      if (result.timedOut) throw new HarnessError('analysis-failed', 'Claude hat die Analyse nicht innerhalb von 5 Minuten beendet.');
      if (result.code !== 0) throw new HarnessError('analysis-failed', `Claude endete mit Exit-Code ${result.code}: ${excerpt(result.output)}`);
      const { text, costUsd } = this.envelope.parse(result.output);
      const { suggestions, skipped } = this.suggestions(text, ctx);
      await this.service.saveAnalysis({ at: this.clock().toISOString(), configHash: ctx.configHash, costUsd, suggestions, skipped });
      return this.service.list();
    } finally {
      this.running = false;
    }
  }

  // Aufrufargumente: ein Durchgang, Antwort als JSON, keine schreibenden Werkzeuge
  private args(cfg: DispatchConfig): string[] {
    return [
      '-p', '--output-format', 'json', '--max-turns', '1',
      ...(cfg.claude.model ? ['--model', cfg.claude.model] : []),
      '--permission-mode', 'default',
      '--disallowedTools', ...DENIED_TOOLS,
    ];
  }

  // Arbeitsanweisung: Erklärung des Dispatchers, Antwortformat und die verdichteten Daten
  private prompt(ctx: SuggestionContext): string {
    const c = ctx.read.config;
    const data = {
      config: { profiles: c.profiles, defaultProfile: c.defaultProfile, rules: c.rules, ladder: c.ladder, escalate: c.escalate, maxAttempts: c.maxAttempts },
      facts: ctx.facts,
      existing: ctx.advice.suggestions.map((s) => ({ title: s.title, patch: s.patch })),
    };
    return [
      'Du berätst bei der Einstellung eines Dispatchers, der Tickets von Claude Code abarbeiten lässt. Jedes Ticket startet auf einem Profil',
      '(Modell, Effort, maxTurns = Schritte, timeoutMinutes); scheitert ein Lauf, eskaliert der Dispatcher entlang der Leiter (ladder) zum nächsten Profil.',
      'Regeln (rules) wählen das Startprofil; ohne passende Regel gilt defaultProfile.',
      '',
      'Werte die Kennzahlen unten aus und schlage höchstens 5 Verbesserungen vor, die über die bereits vorhandenen Vorschläge (existing) hinausgehen.',
      'Begründe jeden Vorschlag mit Zahlen aus den Daten. Gibt es nichts zu verbessern, antworte mit [].',
      '',
      'Antworte ausschließlich mit einem JSON-Array, ohne Markdown und ohne Werkzeuge. Jedes Element:',
      '{"title": string, "reason": string, "target": {"profile"?: string, "ruleIndex"?: number}, "patch": [{"path": string, "from": any, "to": any}] | null}',
      'Erlaubte Pfade: profiles.<name>.maxTurns, profiles.<name>.timeoutMinutes, profiles.<name>.model, profiles.<name>.effort, defaultProfile,',
      'rules.<Index ab 0>.profile, escalation (die ganze Leiter als Liste). "from" ist der aktuelle Wert aus config. Ohne sichere Änderung: "patch": null.',
      '',
      'Daten:',
      JSON.stringify(data, null, 2),
    ].join('\n');
  }

  // Antworttext → Vorschläge; ungültige Änderungen bleiben als Hinweis (patch null) erhalten, unbrauchbare Elemente werden gezählt
  private suggestions(text: string, ctx: SuggestionContext): { suggestions: Suggestion[]; skipped: number } {
    const start = text.indexOf('[');
    const end = text.lastIndexOf(']');
    let items: unknown;
    try {
      items = start >= 0 && end > start ? JSON.parse(text.slice(start, end + 1)) : null;
    } catch {
      items = null;
    }
    if (!Array.isArray(items)) throw new HarnessError('analysis-failed', `Antwort nicht lesbar: ${excerpt(text)}`);
    let skipped = Math.max(0, items.length - MAX_SUGGESTIONS);
    const suggestions = items.slice(0, MAX_SUGGESTIONS).flatMap((item): Suggestion[] => {
      const i = item as { title?: unknown; reason?: unknown; target?: { profile?: unknown; ruleIndex?: unknown }; patch?: unknown };
      if (typeof i.title !== 'string' || !i.title.trim()) {
        skipped += 1;
        return [];
      }
      const patch = this.validPatch(i.patch, ctx);
      const target = {
        ...(typeof i.target?.profile === 'string' ? { profile: i.target.profile } : {}),
        ...(typeof i.target?.ruleIndex === 'number' ? { ruleIndex: i.target.ruleIndex } : {}),
      };
      const fingerprint = `claude:${i.title}:${JSON.stringify(patch)}`;
      return [{
        id: suggestionId(fingerprint), fingerprint, source: 'claude', kind: 'claude', title: i.title.trim(),
        reason: typeof i.reason === 'string' ? i.reason : '', target, patch, weight: 0, evidence: null,
      }];
    });
    return { suggestions, skipped };
  }

  // Nur erlaubte Pfade mit stimmendem Ausgangswert, deren Ergebnis die Prüfung der Einstellung besteht; sonst null (Hinweis)
  private validPatch(value: unknown, ctx: SuggestionContext): ConfigPatchOp[] | null {
    if (!Array.isArray(value) || value.length === 0) return null;
    const ops = value as ConfigPatchOp[];
    if (!ops.every((op) => op && typeof op.path === 'string' && this.patcher.allowed(op.path))) return null;
    try {
      this.normalizer.normalize(this.patcher.apply(ctx.read.raw, ctx.read.config, ops));
      return ops.map((op) => ({ path: op.path, from: op.from, to: op.to }));
    } catch {
      return null;
    }
  }
}
