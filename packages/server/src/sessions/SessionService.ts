import { SessionPersistence, SessionStore, type HarnessHome, type HookEvent, type Session, type SessionView } from '@harness/core';
import type { EventHub } from '../events/EventHub.js';

const SAVE_DELAY_MS = 300;

// Hält die Sessions des Servers: Zustandsspeicher im Speicher, entprelltes Speichern, Meldung an offene Seiten.
export class SessionService {
  private timer: NodeJS.Timeout | null = null;

  private constructor(
    private readonly store: SessionStore,
    private readonly persistence: SessionPersistence,
    private readonly hub: EventHub,
  ) {}

  // Lädt die gespeicherten Sessions; eine beschädigte Datei führt zu einem leeren Start
  static async create(home: HarnessHome, hub: EventHub): Promise<SessionService> {
    const persistence = new SessionPersistence(home);
    let sessions: Session[] = [];
    try {
      sessions = await persistence.read();
    } catch {
      /* beschädigte Datei: neu beginnen, sie wird beim nächsten Speichern ersetzt */
    }
    const store = new SessionStore(sessions);
    store.prune();
    return new SessionService(store, persistence, hub);
  }

  // Wendet ein Ereignis an, plant das Speichern und meldet die Änderung
  apply(projectId: string, event: HookEvent): Session {
    const session = this.store.apply(projectId, event);
    this.changed(projectId);
    return session;
  }

  // Ordnet eine Session einem Ticket zu
  claim(projectId: string, sessionId: string, ticketId: string): Session {
    const session = this.store.claim(projectId, sessionId, ticketId);
    this.changed(projectId);
    return session;
  }

  // Sessions eines Projekts
  list(projectId: string): SessionView[] {
    return this.store.list(projectId);
  }

  // Sessions eines Tickets
  forTicket(projectId: string, ticketId: string): SessionView[] {
    return this.store.forTicket(projectId, ticketId);
  }

  // Schreibt ausstehende Änderungen sofort (beim Beenden des Servers)
  async close(): Promise<void> {
    if (this.timer) {
      clearTimeout(this.timer);
      this.timer = null;
      await this.save();
    }
  }

  // Nach einer Änderung: Speichern einplanen und offene Seiten benachrichtigen
  private changed(projectId: string): void {
    this.hub.publish(projectId, { type: 'sessions' });
    if (this.timer) return;
    this.timer = setTimeout(() => {
      this.timer = null;
      void this.save();
    }, SAVE_DELAY_MS);
  }

  // Speichert alle Sessions; Fehler sind nur ein Verlust des Verlaufs, kein Grund zum Abbruch
  private async save(): Promise<void> {
    try {
      await this.persistence.write(this.store.toJSON());
    } catch {
      /* wird beim nächsten Ereignis erneut versucht */
    }
  }
}
