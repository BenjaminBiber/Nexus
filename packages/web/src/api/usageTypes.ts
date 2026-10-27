/** Auslastung eines Limit-Fensters; `resetsAt` ist ein ISO-Zeitpunkt */
export interface UsageWindowDto {
  percent: number;
  resetsAt: string | null;
}

/** Antwort von GET /api/claude-usage; bei `available: false` erklärt `reason`, warum keine Werte da sind */
export interface ClaudeUsageDto {
  available: boolean;
  reason: 'disabled' | 'no-login' | 'token-expired' | 'request-failed' | null;
  plan: string | null;
  fiveHour: UsageWindowDto | null;
  sevenDay: UsageWindowDto | null;
  fetchedAt: string | null;
}
