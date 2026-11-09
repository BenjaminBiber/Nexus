import type { DbColumnId, DbStatus } from './dbTypes.js';
import type { WorkflowPhase } from './generated/workflow.js';
import type { SessionClient } from './sessionDetailTypes.js';
import type { ThemeManifest } from './generated/themes.js';
// Antwortformate des Servers

export interface ProjectDto {
  id: string;
  name: string;
  path: string;
  reachable: boolean;
}

export type SessionDisplayState = 'running' | 'waiting' | 'idle' | 'ended' | 'stale';

export interface SubagentDto {
  id: string;
  type: string;
  state: 'running' | 'ended';
  startedAt: string;
  endedAt: string | null;
}

export interface TicketInfoDto {
  id: string;
  title: string;
  status: string;
}

export interface SessionDto {
  id: string;
  projectId: string;
  tickets: string[];
  ticketInfo: TicketInfoDto[];
  cwd: string;
  state: 'idle' | 'running' | 'waiting' | 'ended';
  displayState: SessionDisplayState;
  listening: boolean;
  startedAt: string;
  lastEventAt: string;
  endedAt: string | null;
  runtimeMs: number;
  title: string | null;
  model: string | null;
  subagents: SubagentDto[];
  /** Quelle der Session (Nexus-Lauf, Claude Code, Desktop-App) */
  client: SessionClient;
  /** Name des Lauf-Protokolls bei Nexus-Läufen, sonst null */
  runName: string | null;
}

export interface DocFileDto {
  path: string;
  title: string;
  group: 'spec' | 'plan' | 'glossary' | 'adr';
  /** Sorte des Superpowers-Dokuments; bei Glossar und ADRs null */
  kind: 'spec' | 'plan' | null;
  topic: string | null;
  date: string | null;
  mtimeMs: number;
  exists: boolean;
  /** Zur Datei sind Ticket-Entwürfe gespeichert */
  hasDrafts?: boolean;
  /** Nur im Client gesetzt (Board-Umschalter): Board, aus dessen Repo die Datei stammt, wenn es nicht das des Projektordners ist */
  board?: number;
  /** Nur im Client gesetzt: Name des Boards, das die Datei liefert (für die Marke im Explorer) */
  boardName?: string;
}

// Warum es keine Docs-Quelle gibt
export type DocsSourceReason = 'no-board' | 'no-repo-path' | 'missing-folder';

// Repo des Boards, aus dem die Dokumente kommen; ohne `root` sagt `reason`, was fehlt
export interface DocsSource {
  root: string | null;
  boardId: number | null;
  boardName: string | null;
  reason: DocsSourceReason | null;
}

export interface DocsListDto {
  source: DocsSource;
  docs: DocFileDto[];
}

// Abschnitt eines Markdown-Dokuments; Zeilennummern sind 1-basiert und einschließlich
export interface MarkdownSectionDto {
  heading: string;
  level: number;
  startLine: number;
  endLine: number;
}

export interface DocFileContentDto {
  path: string;
  content: string;
  hash: string;
  sections: MarkdownSectionDto[];
}

/** Markierte Stelle, aus der Ticket-Entwürfe entstehen sollen */
export interface TicketDraftSelectionInput {
  quote?: string;
  heading?: string;
  startLine?: number;
  endLine?: number;
}

export interface TicketDraftInput {
  path: string;
  selections: TicketDraftSelectionInput[];
  /** true zerlegt das ganze Dokument ohne Markierungen */
  whole?: boolean;
  note?: string;
}

/** Vorschlag für ein Ticket; `dependsOn` sind 0-basierte Indizes in dieselbe Liste */
export interface TicketDraftDto {
  title: string;
  description: string;
  acceptance: string;
  steps: string[];
  dependsOn: number[];
  labels: string[];
  source: { heading: string; startLine: number; endLine: number };
}

export interface TicketDraftResultDto {
  drafts: TicketDraftDto[];
  /** kurze Hinweise auf Verworfenes */
  dropped: string[];
  costUsd: number | null;
  at: string;
}

/** Geprüfte Entwürfe eines Dokuments als Tickets anlegen */
export interface DocTicketInput {
  path: string;
  drafts: TicketDraftDto[];
}

/** Übersprungener Entwurf; bei einer Dublette zeigt `existingId` auf das vorhandene Ticket */
export interface SkippedDocTicketDto {
  index: number;
  reason: string;
  existingId?: string;
}

/** Ticket, das aus einer Stelle des Dokuments entstanden ist; ohne Quellenzeile ist `heading` null */
export interface DocTicketRefDto {
  id: string;
  title: string;
  status: DbStatus;
  column: DbColumnId;
  heading: string | null;
  startLine: number;
  endLine: number;
}

export interface DocTicketResultDto {
  /** angelegte Tickets mit dem Platz des Entwurfs in der Eingabe */
  created: { index: number; id: string }[];
  skipped: SkippedDocTicketDto[];
}

/** Vorschlag für eine Board-Frage aus einer offenen Stelle des Dokuments */
export interface QuestionDraftDto {
  title: string;
  description: string;
  source: { heading: string; startLine: number; endLine: number };
}

export interface QuestionDraftResultDto {
  drafts: QuestionDraftDto[];
  /** kurze Hinweise auf Verworfenes */
  dropped: string[];
  costUsd: number | null;
  at: string;
}

/** Entwurf beim Anlegen; mit `answer` wird die Frage gleich entschieden */
export type QuestionDraftInputDto = QuestionDraftDto & { answer?: string };

export interface DocQuestionResultDto {
  /** angelegte Fragen mit dem Platz des Entwurfs in der Eingabe; `answered` = gleich entschieden, sonst offen */
  created: { index: number; id: string; answered: boolean; answerError?: string }[];
  skipped: { index: number; reason: string }[];
}

// Abschnitt ersetzen; `expectedHash` ist der Hash aus dem letzten Lesen und schützt vor fremden Änderungen
export interface WriteDocSectionInput {
  path: string;
  startLine: number;
  endLine: number;
  content: string;
  expectedHash: string;
}

export interface DocWriteResultDto {
  path: string;
  hash: string;
  sections: MarkdownSectionDto[];
}

// Bereich von Claude umschreiben lassen; der Server schreibt nichts, das Übernehmen läuft über writeDocSection
export interface ReviseDocInput {
  path: string;
  startLine: number;
  endLine: number;
  instruction: string;
  expectedHash: string;
}

export interface ReviseDocResultDto {
  original: string;
  proposal: string;
  startLine: number;
  endLine: number;
  /** unveränderter Hash des Dokuments; geht so an writeDocSection weiter */
  hash: string;
}

export interface SearchHitDto {
  path: string;
  line: number;
  excerpt: string;
  /** Nur im Client gesetzt: Board des Treffers, wenn es nicht das des Projektordners ist */
  board?: number;
}

// Kommentar zu einer markierten Stelle eines Dokuments
export interface DocCommentDto {
  id: string;
  status: 'open' | 'resolved';
  text: string;
  createdAt: string;
  quote: string;
  heading: string | null;
  lines: [number, number] | null;
  resolution: string | null;
}

// Angaben zum Anlegen eines Kommentars
export interface NewDocCommentInput {
  text: string;
  quote: string;
  heading: string | null;
  lines: [number, number] | null;
}

// Änderung an einem Kommentar
export interface DocCommentPatch {
  text?: string;
  status?: 'open' | 'resolved';
  resolution?: string | null;
}

export interface FavoriteDto {
  sessionId: string;
  note: string;
  tickets: string[];
  ticketInfo: TicketInfoDto[];
  cwd: string;
  cwdExists: boolean;
  title: string | null;
  markedAt: string;
  state: SessionDto['state'];
  displayState: SessionDisplayState;
  live: boolean;
  lastEventAt: string;
  resumedAs: string[];
  command: string;
}

export interface SettingsDto { project: { id: string; name: string; path: string }; token: string }

export type NotificationMode = 'off' | 'browser' | 'windows';

/** Eingebaute Themes in fester Reihenfolge (wie `BUILTIN_THEME_IDS` in Rust) */
export const BUILTIN_THEME_IDS = ['system', 'light', 'dark'] as const;
export type BuiltinThemeId = (typeof BUILTIN_THEME_IDS)[number];

/** Gewähltes Theme als ID: eingebaut oder installiert (`~/.nexus/themes/<id>/`); eine unbekannte ID gilt als „system“ */
export type ThemeMode = string;

/** CSS-Schnipsel des Nutzers (GET/PUT /api/snippets) */
export interface SnippetDto {
  id: string;
  name: string;
  css: string;
  enabled: boolean;
  order: number;
  updatedAt: string;
}

/** Vom Nutzer eingetragenes Store-Repo (GET/POST /api/themes/store/repos) */
export interface StoreRepoDto {
  id: string;
  url: string;
  addedAt: string;
}

/** Eintrag im Store-Index mit seinem Repo (GET /api/themes/store/index) */
export interface StoreIndexEntryDto {
  repoId: string;
  repoUrl: string;
  id: string;
  name: string;
  version: string;
  sdkVersion: string;
  downloadUrl: string;
  sha256: string;
  preview?: string;
  description?: string;
  signature?: string;
  compatible: boolean;
}

/** Ladeergebnis je Repo; bei Fehler steht die Meldung des Servers in error */
export interface StoreIndexRepoDto {
  id: string;
  url: string;
  ok: boolean;
  entryCount: number;
  skipped: unknown[];
  error?: { code: string; message: string };
}

export interface StoreIndexDto {
  entries: StoreIndexEntryDto[];
  repos: StoreIndexRepoDto[];
}

/** Antwort von POST /api/themes/store/install (bei update zusätzlich previousVersion) */
export interface StoreInstallResultDto {
  manifest: ThemeManifest;
  verified: boolean;
  repoId: string;
  sha256: string;
  previousVersion?: string;
}

export interface StoreUpdateDto {
  id: string;
  name: string;
  repoId: string;
  installedVersion: string;
  availableVersion: string;
}

/** GET /api/themes/store/updates */
export interface StoreUpdatesDto {
  updates: StoreUpdateDto[];
  errors: Array<{ repoId: string; code: string; message: string }>;
}

/** Setup-Dialoge, die nicht mehr kommen sollen („Nicht mehr fragen“) */
export interface SetupPromptsDto {
  pluginDismissed: boolean;
  migrationDismissed: number[];
}

export interface UserSettingsDto {
  notifications: NotificationMode;
  heroImages: boolean;
  theme: ThemeMode;
  claudeUsage: boolean;
  compactCards: boolean;
  hideNotNeeded: boolean;
  hideInPlanning: boolean;
  ntfy: boolean;
  ntfyServer: string;
  ntfyTopic: string;
  ntfyToken: string;
  setupPrompts: SetupPromptsDto;
}

export type PluginState = 'installed' | 'missing' | 'outdated';

/** Zustand des Claude-Code-Plugins (`GET /api/setup/plugin`) */
export interface PluginSetupDto {
  bundled: boolean;
  claude: boolean;
  state: PluginState;
  installedVersion: string | null;
  bundledVersion: string | null;
}

/** Ergebnis der Installation (`POST /api/setup/plugin`) */
export interface PluginInstallDto {
  ok: boolean;
  installed: boolean;
  steps: string[];
  hints: string[];
}

export type MigrationReason = 'legacy-plugin' | 'legacy-rules';

/** Braucht das Repo des Boards eine Migration? (`GET /api/db/boards/{id}/migration`) */
export interface BoardMigrationDto {
  needed: boolean;
  repoPath: string | null;
  reasons: MigrationReason[];
}

/** Ergebnis der Migration (`POST /api/db/boards/{id}/migration`) */
export interface BoardMigrationResultDto {
  written: string[];
  removed: string[];
  kept: string[];
  pluginSwitched: boolean;
  importApproval: 'approved' | 'unchanged' | 'no-config';
}

export interface TestNotificationDto {
  ok: boolean;
  error: string | null;
}

// Anbieter der Board-Datenbank
export type BoardProvider = 'mssql' | 'sqlite';
export interface BoardConfigDto {
  configured: boolean; file: string; provider: BoardProvider; dbFile: string;
  server: string; port: number; database: string; user: string; hasPassword: boolean;
  encrypt: boolean; trustServerCertificate: boolean; userId: string; userName: string; activeBoard: number | null;
}
export interface BoardConfigInput {
  provider: BoardProvider; dbFile: string;
  server: string; port: number; database: string; user: string; password?: string;
  encrypt: boolean; trustServerCertificate: boolean; userId: string; userName: string;
}
export interface BoardConfigTestDto { ok: boolean; message: string }

// Angewendeter Workflow der Anleitung: Standard oder eigene Anpassung
export interface WorkflowDto {
  source: 'standard' | 'custom';
  phases: WorkflowPhase[];
}

// Problem oder Warnung zu einer Anpassung, mit Fundstelle im Clone
export interface CustomizationProblemDto {
  code: string;
  message: string;
  file?: string;
  line?: number;
}

// Eine Anpassung eines Erweiterungspunkts mit Stand und Rückfall
export interface CustomizationEntryDto {
  point: 'workflow' | 'skill' | 'output-style';
  name: string;
  kind: 'replace' | 'new';
  state: 'active' | 'fallback';
  base: 'current' | 'outdated' | 'unknown' | 'none';
  problems: CustomizationProblemDto[];
  warnings: CustomizationProblemDto[];
}

// Übersicht der Anpassungen nach dem letzten Anwenden (ohne Unterschiede)
export interface CustomizationOverviewDto {
  version: 1;
  createdAt: string;
  cloneDir: string;
  customDir: string;
  harnessVersion: string;
  workflow: 'standard' | 'custom' | 'fallback';
  entries: CustomizationEntryDto[];
}

// Übersicht oder „unbekannt“, solange setup noch nie gelaufen ist
export type CustomizationsDto = CustomizationOverviewDto | { state: 'unknown' };
