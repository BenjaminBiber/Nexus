import type { HookEvent } from '@harness/core';

const TIMEOUT_MS = 1500;

// Meldet Projekt, Hook-Ereignisse und Zuordnungen an den Server; alle Aufrufe sind zeitbegrenzt und still bei Fehlern.
export class ServerClient {
  constructor(private readonly port: number) {}

  // Meldet das Projekt an (idempotent); liefert die Projekt-ID oder null
  async registerProject(path: string): Promise<string | null> {
    const res = await this.post('/api/projects', { path }, {});
    return (res?.id as string | undefined) ?? null;
  }

  // Sendet ein Ereignis mit dem Token des Projekts
  async sendEvent(projectPath: string, token: string, event: HookEvent): Promise<boolean> {
    return (await this.post('/api/hook', { project: projectPath, event }, { 'x-harness-token': token })) !== null;
  }

  // Ordnet eine Session einem Ticket zu
  async claim(projectId: string, sessionId: string, ticketId: string): Promise<boolean> {
    const path = `/api/projects/${projectId}/sessions/${encodeURIComponent(sessionId)}/claim`;
    return (await this.post(path, { ticket: ticketId }, {})) !== null;
  }

  // POST mit Pflichtkopf und Zeitlimit; liefert den JSON-Körper ({} bei 204) oder null bei Fehlern
  private async post(path: string, body: unknown, headers: Record<string, string>): Promise<Record<string, unknown> | null> {
    try {
      const res = await fetch(`http://127.0.0.1:${this.port}${path}`, {
        method: 'POST',
        headers: { 'content-type': 'application/json', 'x-harness': '1', ...headers },
        body: JSON.stringify(body),
        signal: AbortSignal.timeout(TIMEOUT_MS),
      });
      if (!res.ok) return null;
      return res.status === 204 ? {} : ((await res.json().catch(() => ({}))) as Record<string, unknown>);
    } catch {
      return null;
    }
  }
}
