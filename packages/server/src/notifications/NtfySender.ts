import type { ToastKind, ToastModel } from './ToastModel.js';

// Ziel der Push-Meldung: ntfy-Server, Thema und optionales Zugriffstoken
export interface NtfyTarget {
  server: string;
  topic: string;
  token: string;
}

// Schickt eine HTTP-Anfrage; im Test ersetzbar
export type NtfyFetch = (url: string, init: { method: string; headers: Record<string, string>; body: string }) => Promise<{ ok: boolean; status: number; text(): Promise<string> }>;

// Emoji-Tags je Art der Meldung (ntfy zeigt sie vor dem Titel)
const TAGS: Record<ToastKind, string> = {
  bereit: 'white_check_mark',
  wartet: 'raising_hand',
  gesammelt: 'bell',
  'frage-neu': 'grey_question',
  recherche: 'mag',
};

// Sendet eine Meldung als Push über ntfy (https://ntfy.sh oder eigener Server), per JSON-Veröffentlichung.
export class NtfySender {
  constructor(private readonly fetchFn: NtfyFetch = (url, init) => fetch(url, init)) {}

  // Sendet die Meldung; Fehler als Text, sonst null. Wartende Sessions und neue Fragen kommen mit hoher Priorität.
  async send(target: NtfyTarget, model: ToastModel): Promise<string | null> {
    const headers: Record<string, string> = { 'Content-Type': 'application/json' };
    if (target.token) headers.Authorization = `Bearer ${target.token}`;
    const body = {
      topic: target.topic,
      title: model.title,
      message: [model.line1, model.line2].filter(Boolean).join('\n'),
      tags: [TAGS[model.kind]],
      priority: model.kind === 'wartet' || model.kind === 'frage-neu' ? 4 : 3,
      ...(this.reachable(model.openUrl) ? { click: model.openUrl } : {}),
    };
    try {
      const res = await this.fetchFn(target.server.replace(/\/+$/, ''), { method: 'POST', headers, body: JSON.stringify(body) });
      if (res.ok) return null;
      const detail = (await res.text().catch(() => '')).trim();
      return `ntfy antwortete mit ${res.status}${detail ? `: ${detail}` : ''}.`;
    } catch (error) {
      return `ntfy nicht erreichbar: ${(error as Error).message}`;
    }
  }

  // Adressen auf dem eigenen Rechner (127.0.0.1, localhost) öffnen auf dem Handy nichts; sie bleiben ohne Klick-Ziel
  private reachable(url: string): boolean {
    try {
      const host = new URL(url).hostname;
      return !['localhost', '127.0.0.1', '[::1]', '::1', '0.0.0.0'].includes(host);
    } catch {
      return false;
    }
  }
}
