import type { Note } from './BoardTypes.js';

// Ab diesem Anteil gemeinsamer Begriffe (bezogen auf den längeren Text) gilt eine Notiz als derselbe Befund. Am kürzeren
// gemessen genügten drei Allerweltsbegriffe des Bereichs (Worktree, Branch, Dispatcher), damit eine kurze Notiz an
// jeder langen hängen blieb und ohne --force abgelehnt wurde.
const THRESHOLD = 0.25;
// Weniger gemeinsame Begriffe sind Zufall, auch bei sehr kurzen Texten
const MIN_SHARED = 3;
// Ticket- und Fragen-IDs (AIH-0022, ARC-F0001) sagen nichts über den Befund
const TICKET_ID = /\b[a-z][a-z0-9]{1,9}-f?\d{3,}\b/g;
// Füllwörter und Floskeln, die in fast jeder Notiz stehen
const STOP_WORDS = new Set([
  'der', 'die', 'das', 'und', 'oder', 'nicht', 'von', 'mit', 'bei', 'aus', 'auf', 'für', 'ist', 'sind', 'ein', 'eine', 'einer',
  'eines', 'den', 'dem', 'des', 'zu', 'im', 'in', 'am', 'an', 'als', 'auch', 'wenn', 'weil', 'dass', 'sich', 'nur', 'noch',
  'schon', 'wird', 'werden', 'kein', 'keine', 'diese', 'dieser', 'dieses', 'unabhängig', 'unabhaengig', 'verursacht',
  'vermutlich', 'durch', 'nach', 'vor', 'über', 'unter', 'sehr', 'test', 'tests', 'npm', 'run', 'the', 'and',
]);

/** Titel einer Notiz zum Anzeigen; ältere Notizen ohne Titel zeigen die erste Zeile ihres Inhalts */
export function noteTitle(note: Pick<Note, 'id' | 'title' | 'body'>): string {
  const title = note.title.trim() || (note.body.split('\n').map((l) => l.trim()).find(Boolean) ?? '');
  return title || `Notiz ${note.id}`;
}

// Erkennt, ob ein neuer Befund schon als Notiz festgehalten ist (gemeinsame Begriffe, ohne IDs und Füllwörter)
export class NoteSimilarity {
  // Notizen, die denselben Befund beschreiben, ähnlichste zuerst
  similar(text: string, notes: Note[]): Note[] {
    const words = this.words(text);
    return notes
      .map((note) => ({ note, score: this.score(words, this.words(`${note.title}\n${note.body}`)) }))
      .filter((s) => s.score !== null)
      .sort((a, b) => b.score! - a.score!)
      .map((s) => s.note);
  }

  // Anteil gemeinsamer Begriffe am längeren Text; null unter der Schwelle
  private score(a: Set<string>, b: Set<string>): number | null {
    let shared = 0;
    for (const word of a) if (b.has(word)) shared++;
    const larger = Math.max(a.size, b.size);
    if (shared < MIN_SHARED) return null;
    const score = shared / larger;
    return score >= THRESHOLD ? score : null;
  }

  // Begriffe eines Texts: klein, ab drei Zeichen, ohne IDs und Füllwörter
  private words(text: string): Set<string> {
    return new Set(text.toLowerCase().replace(TICKET_ID, ' ').split(/[^\p{L}\p{N}]+/u).filter((w) => w.length >= 3 && !STOP_WORDS.has(w)));
  }
}
