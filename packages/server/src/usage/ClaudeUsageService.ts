import { readFile } from 'node:fs/promises';
import { homedir } from 'node:os';
import { join } from 'node:path';

const USAGE_URL = 'https://api.anthropic.com/api/oauth/usage';
const REQUEST_TIMEOUT_MS = 10_000;
// Fehlschläge gelten nur kurz: erneuert Claude Code gleich danach sein Token, soll der nächste Aufruf es schon sehen
const FAILURE_TTL_MS = 5_000;

/** Auslastung eines Limit-Fensters; `resetsAt` ist ein ISO-Zeitpunkt */
export interface UsageWindowDto {
  percent: number;
  resetsAt: string | null;
}

/** Antwort von GET /api/claude-usage; ohne `available` erklärt `reason`, warum keine Werte da sind */
export interface ClaudeUsageDto {
  available: boolean;
  reason: 'disabled' | 'no-login' | 'token-expired' | 'request-failed' | null;
  plan: string | null;
  fiveHour: UsageWindowDto | null;
  sevenDay: UsageWindowDto | null;
  fetchedAt: string | null;
}

export type UsageFetch = (url: string, init: { headers: Record<string, string>; signal: AbortSignal }) => Promise<{ ok: boolean; status: number; json(): Promise<unknown> }>;

export interface ClaudeUsageOptions {
  /** Anmeldedatei von Claude Code (Standard: ~/.claude/.credentials.json bzw. CLAUDE_CONFIG_DIR) */
  credentialsFile?: string;
  fetch?: UsageFetch;
  clock?: () => Date;
  /** So lange gilt eine erfolgreiche Antwort (Standard 60 s); Fehlschläge nur 5 s; schützt den Endpunkt vor jedem Neuladen der Oberfläche */
  ttlMs?: number;
}

// Liest die Abo-Limits, die auch `/usage` in Claude Code zeigt (Sitzungsfenster von 5 Stunden, Woche), über den
// OAuth-Endpunkt von Anthropic. Der Endpunkt ist nicht dokumentiert und kann sich ändern; Fehler bleiben darum leise
// (`available: false`) und gelten nur kurz, damit ein Aussetzer die Anzeige nicht minutenlang leer lässt.
// Das Token wird nur gelesen und nie erneuert (das übernimmt Claude Code selbst), nie ausgegeben.
export class ClaudeUsageService {
  private readonly file: string;
  private readonly fetchFn: UsageFetch;
  private readonly clock: () => Date;
  private readonly ttlMs: number;
  private cached: { at: number; value: ClaudeUsageDto } | null = null;
  private inflight: Promise<ClaudeUsageDto> | null = null;

  constructor(options: ClaudeUsageOptions = {}) {
    this.file = options.credentialsFile ?? join(process.env.CLAUDE_CONFIG_DIR || join(homedir(), '.claude'), '.credentials.json');
    this.fetchFn = options.fetch ?? ((url, init) => fetch(url, init));
    this.clock = options.clock ?? (() => new Date());
    this.ttlMs = options.ttlMs ?? 60_000;
  }

  // Aktuelle Werte (aus dem Zwischenspeicher, solange er frisch ist)
  async current(): Promise<ClaudeUsageDto> {
    const now = this.clock().getTime();
    if (this.cached && now - this.cached.at < (this.cached.value.available ? this.ttlMs : FAILURE_TTL_MS)) return this.cached.value;
    this.inflight ??= this.load().finally(() => {
      this.inflight = null;
    });
    const value = await this.inflight;
    // Erst nach dem Laden stempeln: sonst zählt die Laufzeit der Anfrage (bis zum Timeout) gegen die Gültigkeit
    this.cached = { at: this.clock().getTime(), value };
    return value;
  }

  private async load(): Promise<ClaudeUsageDto> {
    const login = await this.readLogin();
    if (!login) return this.unavailable('no-login');
    if (login.expiresAt !== null && login.expiresAt <= this.clock().getTime()) return this.unavailable('token-expired', login.plan);
    try {
      const reply = await this.fetchFn(USAGE_URL, {
        headers: { Authorization: `Bearer ${login.token}`, 'anthropic-beta': 'oauth-2025-04-20', 'User-Agent': 'ai-harness' },
        signal: AbortSignal.timeout(REQUEST_TIMEOUT_MS),
      });
      if (!reply.ok) return this.unavailable(reply.status === 401 ? 'token-expired' : 'request-failed', login.plan);
      const body = (await reply.json()) as Record<string, unknown> | null;
      return {
        available: true,
        reason: null,
        plan: login.plan,
        fiveHour: this.window(body?.five_hour),
        sevenDay: this.window(body?.seven_day),
        fetchedAt: this.clock().toISOString(),
      };
    } catch {
      return this.unavailable('request-failed', login.plan);
    }
  }

  // Token, Ablauf (ms) und Abo-Art aus der Anmeldedatei; null, wenn es keine Anmeldung gibt
  private async readLogin(): Promise<{ token: string; expiresAt: number | null; plan: string | null } | null> {
    try {
      const oauth = (JSON.parse(await readFile(this.file, 'utf8')) as { claudeAiOauth?: Record<string, unknown> }).claudeAiOauth;
      if (!oauth || typeof oauth.accessToken !== 'string' || !oauth.accessToken) return null;
      return {
        token: oauth.accessToken,
        expiresAt: typeof oauth.expiresAt === 'number' ? oauth.expiresAt : null,
        plan: typeof oauth.subscriptionType === 'string' ? oauth.subscriptionType : null,
      };
    } catch {
      return null;
    }
  }

  private window(raw: unknown): UsageWindowDto | null {
    const w = raw as { utilization?: unknown; resets_at?: unknown } | null | undefined;
    if (!w || typeof w.utilization !== 'number' || !Number.isFinite(w.utilization)) return null;
    return { percent: Math.max(0, Math.round(w.utilization)), resetsAt: typeof w.resets_at === 'string' ? w.resets_at : null };
  }

  private unavailable(reason: NonNullable<ClaudeUsageDto['reason']>, plan: string | null = null): ClaudeUsageDto {
    return { available: false, reason, plan, fiveHour: null, sevenDay: null, fetchedAt: null };
  }
}
