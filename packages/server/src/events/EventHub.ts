import type { DbBoardEvent, DbQuestionsEvent } from './BoardChangeWatcher.js';
import type { SpecDocsEvent } from './SpecDocWatcher.js';

export type LiveEvent = { type: 'sessions' } | DbBoardEvent | DbQuestionsEvent | SpecDocsEvent;
type Listener = (event: LiveEvent) => void;

// Verteilt Änderungsmeldungen je Projekt an alle offenen Ereignisströme.
export class EventHub {
  private readonly channels = new Map<string, Set<Listener>>();

  // Meldet einen Empfänger an; der Rückgabewert meldet ihn wieder ab
  subscribe(projectId: string, listener: Listener): () => void {
    let channel = this.channels.get(projectId);
    if (!channel) {
      channel = new Set();
      this.channels.set(projectId, channel);
    }
    channel.add(listener);
    const joined = channel;
    return () => {
      joined.delete(listener);
      if (joined.size === 0) this.channels.delete(projectId);
    };
  }

  // Sendet ein Ereignis an alle Empfänger eines Projekts (ohne Empfänger geschieht nichts)
  publish(projectId: string, event: LiveEvent): void {
    this.channels.get(projectId)?.forEach((listener) => listener(event));
  }

  // Anzahl der Empfänger eines Projekts
  subscribers(projectId: string): number {
    return this.channels.get(projectId)?.size ?? 0;
  }

  // Meldet alle Empfänger ab
  close(): void {
    this.channels.clear();
  }
}
