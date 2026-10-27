import { HarnessError } from '../errors/HarnessError.js';
import type { Favorite, SessionSnapshot } from './Favorite.js';

// Zustandsspeicher der Favoriten (reine Logik, ohne Dateien).
export class FavoriteStore {
  private favorites: Favorite[];

  constructor(
    favorites: Favorite[] = [],
    private readonly now: () => Date = () => new Date(),
  ) {
    this.favorites = favorites.map((f) => ({ ...f, tickets: [...f.tickets], resumedAs: [...f.resumedAs] }));
  }

  // Markiert eine Session; erneutes Markieren aktualisiert die Momentaufnahme und behält die Notiz
  add(projectId: string, snapshot: SessionSnapshot, note?: string): Favorite {
    let favorite = this.find(projectId, snapshot.sessionId);
    if (!favorite) {
      favorite = {
        projectId, sessionId: snapshot.sessionId, note: note ?? '', tickets: [], cwd: snapshot.cwd,
        markedAt: this.now().toISOString(), lastState: snapshot.state, lastEventAt: snapshot.lastEventAt, resumedAs: [],
      };
      this.favorites.push(favorite);
    } else if (note !== undefined) {
      favorite.note = note;
    }
    this.apply(favorite, snapshot);
    return favorite;
  }

  // Aktualisiert die Momentaufnahme eines Favoriten (Unbekannte werden ignoriert)
  refresh(projectId: string, snapshot: SessionSnapshot): void {
    const favorite = this.find(projectId, snapshot.sessionId);
    if (favorite) this.apply(favorite, snapshot);
  }

  // Ändert die Notiz
  setNote(projectId: string, sessionId: string, note: string): Favorite {
    const favorite = this.find(projectId, sessionId);
    if (!favorite) throw new HarnessError('favorite-not-found', `Favorit ${sessionId} gibt es nicht.`);
    favorite.note = note;
    return favorite;
  }

  // Entfernt die Markierung; liefert, ob es sie gab
  remove(projectId: string, sessionId: string): boolean {
    const before = this.favorites.length;
    this.favorites = this.favorites.filter((f) => !(f.projectId === projectId && f.sessionId === sessionId));
    return this.favorites.length < before;
  }

  // Favoriten eines Projekts, zuletzt markierte zuerst
  list(projectId: string): Favorite[] {
    return this.favorites.filter((f) => f.projectId === projectId).sort((a, b) => b.markedAt.localeCompare(a.markedAt));
  }

  isFavorite(projectId: string, sessionId: string): boolean {
    return this.find(projectId, sessionId) !== undefined;
  }

  // Ordnet eine fortgesetzte Session dem einzigen Favoriten desselben Arbeitsverzeichnisses zu (sonst null)
  linkResumed(projectId: string, cwd: string, newSessionId: string): Favorite | null {
    const key = this.key(cwd);
    const matches = this.favorites.filter((f) => f.projectId === projectId && this.key(f.cwd) === key);
    if (matches.length !== 1 || matches[0].sessionId === newSessionId) return null;
    if (!matches[0].resumedAs.includes(newSessionId)) matches[0].resumedAs.push(newSessionId);
    return matches[0];
  }

  // Alle Favoriten zum Speichern
  toJSON(): Favorite[] {
    return this.favorites;
  }

  // Übernimmt Angaben aus der Momentaufnahme
  private apply(favorite: Favorite, snapshot: SessionSnapshot): void {
    favorite.tickets = [...snapshot.tickets];
    favorite.cwd = snapshot.cwd;
    if (snapshot.title) favorite.title = snapshot.title;
    favorite.lastState = snapshot.state;
    favorite.lastEventAt = snapshot.lastEventAt;
  }

  private find(projectId: string, sessionId: string): Favorite | undefined {
    return this.favorites.find((f) => f.projectId === projectId && f.sessionId === sessionId);
  }

  // Vergleichsschlüssel für Verzeichnisse: Schrägstriche, ohne Endstrich, unter Windows ohne Beachtung der Schreibweise
  private key(path: string): string {
    const cleaned = path.replace(/\\/g, '/').replace(/\/+$/, '');
    return process.platform === 'win32' ? cleaned.toLowerCase() : cleaned;
  }
}
