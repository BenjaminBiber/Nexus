import { FavoritePersistence, FavoriteStore, HarnessError, type Favorite, type HarnessHome, type SessionView } from '@harness/core';

// Hält die Favoriten des Servers: Zustandsspeicher im Speicher, jede Änderung wird sofort gespeichert.
export class FavoriteService {
  private constructor(
    private readonly store: FavoriteStore,
    private readonly persistence: FavoritePersistence,
    private readonly log: (line: string) => void,
  ) {}

  // Lädt die gespeicherten Favoriten; eine beschädigte Datei wird gesichert und im Protokoll vermerkt
  static async create(home: HarnessHome, log: (line: string) => void = () => {}): Promise<FavoriteService> {
    const persistence = new FavoritePersistence(home);
    const { favorites, recovered } = await persistence.read();
    if (recovered) log('favorites.json war beschädigt und wurde nach favorites.json.bak gesichert.');
    return new FavoriteService(new FavoriteStore(favorites), persistence, log);
  }

  // Markiert eine bekannte Session (oder aktualisiert Favorit und Notiz)
  async mark(projectId: string, session: SessionView | undefined, note?: string): Promise<Favorite> {
    if (!session) throw new HarnessError('session-unknown', 'Diese Session ist nicht bekannt.');
    const favorite = this.store.add(projectId, this.snapshot(session), note);
    await this.save();
    return favorite;
  }

  // Ändert die Notiz eines Favoriten
  async setNote(projectId: string, sessionId: string, note: string): Promise<Favorite> {
    const favorite = this.store.setNote(projectId, sessionId, note);
    await this.save();
    return favorite;
  }

  // Entfernt die Markierung; liefert, ob es sie gab
  async unmark(projectId: string, sessionId: string): Promise<boolean> {
    const removed = this.store.remove(projectId, sessionId);
    if (removed) await this.save();
    return removed;
  }

  // Favoriten eines Projekts, zuletzt markierte zuerst
  list(projectId: string): Favorite[] {
    return this.store.list(projectId);
  }

  // Nach einem Hook-Ereignis: Momentaufnahme nachführen und fortgesetzte Session zuordnen
  async onSession(projectId: string, session: SessionView, resumed: boolean): Promise<void> {
    let changed = false;
    if (this.store.isFavorite(projectId, session.id)) {
      this.store.refresh(projectId, this.snapshot(session));
      changed = true;
    }
    if (resumed && this.store.linkResumed(projectId, session.cwd, session.id)) changed = true;
    if (changed) await this.save();
  }

  // Momentaufnahme einer Session
  private snapshot(session: SessionView) {
    return {
      sessionId: session.id, tickets: session.tickets, cwd: session.cwd, title: session.title,
      state: session.state, lastEventAt: session.lastEventAt,
    };
  }

  // Schreibt die Favoriten; ein Fehler wird protokolliert und weitergegeben
  private async save(): Promise<void> {
    try {
      await this.persistence.write(this.store.toJSON());
    } catch (error) {
      this.log(`Favoriten konnten nicht gespeichert werden: ${(error as Error).message}`);
      throw error;
    }
  }
}
