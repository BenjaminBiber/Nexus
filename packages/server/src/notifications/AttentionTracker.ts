import type { DisplayState, SessionView } from '@harness/core';

export interface PendingSession {
  kind: 'bereit' | 'wartet';
  /** Ticketnummer, sonst Sessiontitel, sonst Projektname */
  label: string;
}

// Merkt sich, welche Sessions auf den Nutzer warten, und erkennt neue Ereignisse (bereit, wartet).
export class AttentionTracker {
  private readonly pending = new Map<string, PendingSession>();

  // Prüft den Zustandswechsel einer Session; liefert die Art der Meldung oder null
  observe(before: DisplayState | undefined, view: SessionView, label: string): PendingSession['kind'] | null {
    const key = `${view.projectId}:${view.id}`;
    const now = view.displayState;
    if (now === 'running' || now === 'ended' || now === 'stale') {
      this.pending.delete(key);
      return null;
    }
    if (now === 'waiting' && before !== 'waiting') return this.add(key, 'wartet', label);
    if (now === 'idle' && (before === 'running' || before === 'waiting')) return this.add(key, 'bereit', label);
    return null;
  }

  // Alle wartenden Sessions
  list(): PendingSession[] {
    return [...this.pending.values()];
  }

  // Nimmt die Session auf und gibt die Art zurück
  private add(key: string, kind: PendingSession['kind'], label: string): PendingSession['kind'] {
    this.pending.set(key, { kind, label });
    return kind;
  }
}
