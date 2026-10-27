import { HOOK_KINDS, type HookEvent, type HookKind } from '../sessions/Session.js';

// Bildet die Eingabe eines Claude-Code-Hooks auf ein normalisiertes Ereignis ab.
export class HookEventFactory {
  // Liefert null, wenn die Eingabe unbrauchbar ist (keine Kennung, kein Objekt, unbekanntes Ereignis)
  create(kind: HookKind, payload: unknown, fallbackCwd: string, tickets: string[]): HookEvent | null {
    if (!HOOK_KINDS.includes(kind) || typeof payload !== 'object' || payload === null) return null;
    const p = payload as Record<string, unknown>;
    const sessionId = this.text(p.session_id);
    if (!sessionId) return null;
    const event: HookEvent = {
      kind,
      sessionId,
      cwd: this.slashes(this.text(p.cwd) ?? fallbackCwd),
      tickets,
    };
    const optional: Array<[keyof HookEvent, string | undefined]> = [
      ['agentId', this.text(p.agent_id)],
      ['agentType', this.text(p.agent_type)],
      ['notificationType', this.text(p.notification_type) ?? this.text(p.type)],
      ['title', this.text(p.session_title)],
      ['model', this.text(p.model)],
      ['source', this.text(p.source)],
    ];
    for (const [key, value] of optional) if (value) (event as unknown as Record<string, string>)[key] = value;
    return event;
  }

  // Nicht leerer Text oder undefined
  private text(value: unknown): string | undefined {
    return typeof value === 'string' && value.trim() ? value : undefined;
  }

  // Schrägstriche statt Rückwärtsstrichen
  private slashes(path: string): string {
    return path.replace(/\\/g, '/');
  }
}
