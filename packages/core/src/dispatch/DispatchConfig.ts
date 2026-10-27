// Einstellung des Dispatchers (~/.harness/dispatch.yaml), vollständig aufgelöst.

import type { DispatchProfileReason } from '../board/DispatchRunTypes.js';
import type { PromptKind } from './PromptTemplateCheck.js';

export interface TimeWindow {
  /** HH:MM Ortszeit, einschließlich */
  from: string;
  /** HH:MM Ortszeit, ausschließlich; liegt `to` vor `from`, geht das Fenster über Mitternacht */
  to: string;
}

export interface ClaudeSettings {
  /** Aufruf von Claude Code – `claude` oder ein voller Pfad */
  command: string;
  /** Vorgabe für Profile ohne `model`; null = Einstellung von Claude Code */
  model: string | null;
  /** Vorgabe für Profile ohne `effort`; null = Standard des Modells */
  effort: string | null;
  maxTurns: number;
  timeoutMinutes: number;
  permissionMode: string | null;
  /** Was der Agent ohne Rückfrage darf (`--allowedTools`) */
  allowedTools: string[];
  /** true = API-Key-Variablen aus der Umgebung des Workers entfernen */
  useSubscription: boolean;
  extraArgs: string[];
  /** Pfad der bash.exe von Git for Windows; null = automatisch suchen (nur Windows) */
  gitBashPath: string | null;
}

export interface Profile {
  model: string | null;
  effort: string | null;
  maxTurns: number;
  timeoutMinutes: number;
}

export interface RuleCondition {
  /** Das Ticket trägt eines dieser Labels */
  labels?: string[];
  /** Die Priorität ist eine davon */
  priority?: string[];
  /** Mindestens so viele Zeilen in der Tabelle unter `## Integration` im Plan */
  filesMin?: number;
  /** Der Plan hat mindestens so viele Zeichen */
  planMinChars?: number;
}

export interface Rule {
  when: RuleCondition;
  profile: string;
}

/** Einstellungen eines Laufs samt Begründung für den Start-Kommentar */
export interface RunSettings extends Profile {
  profile: string;
  why: string;
  /** Basisprofil vor der Eskalation; fehlt ohne Eskalation */
  escalatedFrom?: string;
  /** Warum das Basisprofil gewählt wurde (vor einer Eskalation); fehlt bei Läufen älterer Stände */
  reason?: DispatchProfileReason;
  /** 1-basierte Nummer der Regel, die gegriffen hat – nur bei `reason: 'rule'` */
  ruleIndex?: number;
  /** Hash der Dispatcher-Einstellung, die beim Start galt; fehlt bei Läufen älterer Stände */
  configHash?: string;
}

/** Recherche-Läufe zu offenen Fragen */
export interface ResearchSettings {
  /** true = neue offene Fragen ohne Recherche, die den eigenen Benutzer betreffen, automatisch recherchieren lassen
   *  (je Frage einmal; nur Fragen, die nach dem Einschalten entstanden sind) */
  auto: boolean;
  /** Höchstens so viele Recherche-Läufe gleichzeitig (1–5, zusätzlich zum WIP der Tickets) */
  wip: number;
  /** Höchstens so viele automatische Recherchen je Kalendertag (Ortszeit) */
  maxPerDay: number;
  /** Profil der Recherche; null = Profil „research“, falls vorhanden, sonst defaultProfile */
  profile: string | null;
}

/** Planungsläufe an Tickets („Planen“ im Ticket-Panel) */
export interface PlanningSettings {
  /** true = Tickets mit Planungsbedarf auf eigenen Boards automatisch planen lassen, solange im Dispatcher Platz ist
   *  (je Ticket einmal; nur Tickets, die nach dem Einschalten dazugekommen sind) */
  auto: boolean;
  /** Höchstens so viele Planungsläufe gleichzeitig (1–5, zusätzlich zum WIP der Tickets) */
  wip: number;
  /** true = die Planung füllt auch die Abnahme des Tickets (woran man sieht, dass es fertig ist) */
  acceptance: boolean;
  /** Profil der Planung; null = Profil „planning“, falls vorhanden, sonst defaultProfile */
  profile: string | null;
}

/** Wie ein Ticket-Branch in den Branch des Hauptordners kommt */
export type MergeStrategy = 'merge' | 'squash';

/** Erlaubte Werte von mergeStrategy */
export const MERGE_STRATEGIES: readonly MergeStrategy[] = ['merge', 'squash'];

/** Einrichtungsbefehl für einen neu angelegten Worktree (z. B. npm ci); cwd relativ zum Worktree, „.“ = Worktree selbst */
export interface WorktreeSetupStep {
  cwd: string;
  run: string;
}

/** Worktrees des Dispatchers: ein Worktree je Ticket oder Tickets einer Spec in einem gemeinsamen Worktree (alt) */
export type JoinWorktrees = 'none' | 'sameSpec';

/** Erlaubte Werte von joinWorktrees */
export const JOIN_WORKTREES: readonly JoinWorktrees[] = ['none', 'sameSpec'];

/** Eigene Prompt-Vorlagen je Laufart; null oder leer = Standardvorlage */
export interface PromptTemplates {
  work: string | null;
  plan: string | null;
  research: string | null;
  merge: string | null;
}

/** Erlaubte Laufarten einer Vorlage, in der Reihenfolge der Oberfläche */
export const PROMPT_KINDS: readonly PromptKind[] = ['work', 'plan', 'research', 'merge'];

/** Befund zu einer eigenen Vorlage: unbekannte Platzhalter und fehlende Pflichtteile */
export interface PromptProblem {
  kind: PromptKind;
  unknownPlaceholders: string[];
  /** Beschriftungen der fehlenden Pflichtteile (PromptTemplateCheck) */
  missingParts: string[];
}

export interface DispatchConfig {
  /** false = nichts Neues starten (Läufe werden trotzdem ausgewertet) */
  enabled: boolean;
  /** Board-IDs; null = aktives Board */
  boards: number[] | null;
  wip: number;
  window: TimeWindow | null;
  optInLabel: string | null;
  optOutLabel: string | null;
  agentPrefix: string;
  dispatcherName: string;
  staleMinutes: number;
  maxAttempts: number;
  pauseMinutesOnLimit: number;
  promptExtra: string;
  /** Eigene Vorlagen je Laufart; null/leer = Standardvorlage */
  prompts: PromptTemplates;
  /** Zurückmergen der Ticket-Branches: Merge-Commit (alle Commits bleiben) oder Squash (ein Commit je Branch) */
  mergeStrategy: MergeStrategy;
  /** true = ein vom Lauf erledigtes Ticket wartet vor dem Merge auf Freigabe (Blockiert, REVIEW_REASON) */
  reviewBeforeMerge: boolean;
  /** none = jedes Ticket bekommt einen eigenen Worktree und wird einzeln gemergt; sameSpec = Tickets einer Spec teilen einen Worktree */
  joinWorktrees: JoinWorktrees;
  /** Befehle, die der Worker in einem neu angelegten Worktree vor Claude ausführt (nacheinander; leer = keine) */
  worktreeSetup: WorktreeSetupStep[];
  /** Zeitlimit je Einrichtungsbefehl in Minuten */
  worktreeSetupTimeoutMinutes: number;
  claude: ClaudeSettings;
  profiles: Record<string, Profile>;
  defaultProfile: string;
  rules: Rule[];
  escalate: boolean;
  /** Profilleiter von günstig nach teuer */
  ladder: string[];
  allowFable: boolean;
  research: ResearchSettings;
  planning: PlanningSettings;
}

/** Rohe Einstellung, wie sie in der Datei steht (alles optional) */
export interface RawDispatchConfig {
  enabled?: boolean;
  boards?: Array<number | string> | null;
  wip?: number;
  window?: { from?: string; to?: string } | null;
  optInLabel?: string | null;
  optOutLabel?: string | null;
  agentPrefix?: string;
  dispatcherName?: string;
  staleMinutes?: number | null;
  maxAttempts?: number;
  pauseMinutesOnLimit?: number;
  promptExtra?: string;
  prompts?: { work?: string | null; plan?: string | null; research?: string | null; merge?: string | null } | null;
  mergeStrategy?: MergeStrategy;
  reviewBeforeMerge?: boolean;
  joinWorktrees?: JoinWorktrees;
  worktreeSetup?: Array<{ cwd?: string; run?: string }> | null;
  worktreeSetupTimeoutMinutes?: number;
  claude?: Partial<ClaudeSettings>;
  profiles?: Record<string, Partial<Profile>>;
  defaultProfile?: string;
  rules?: Array<{ when?: RuleCondition; profile?: string }>;
  escalate?: boolean;
  escalation?: string[];
  allowFable?: boolean;
  research?: { auto?: boolean; wip?: number; maxPerDay?: number; profile?: string | null } | null;
  planning?: { auto?: boolean; wip?: number; acceptance?: boolean; profile?: string | null } | null;
}

export const DEFAULT_ALLOWED_TOOLS = [
  'Read', 'Edit', 'Write', 'Glob', 'Grep',
  // Board-CLI des Harness
  'Bash(harness:*)',
  // node deckt Tests und Skripte des Projekts ab
  'Bash(node:*)',
  'Bash(npm test:*)', 'Bash(npm run:*)',
  'Bash(git status:*)', 'Bash(git diff:*)', 'Bash(git log:*)', 'Bash(git branch:*)',
  'Bash(git add:*)', 'Bash(git commit:*)',
];

/** Zusätzlich erlaubt nur in der Merge-Session */
export const MERGE_SESSION_TOOLS = ['Bash(git merge:*)', 'Bash(git checkout:*)'];

/** Einzige erlaubte Tools einer Recherche: lesen, lesende Board-Befehle, das Schreiben genau einer Datei research.json im
 *  Arbeitsordner (Heredocs mit JSON lehnt Claude Code bei Präfix-Regeln ab), das Speichern der Recherche aus dieser Datei und
 *  lesende Git-Befehle; ersetzt claude.allowedTools. Kein `git grep` (`-O` startet ein Programm) – dafür gibt es Grep */
export const RESEARCH_TOOLS = [
  'Read', 'Glob', 'Grep', 'Write(./research.json)', 'Edit(./research.json)',
  'Bash(harness board question show:*)', 'Bash(harness board question research:*)',
  'Bash(harness board show:*)', 'Bash(harness board list:*)',
  'Bash(git log:*)', 'Bash(git show:*)', 'Bash(git diff:*)',
];

/** Für eine Recherche ausdrücklich verboten (`--disallowedTools`): Verbote schlagen Erlaubnisse, auch solche aus
 *  ~/.claude/settings.json und den Einstellungen des Repos */
export const RESEARCH_DENIED_TOOLS = [
  'MultiEdit', 'NotebookEdit',
  'Bash(git commit:*)', 'Bash(git push:*)', 'Bash(git checkout:*)', 'Bash(git reset:*)', 'Bash(git switch:*)',
  'Bash(git merge:*)', 'Bash(git rebase:*)',
  'Bash(npm:*)', 'Bash(node:*)',
  'Bash(harness board dispatch:*)', 'Bash(harness board question research-start:*)',
  'Bash(harness board question answer:*)', 'Bash(harness board question discard:*)',
  'Bash(git diff --output:*)', 'Bash(git log --output:*)', 'Bash(git show --output:*)',
];

/** Einzige erlaubte Tools einer Planung: lesen, lesende Board-Befehle, das Schreiben genau einer Datei plan.json im
 *  Arbeitsordner, das Speichern des Plans aus dieser Datei, eine Frage oder einen Kommentar am Ticket anlegen und lesende
 *  Git-Befehle; ersetzt claude.allowedTools */
export const PLANNING_TOOLS = [
  'Read', 'Glob', 'Grep', 'Write(./plan.json)', 'Edit(./plan.json)',
  'Bash(harness board show:*)', 'Bash(harness board list:*)', 'Bash(harness board plan-json:*)',
  'Bash(harness board comment:*)', 'Bash(harness board note list:*)',
  'Bash(harness board question show:*)', 'Bash(harness board question list:*)', 'Bash(harness board question add:*)',
  'Bash(git log:*)', 'Bash(git show:*)', 'Bash(git diff:*)',
];

/** Für eine Planung ausdrücklich verboten: die Verbote der Recherche, dazu alles, was Status, Zuständigkeit oder Felder des
 *  Tickets von Hand ändert (das erledigt plan-json) */
export const PLANNING_DENIED_TOOLS = [
  ...RESEARCH_DENIED_TOOLS,
  'Bash(harness board move:*)', 'Bash(harness board set:*)', 'Bash(harness board claim:*)', 'Bash(harness board start:*)',
  'Bash(harness board done:*)', 'Bash(harness board delete:*)', 'Bash(harness board plan-start:*)',
];

/** Rechte-Modus einer Recherche, unabhängig von claude.permissionMode: „default“ lässt ohne Rückfrage nur die freigegebenen
 *  Tools zu (`claude -p` kann nicht fragen, alles andere wird abgelehnt); „acceptEdits“ würde Dateiänderungen ohne Freigabe erlauben */
export const RESEARCH_PERMISSION_MODE = 'default';

/** Effort-Stufen von Claude Code; welche ein Modell kann, entscheidet Claude Code */
export const EFFORT_LEVELS = ['low', 'medium', 'high', 'xhigh', 'max'] as const;

export const MODEL_ALIASES = [
  'default', 'sonnet', 'opus', 'haiku', 'fable', 'best', 'opusplan',
  'sonnet[1m]', 'opus[1m]', 'opusplan[1m]', 'fable[1m]',
];

/** Anfänge der Dispatcher-Kommentare; Versuche zählen an ABORT */
export const DISPATCH_MARKERS = {
  START: '[dispatch] Start',
  ABORT: '[dispatch] Abbruch',
  PAUSE: '[dispatch] Pause',
  QUESTION: '[dispatch] Frage',
  MERGED: '[dispatch] Gemergt',
  MERGE_CONFLICT: '[dispatch] Merge-Konflikt',
  MERGE_WAIT: '[dispatch] Merge wartet',
  MERGE: '[dispatch] Merge',
  /** Einrichtung eines neuen Worktrees fehlgeschlagen (zählt nicht als Versuch) */
  SETUP: '[dispatch] Einrichtung',
  /** Kommentar an der Frage, wenn ein Recherche-Lauf keine Recherche hinterlassen hat */
  RESEARCH_EMPTY: '[dispatch] Recherche ohne Ergebnis',
  /** Kommentar am Ticket, wenn ein Planungslauf weder einen Plan noch eine Frage hinterlassen hat */
  PLAN_EMPTY: '[dispatch] Planung ohne Ergebnis',
  /** Ticket wartet nach dem Lauf auf Freigabe vor dem Merge */
  REVIEW: '[dispatch] Review',
  /** Mensch hat das Ticket aus dem Review zurück an den Agenten gegeben (zählt nicht als Versuch) */
  REVIEW_RETURN: '[dispatch] Zurück an den Agenten',
} as const;

/** Blockiert-Grund eines Tickets, das nach dem Lauf auf Freigabe vor dem Merge wartet */
export const REVIEW_REASON = 'Human Interaction: Review vor dem Merge';

/** Vorsilbe des Blockiert-Grunds, wenn die Einrichtung eines neuen Worktrees scheiterte */
export const SETUP_REASON_PREFIX = 'Einrichtung fehlgeschlagen:';

/** Vorsilbe des Blockiert-Grunds, wenn der Merge scheiterte („Erneut mergen“ im Panel) */
export const MERGE_REASON_PREFIX = 'Merge:';
