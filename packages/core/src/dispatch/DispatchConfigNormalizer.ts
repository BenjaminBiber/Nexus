import { HarnessError } from '../errors/HarnessError.js';
import {
  DEFAULT_ALLOWED_TOOLS, EFFORT_LEVELS, JOIN_WORKTREES, MERGE_STRATEGIES, MODEL_ALIASES, PROMPT_KINDS,
  type ClaudeSettings, type DispatchConfig, type JoinWorktrees, type MergeStrategy, type PromptProblem, type PromptTemplates, type WorktreeSetupStep, type Profile, type PlanningSettings, type RawDispatchConfig, type ResearchSettings, type Rule, type RuleCondition, type TimeWindow,
} from './DispatchConfig.js';
import { DISPATCH_PROMPT_PLACEHOLDERS } from './DispatchPromptDefaults.js';
import { PromptTemplate } from './PromptTemplate.js';
import { PromptTemplateCheck, type PromptKind } from './PromptTemplateCheck.js';

/** Obergrenze gleichzeitiger Recherche-Läufe */
const MAX_RESEARCH_WIP = 5;
/** Obergrenze gleichzeitiger Planungsläufe */
const MAX_PLANNING_WIP = 5;

const HHMM = /^([01]\d|2[0-3]):([0-5]\d)$/;

const CLAUDE_DEFAULTS: ClaudeSettings = {
  command: 'claude', model: 'sonnet', effort: null, maxTurns: 80, timeoutMinutes: 60, permissionMode: 'acceptEdits',
  allowedTools: DEFAULT_ALLOWED_TOOLS, useSubscription: true, extraArgs: [], gitBashPath: null,
};

// Prüft die rohe Einstellung und füllt sie mit Vorgaben auf (Port von normalizeConfig des alten Boards).
export class DispatchConfigNormalizer {
  private readonly template = new PromptTemplate();
  private readonly templateCheck = new PromptTemplateCheck();

  // Vollständig aufgelöste Einstellung oder HarnessError „dispatch-config-invalid“
  normalize(raw: RawDispatchConfig = {}): DispatchConfig {
    const input = (raw ?? {}) as RawDispatchConfig;
    const c = { ...CLAUDE_DEFAULTS, ...(input.claude ?? {}) };
    const claude: ClaudeSettings = {
      command: this.text(c.command) ?? 'claude',
      model: this.text(c.model),
      effort: this.text(c.effort),
      maxTurns: this.int(c.maxTurns, 'claude.maxTurns'),
      timeoutMinutes: this.int(c.timeoutMinutes, 'claude.timeoutMinutes'),
      permissionMode: this.text(c.permissionMode),
      allowedTools: Array.isArray(c.allowedTools) ? c.allowedTools.map(String).map((t) => t.trim()).filter(Boolean) : DEFAULT_ALLOWED_TOOLS,
      useSubscription: c.useSubscription !== false,
      extraArgs: Array.isArray(c.extraArgs) ? c.extraArgs.map(String) : [],
      gitBashPath: this.text(c.gitBashPath),
    };
    const allowFable = input.allowFable === true;
    if (claude.model) this.check(this.modelProblem(claude.model, allowFable), 'claude.model');
    if (claude.effort) this.check(this.effortProblem(claude.effort), 'claude.effort');

    const profiles = this.profiles(input, claude, allowFable);
    const names = Object.keys(profiles);
    const known = (name: string, where: string): string => {
      if (!profiles[name]) this.fail(`${where}: Profil „${name}“ gibt es nicht (${names.join(', ')}).`);
      return name;
    };
    const defaultProfile = known(this.text(input.defaultProfile) ?? (profiles.standard ? 'standard' : names[0]), 'defaultProfile');
    const ladder = Array.isArray(input.escalation) ? input.escalation.map((n, i) => known(String(n), `escalation[${i}]`)) : names;
    const rules = this.rules(input, known);
    const research = this.research(input.research, known);
    const planning = this.planning(input.planning, known);

    // Ein laufender Worker darf nie als verwaist gelten: Schwelle immer über dem längsten Zeitlimit
    const longest = Math.max(claude.timeoutMinutes, ...Object.values(profiles).map((p) => p.timeoutMinutes));
    const staleRaw = input.staleMinutes == null ? 0 : this.int(input.staleMinutes, 'staleMinutes');
    const staleMinutes = staleRaw >= longest + 10 ? staleRaw : longest + 30;

    return {
      enabled: input.enabled === true,
      boards: this.boards(input.boards),
      wip: this.int(input.wip ?? 1, 'wip', 0),
      window: this.window(input.window),
      optInLabel: input.optInLabel === undefined ? 'auto' : this.text(input.optInLabel),
      optOutLabel: input.optOutLabel === undefined ? 'manuell' : this.text(input.optOutLabel),
      agentPrefix: this.text(input.agentPrefix) ?? 'junior-',
      dispatcherName: this.text(input.dispatcherName) ?? 'dispatcher',
      staleMinutes,
      maxAttempts: this.int(input.maxAttempts ?? 2, 'maxAttempts'),
      pauseMinutesOnLimit: this.int(input.pauseMinutesOnLimit ?? 60, 'pauseMinutesOnLimit'),
      promptExtra: typeof input.promptExtra === 'string' ? input.promptExtra : '',
      prompts: this.prompts(input.prompts),
      mergeStrategy: this.mergeStrategy(input.mergeStrategy),
      reviewBeforeMerge: this.reviewBeforeMerge(input.reviewBeforeMerge),
      joinWorktrees: this.joinWorktrees(input.joinWorktrees),
      worktreeSetup: this.worktreeSetup(input.worktreeSetup),
      worktreeSetupTimeoutMinutes: this.int(input.worktreeSetupTimeoutMinutes ?? 15, 'worktreeSetupTimeoutMinutes'),
      claude,
      profiles,
      defaultProfile,
      rules,
      escalate: input.escalate !== false,
      ladder,
      allowFable,
      research,
      planning,
    };
  }

  // Beispiel-Einstellung wie `dispatch.js init` des alten Boards (Dispatcher aus, Opt-in `auto`)
  example(): RawDispatchConfig {
    return {
      enabled: false,
      boards: null,
      wip: 1,
      window: null,
      optInLabel: 'auto',
      optOutLabel: 'manuell',
      maxAttempts: 2,
      pauseMinutesOnLimit: 60,
      mergeStrategy: 'merge',
      reviewBeforeMerge: true,
      joinWorktrees: 'none',
      // Beispiel: [{ cwd: 'BackofficeNextGen', run: 'npm ci --prefer-offline --no-audit' }]
      worktreeSetup: [],
      worktreeSetupTimeoutMinutes: 15,
      profiles: {
        klein: { model: 'sonnet', effort: 'high', maxTurns: 40, timeoutMinutes: 45 },
        standard: { model: 'opus', effort: 'high', maxTurns: 80, timeoutMinutes: 90 },
        schwer: { model: 'opus', effort: 'max', maxTurns: 150, timeoutMinutes: 120 },
      },
      defaultProfile: 'standard',
      rules: [
        { when: { labels: ['doku', 'tippfehler', 'klein'] }, profile: 'klein' },
        { when: { labels: ['architektur', 'refactoring'] }, profile: 'schwer' },
        { when: { priority: ['urgent'] }, profile: 'schwer' },
        { when: { filesMin: 6 }, profile: 'schwer' },
      ],
      escalate: true,
      allowFable: false,
      research: { auto: false, wip: 1, maxPerDay: 10 },
      planning: { auto: false, wip: 1, acceptance: true, profile: null },
      promptExtra: '',
      // Eigene Vorlagen; null = die Standardvorlage der jeweiligen Laufart
      prompts: { work: null, plan: null, research: null, merge: null },
      claude: { command: 'claude', permissionMode: 'acceptEdits', useSubscription: true, allowedTools: DEFAULT_ALLOWED_TOOLS, extraArgs: [] },
    };
  }

  // Befunde zu den eigenen Vorlagen der rohen Einstellung; leer = alles in Ordnung.
  // Für das Speichern gedacht: normalize() verwirft eine fehlerhafte Vorlage still, write() lehnt sie ab.
  promptProblems(raw: RawDispatchConfig = {}): PromptProblem[] {
    const problems: PromptProblem[] = [];
    for (const [kind, text] of this.promptEntries(raw?.prompts)) {
      const problem = this.templateProblem(kind, text);
      if (problem) problems.push(problem);
    }
    return problems;
  }

  // Der Fehler zu Befunden an eigenen Vorlagen – derselbe beim Speichern wie in der Vorschau.
  // `details` nennt die erste betroffene Laufart samt fehlenden und unbekannten Teilen, damit die
  // Oberfläche sie anzeigen kann, ohne die Meldung zu zerlegen.
  static promptError(problems: PromptProblem[]): HarnessError {
    const first = problems[0];
    const message = problems.map((p) => {
      const teile: string[] = [];
      if (p.unknownPlaceholders.length) teile.push(`unbekannte Platzhalter ${p.unknownPlaceholders.map((n) => `{{${n}}}`).join(', ')}`);
      if (p.missingParts.length) teile.push(`fehlende Pflichtteile: ${p.missingParts.join('; ')}`);
      return `Vorlage „${p.kind}“: ${teile.join('; ')}.`;
    }).join('\n');
    return new HarnessError('dispatch-prompt-invalid', message, {
      kind: first.kind, missing: first.missingParts, unknown: first.unknownPlaceholders, problems,
    });
  }

  // Die angegebenen Vorlagen als Paare (Laufart, Text); leere und nicht gesetzte fallen weg.
  // Struktur-Fehler (kein Objekt, unbekannte Laufart) bleiben hart, nur der Inhalt einer Vorlage ist weich.
  private promptEntries(value: RawDispatchConfig['prompts']): Array<[PromptKind, string]> {
    if (value == null) return [];
    if (typeof value !== 'object' || Array.isArray(value)) this.fail('prompts muss ein Objekt sein (work, plan, research, merge).');
    const entries: Array<[PromptKind, string]> = [];
    for (const [key, text] of Object.entries(value as Record<string, unknown>)) {
      if (!(PROMPT_KINDS as readonly string[]).includes(key)) this.fail(`prompts: „${key}“ ist keine Laufart (${PROMPT_KINDS.join(', ')}).`);
      if (typeof text !== 'string' || !text.trim()) continue;
      // Zeilenenden vereinheitlichen, damit eine Datei von Windows den configHash nicht verschiebt
      entries.push([key as PromptKind, text.replace(/\r\n/g, '\n')]);
    }
    return entries;
  }

  // Eigene Vorlagen; was die Prüfung nicht besteht, wird null (= Standardvorlage)
  private prompts(value: RawDispatchConfig['prompts']): PromptTemplates {
    const prompts: PromptTemplates = { work: null, plan: null, research: null, merge: null };
    for (const [kind, text] of this.promptEntries(value)) {
      if (!this.templateProblem(kind, text)) prompts[kind] = text;
    }
    return prompts;
  }

  // Was an dieser Vorlage nicht stimmt – oder null
  private templateProblem(kind: PromptKind, text: string): PromptProblem | null {
    const allowed = DISPATCH_PROMPT_PLACEHOLDERS[kind].map((p) => p.name);
    const unknownPlaceholders = this.template.names(text).filter((name) => !allowed.includes(name));
    const missingParts = this.templateCheck.missing(kind, text).map((part) => part.label);
    if (!unknownPlaceholders.length && !missingParts.length) return null;
    return { kind, unknownPlaceholders, missingParts };
  }

  // Warum dieser Modellname nicht geht – oder null
  modelProblem(model: string, allowFable: boolean): string | null {
    const known = MODEL_ALIASES.includes(model) || /^claude-[a-z0-9.-]+(\[1m\])?$/.test(model);
    if (!known) return `Unbekanntes Modell „${model}“ (Alias wie sonnet/opus/haiku oder voller Name claude-…)`;
    if ((/fable/i.test(model) || model === 'best') && !allowFable) {
      return `Fable („${model}“) ist nicht freigegeben – im Headless-Modus kann es ohne Rückfrage über Usage-Credits abrechnen. Freigeben mit allowFable: true`;
    }
    return null;
  }

  // Warum diese Effort-Stufe nicht geht – oder null
  effortProblem(effort: string): string | null {
    return (EFFORT_LEVELS as readonly string[]).includes(effort) ? null : `Unbekannte effort-Stufe „${effort}“ (${EFFORT_LEVELS.join(', ')})`;
  }

  // Profile; ohne Angabe genau eines („standard“) aus den claude-Vorgaben
  private profiles(input: RawDispatchConfig, claude: ClaudeSettings, allowFable: boolean): Record<string, Profile> {
    const raw = input.profiles && typeof input.profiles === 'object' && Object.keys(input.profiles).length ? input.profiles : { standard: {} };
    const profiles: Record<string, Profile> = {};
    for (const [name, p] of Object.entries(raw)) {
      const where = `profiles.${name}`;
      const model = p?.model === undefined ? claude.model : this.text(p.model);
      const effort = p?.effort === undefined ? claude.effort : this.text(p.effort);
      if (model) this.check(this.modelProblem(model, allowFable), where);
      if (effort) this.check(this.effortProblem(effort), where);
      profiles[name] = {
        model,
        effort,
        maxTurns: p?.maxTurns == null ? claude.maxTurns : this.int(p.maxTurns, `${where}.maxTurns`),
        timeoutMinutes: p?.timeoutMinutes == null ? claude.timeoutMinutes : this.int(p.timeoutMinutes, `${where}.timeoutMinutes`),
      };
    }
    return profiles;
  }

  // Regeln; jede braucht mindestens eine Bedingung und ein bekanntes Profil
  private rules(input: RawDispatchConfig, known: (name: string, where: string) => string): Rule[] {
    const list = (v: unknown) => (Array.isArray(v) ? v.map(String) : []);
    return (Array.isArray(input.rules) ? input.rules : []).map((r, i) => {
      const where = `rules[${i}]`;
      const when = (r?.when ?? {}) as RuleCondition;
      const cond: RuleCondition = {};
      if (when.labels != null) cond.labels = list(when.labels);
      if (when.priority != null) cond.priority = list(when.priority);
      if (when.filesMin != null) cond.filesMin = this.int(when.filesMin, `${where}.when.filesMin`);
      if (when.planMinChars != null) cond.planMinChars = this.int(when.planMinChars, `${where}.when.planMinChars`);
      if (!Object.keys(cond).length) this.fail(`${where}: „when“ braucht mindestens eine Bedingung.`);
      return { when: cond, profile: known(String(r?.profile ?? ''), where) };
    });
  }

  // Recherchen: Automatik (Vorgabe aus), gleichzeitige Läufe (1–5, Vorgabe 1), automatische Läufe je Tag (Vorgabe 10),
  // Profil (muss es geben, wenn genannt)
  private research(value: RawDispatchConfig['research'], known: (name: string, where: string) => string): ResearchSettings {
    if (value != null && (typeof value !== 'object' || Array.isArray(value))) this.fail('research muss ein Objekt sein (auto, wip, profile).');
    const raw = value ?? {};
    const profile = this.text(raw.profile);
    return {
      auto: raw.auto === true,
      wip: this.int(raw.wip ?? 1, 'research.wip', 1, MAX_RESEARCH_WIP),
      maxPerDay: this.int(raw.maxPerDay ?? 10, 'research.maxPerDay'),
      profile: profile ? known(profile, 'research.profile') : null,
    };
  }

  // Planungen: Automatik (Vorgabe aus), gleichzeitige Läufe (1–5, Vorgabe 1), Abnahme mit füllen (Vorgabe an),
  // Profil (muss es geben, wenn genannt)
  private planning(value: RawDispatchConfig['planning'], known: (name: string, where: string) => string): PlanningSettings {
    if (value != null && (typeof value !== 'object' || Array.isArray(value))) this.fail('planning muss ein Objekt sein (auto, wip, acceptance, profile).');
    const raw = value ?? {};
    const profile = this.text(raw.profile);
    return {
      auto: raw.auto === true,
      wip: this.int(raw.wip ?? 1, 'planning.wip', 1, MAX_PLANNING_WIP),
      acceptance: raw.acceptance !== false,
      profile: profile ? known(profile, 'planning.profile') : null,
    };
  }

  // Board-IDs als Zahlen; leer oder fehlend = aktives Board
  private boards(value: RawDispatchConfig['boards']): number[] | null {
    if (value == null) return null;
    if (!Array.isArray(value)) this.fail('boards muss eine Liste von Board-IDs sein.');
    if (value.length === 0) return null;
    return value.map((v) => {
      const n = Number(v);
      if (!Number.isInteger(n) || n < 1) this.fail(`boards: „${String(v)}“ ist keine Board-ID.`);
      return n;
    });
  }

  // Zeitfenster HH:MM–HH:MM oder null
  private window(value: RawDispatchConfig['window']): TimeWindow | null {
    if (value == null) return null;
    const { from, to } = value;
    if (typeof from !== 'string' || typeof to !== 'string' || !HHMM.test(from) || !HHMM.test(to)) {
      this.fail('window braucht from und to im Format HH:MM, z. B. from: "18:00", to: "07:00".');
    }
    return { from: from as string, to: to as string };
  }

  // Merge-Strategie; fehlend = Merge-Commit
  private mergeStrategy(value: unknown): MergeStrategy {
    if (value == null) return 'merge';
    const text = typeof value === 'string' ? value.trim() : '';
    if (!(MERGE_STRATEGIES as readonly string[]).includes(text)) this.fail(`mergeStrategy muss ${MERGE_STRATEGIES.join(' oder ')} sein (nicht „${String(value)}“).`);
    return text as MergeStrategy;
  }

  // Freigabe vor dem Merge; fehlend = warten
  private reviewBeforeMerge(value: unknown): boolean {
    if (value == null) return true;
    if (typeof value !== 'boolean') this.fail(`reviewBeforeMerge muss true oder false sein (nicht „${String(value)}“).`);
    return value as boolean;
  }

  // Einrichtungsbefehle: run nicht leer, cwd relativ zum Worktree und darin (kein absoluter Pfad, kein „..“); fehlend = keine
  private worktreeSetup(value: unknown): WorktreeSetupStep[] {
    if (value == null) return [];
    if (!Array.isArray(value)) this.fail('worktreeSetup muss eine Liste von { cwd, run } sein.');
    return (value as unknown[]).map((raw, i) => {
      const where = `worktreeSetup[${i}]`;
      if (!raw || typeof raw !== 'object' || Array.isArray(raw)) this.fail(`${where} muss ein Objekt { cwd, run } sein.`);
      const step = raw as { cwd?: unknown; run?: unknown };
      const run = this.text(step.run);
      if (!run) this.fail(`${where}: run (Befehl) fehlt.`);
      if (step.cwd != null && typeof step.cwd !== 'string') this.fail(`${where}: cwd muss ein Pfad relativ zum Worktree sein.`);
      return { cwd: this.relativeDir((step.cwd as string | undefined) ?? '', where), run: run as string };
    });
  }

  // Ordner relativ zum Worktree mit „/“, ohne „./“ am Anfang und „/“ am Ende; leer = „.“; absolut oder mit „..“ → Fehler
  private relativeDir(value: string, where: string): string {
    const text = value.trim().replace(/\\/g, '/');
    if (/^([a-zA-Z]:|\/)/.test(text)) this.fail(`${where}: cwd „${value}“ ist absolut – erlaubt ist nur ein Pfad relativ zum Worktree.`);
    const parts = text.split('/').filter((p) => p !== '' && p !== '.');
    if (parts.includes('..')) this.fail(`${where}: cwd „${value}“ verlässt den Worktree („..“ ist nicht erlaubt).`);
    return parts.join('/') || '.';
  }

  // Worktrees bündeln; fehlend = ein Worktree je Ticket
  private joinWorktrees(value: unknown): JoinWorktrees {
    if (value == null) return 'none';
    const text = typeof value === 'string' ? value.trim() : '';
    if (!(JOIN_WORKTREES as readonly string[]).includes(text)) this.fail(`joinWorktrees muss ${JOIN_WORKTREES.join(' oder ')} sein (nicht „${String(value)}“).`);
    return text as JoinWorktrees;
  }

  // Ganze Zahl ab `min` (und höchstens `max`, wenn angegeben)
  private int(value: unknown, name: string, min = 1, max?: number): number {
    const n = Number(value);
    if (max !== undefined && !(Number.isInteger(n) && n >= min && n <= max)) this.fail(`${name} muss eine ganze Zahl von ${min} bis ${max} sein.`);
    if (!Number.isInteger(n) || n < min) this.fail(`${name} muss eine ganze Zahl ab ${min} sein.`);
    return n;
  }

  // Getrimmter Text oder null
  private text(value: unknown): string | null {
    return typeof value === 'string' && value.trim() ? value.trim() : null;
  }

  // Wirft bei einem Problem
  private check(problem: string | null, where: string): void {
    if (problem) this.fail(`${where}: ${problem}`);
  }

  // Prüffehler der Einstellung
  private fail(message: string): never {
    throw new HarnessError('dispatch-config-invalid', message);
  }
}
