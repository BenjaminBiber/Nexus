import { HarnessError } from '@harness/core';

export interface MappedError {
  status: number;
  body: { code: string; message: string; details?: Record<string, unknown> };
}

// Board-Datenbank fehlt, ist nicht erreichbar oder hat das falsche Schema: Dienst nicht verfügbar
const UNAVAILABLE = new Set(['board-config-missing', 'board-config-broken', 'board-db-unreachable', 'board-db-overloaded', 'board-db-storage', 'board-schema-missing', 'board-schema-outdated', 'board-schema-newer']);

// Übersetzt Fehler in HTTP-Status und deutschsprachige Fehlermeldungen (ErrorDto).
export class ErrorMapper {
  // HarnessError → 404/409/422/503, alles andere → 500 ohne Interna
  map(error: unknown): MappedError {
    if (error instanceof HarnessError) {
      const details = error.details ? { details: error.details } : {};
      return { status: this.statusOf(error.code), body: { code: error.code, message: error.message, ...details } };
    }
    const status = (error as { statusCode?: number }).statusCode;
    if (status === 400) return { status, body: { code: 'bad-request', message: 'Die Anfrage ist ungültig.' } };
    return { status: 500, body: { code: 'internal', message: 'Interner Fehler im Server.' } };
  }

  // Status zum Fehlercode
  private statusOf(code: string): number {
    if (code.endsWith('-not-found') || code === 'not-found') return 404;
    if (code === 'token-invalid') return 403;
    if (code === 'conflict' || code === 'export-target-not-empty' || code === 'doc-changed') return 409;
    // Zeilenbereich passt nicht zum Dokument: die Anfrage selbst ist falsch gebaut
    if (code === 'doc-range-invalid') return 400;
    if (UNAVAILABLE.has(code)) return 503;
    return 422;
  }
}
