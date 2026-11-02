/** Auslastung eines Limit-Fensters; `resetsAt` ist ein ISO-Zeitpunkt */
export interface UsageWindowDto {
  percent: number;
  resetsAt: string | null;
}

/** Antwort von GET /api/claude-usage; bei `available: false` erklärt `reason`, warum keine Werte da sind */
export interface ClaudeUsageDto {
  available: boolean;
  reason: 'disabled' | 'no-login' | 'token-expired' | 'rate-limited' | 'request-failed' | null;
  plan: string | null;
  fiveHour: UsageWindowDto | null;
  sevenDay: UsageWindowDto | null;
  fetchedAt: string | null;
}

/** Limits eines aktiven Accounts für die Ringe in der Seitenleiste */
export interface AccountUsageEntryDto {
  id: string;
  name: string;
  fallback: boolean;
  /** Eigene Obergrenzen in Prozent; null = Vorgabe des Dispatchers */
  limitFiveHour: number | null;
  limitWeek: number | null;
  usage: ClaudeUsageDto;
}

/** Antwort von GET /api/claude-usage/accounts; `enabled: false` = Einstellung aus, dann ohne Accounts */
export interface UsageOverviewDto {
  enabled: boolean;
  /** Vorgabe des Dispatchers, ab der ein Account als voll gilt (Prozent) */
  defaultLimit: number;
  accounts: AccountUsageEntryDto[];
}

/** Claude-Account mit Anmeldung, Plugin und Limits (GET /api/accounts) */
export interface AccountDto {
  id: string;
  name: string;
  /** Ordner wie eingegeben; null = Standard (Umgebung des Servers) */
  configDir: string | null;
  effectiveDir: string;
  enabled: boolean;
  /** Nur als Fallback, wenn kein anderer erlaubter Account frei ist */
  fallback: boolean;
  /** Eigene Obergrenzen in Prozent; null = Vorgabe des Dispatchers */
  limitFiveHour: number | null;
  limitWeek: number | null;
  loggedIn: boolean;
  email: string | null;
  plugin: boolean;
  usage: ClaudeUsageDto;
  pausedUntil: string | null;
  /** Stellen in der Dispatcher-Einstellung, die den Account nennen */
  references: string[];
  /** Laufende oder letzte Anmeldung über `claude auth login` */
  login: AccountLoginDto | null;
}

/** Stand einer Anmeldung */
export interface AccountLoginDto {
  state: 'running' | 'done' | 'failed' | 'cancelled';
  startedAt: string;
  /** Anmeldeseite, falls sich kein Browser geöffnet hat */
  url: string | null;
  output: string;
  /** Ergebnis der Plugin-Einrichtung nach der Anmeldung */
  plugin: string | null;
}

export interface AccountListDto {
  accounts: AccountDto[];
  multi: boolean;
  accountsDir: string;
  suggestions: string[];
  /** Befehle zum Einrichten des Nexus-Plugins in einem Account-Ordner */
  pluginCommands: string[];
}
