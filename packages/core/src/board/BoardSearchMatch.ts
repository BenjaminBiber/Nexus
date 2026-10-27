import { SEARCH_WINDOW } from './BoardSearchTypes.js';

// Ausschnitt eines Feldes: Text ab textStart und die Länge des ganzen Feldes
export interface SearchExcerpt {
  text: string;
  textStart: number;
  textLength: number;
}

// Treffer und Ausschnitt der Board-Suche ohne Datenbank: Groß/Klein egal (Unicode über toLowerCase), Akzente zählen,
// Teilwörter zählen, Sonderzeichen wörtlich. Gemeinsam für die Suche im Speicher und in SQLite.
export class BoardSearchMatch {
  private readonly needles: string[];

  constructor(terms: string[]) {
    this.needles = terms.map((t) => t.toLowerCase());
  }

  // Alle Begriffe kommen im Text vor; null nie
  matches(text: string | null): text is string {
    if (text === null) return false;
    const lower = text.toLowerCase();
    return this.needles.every((n) => lower.includes(n));
  }

  // Fenster wie MssqlSearchRepository: bis `window` Zeichen vor der ersten Fundstelle des ersten Begriffs,
  // insgesamt 2 * window + Länge des Begriffs
  excerpt(text: string, window = SEARCH_WINDOW): SearchExcerpt {
    const first = this.needles.length ? text.toLowerCase().indexOf(this.needles[0]) : -1;
    const start = first > window ? first - window : 0;
    const length = window * 2 + (this.needles[0]?.length ?? 0);
    return { text: text.slice(start, start + length), textStart: start, textLength: text.length };
  }
}
