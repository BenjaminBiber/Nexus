import { HarnessError } from '@harness/core';
import { ServerLauncher } from './ServerLauncher.js';

// Ein laufender Worker; bei einer Recherche steht die Frage in ticketId und questionId
export interface DispatchRunView {
  id: string;
  kind: 'work' | 'merge' | 'research' | 'plan';
  ticketId: string;
  questionId?: string;
  title: string;
  profile: string;
  model: string | null;
  effort: string | null;
  startedAt: string;
  log: string;
}

// Antwort von /api/dispatch/status (wie DispatchStatus im Server)
export interface DispatchStatusView {
  enabled: boolean;
  state: 'off' | 'idle' | 'running' | 'paused' | 'outside' | 'error';
  message: string;
  pausedUntil: string | null;
  wip: number;
  active: number;
  runs: DispatchRunView[];
  /** Recherchen (ältere Server liefern das Feld nicht) */
  research?: { auto: boolean; wip: number; active: number; maxPerDay?: number; today?: number; retryAt?: string | null; error?: string | null };
  /** Planungs-Automatik (ältere Server liefern das Feld nicht) */
  planningAuto?: { auto: boolean; wip: number; active: number; retryAt?: string | null; error?: string | null };
  /** target: Zielbranch des Tickets (null ohne Zielbranch; ältere Server liefern das Feld nicht) */
  merges: Array<{ ticketId: string; phase: 'pending' | 'waiting' | 'session'; branch: string; target?: string | null }>;
  boardId: number | null;
  boardInScope: boolean;
  tickets: Array<{ id: string; title: string; reason: string }>;
  ticketsError: string | null;
  lastTick: string | null;
  lastErrors: string[];
  configFile: string;
  configError: string | null;
  agentPrefix: string;
}

// Antwort von „Erneut mergen“: Zustand plus Ergebnis für das Ticket (ältere Server liefern kein result)
export interface DispatchMergeView extends DispatchStatusView {
  result?: { outcome: 'merged' | 'blocked' | 'waiting' | 'session' | 'pending' | 'accepted'; text: string };
}

// Antwort von „Recherche starten“: Zustand plus der gestartete Lauf
export interface DispatchResearchView extends DispatchStatusView {
  started: DispatchRunView;
}

// Spricht die Dispatcher-Routen des lokalen Servers an; startet den Server bei Bedarf.
export class DispatchClient {
  constructor(private readonly port: number, private readonly launcher: ServerLauncher = new ServerLauncher()) {}

  // Zustand, mit Board die Bereitschaft seiner Geplant-Tickets (ohne Board: aktives Board)
  status(boardId?: number): Promise<DispatchStatusView> {
    return this.request('GET', `/api/dispatch/status${boardId ? `?boardId=${boardId}` : ''}`);
  }

  // Sofort einen Tick ausführen
  run(boardId?: number): Promise<DispatchStatusView> {
    return this.request('POST', '/api/dispatch/run', boardId ? { boardId } : {});
  }

  // Neue Starts pausieren
  pause(minutes: number): Promise<DispatchStatusView> {
    return this.request('POST', '/api/dispatch/pause', { minutes });
  }

  // Pause aufheben
  resume(): Promise<DispatchStatusView> {
    return this.request('POST', '/api/dispatch/resume', {});
  }

  // Ein- oder ausschalten
  setEnabled(on: boolean): Promise<DispatchStatusView> {
    return this.request('POST', `/api/dispatch/${on ? 'enable' : 'disable'}`, {});
  }

  // „Erneut mergen“; liefert Zustand und Ergebnis für das Ticket
  merge(id: string): Promise<DispatchMergeView> {
    return this.request('POST', `/api/dispatch/merge/${encodeURIComponent(id)}`, {});
  }

  // „Abnehmen“ eines Human-Interaction-Tickets; liefert Zustand und Ergebnis (Merge oder nur erledigt)
  accept(id: string, by: string): Promise<DispatchMergeView> {
    return this.request('POST', `/api/dispatch/accept/${encodeURIComponent(id)}`, { by });
  }

  // „Recherche starten“ für eine offene Frage (sofort, unabhängig von Ein/Aus und Zeitfenster)
  startResearch(questionId: string): Promise<DispatchResearchView> {
    return this.request('POST', `/api/dispatch/research/${encodeURIComponent(questionId)}`, {});
  }

  // „Planen“ für ein Ticket (sofort, unabhängig von Ein/Aus und Zeitfenster)
  startPlanning(ticketId: string): Promise<DispatchResearchView> {
    return this.request('POST', `/api/dispatch/plan/${encodeURIComponent(ticketId)}`, {});
  }

  // Anfrage mit Pflichtkopf; Fehlerantworten werden zu HarnessError
  private async request<T>(method: 'GET' | 'POST', path: string, body?: unknown): Promise<T> {
    await this.ensureServer();
    const res = await fetch(`http://127.0.0.1:${this.port}${path}`, {
      method,
      headers: method === 'POST' ? { 'content-type': 'application/json', 'x-harness': '1' } : {},
      body: method === 'POST' ? JSON.stringify(body ?? {}) : undefined,
    });
    const data = (await res.json().catch(() => null)) as (T & { code?: string; message?: string }) | null;
    if (!res.ok) throw new HarnessError(data?.code ?? 'dispatch-failed', data?.message ?? `Der Server antwortet mit ${res.status}.`);
    if (data === null) throw new HarnessError('dispatch-failed', 'Der Server lieferte keine lesbare Antwort.');
    return data;
  }

  // Server prüfen und bei freiem Port starten
  private async ensureServer(): Promise<void> {
    const state = await this.launcher.probe(this.port);
    if (state === 'foreign') throw new HarnessError('port-foreign', `Port ${this.port} wird von einem anderen Programm benutzt.`);
    if (state === 'free') await this.launcher.start(this.port);
  }
}
