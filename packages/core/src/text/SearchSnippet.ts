export interface Snippet {
  text: string;
  // Bereiche [Anfang, Ende) der Treffer im Text
  matches: Array<[number, number]>;
}

// Schnitt eines Feldes, das das Repository schon gekürzt hat (Ausschnitt aus der Mitte)
export interface SnippetCut {
  cutBefore?: boolean;
  cutAfter?: boolean;
}

const ELLIPSIS = '…';
// Zeichen vor der ersten Fundstelle, wenn geschnitten werden muss
const LEAD = 40;

// Fundstelle für die Trefferliste: ein kurzer Ausschnitt um den ersten Treffer, Leerraum zusammengefasst,
// mit Bereichen aller Treffer (Groß/Klein egal) zum Hervorheben – ohne HTML.
export class SearchSnippet {
  constructor(private readonly size = 160) {}

  build(raw: string, terms: string[], cut: SnippetCut = {}): Snippet {
    const flat = raw.replace(/\s+/g, ' ').trim();
    const lower = flat.toLowerCase();
    // Nur wenn Kleinschreibung die Länge nicht ändert, stimmen Positionen im Original
    const needles = lower.length === flat.length ? terms.map((t) => t.toLowerCase()).filter(Boolean) : [];
    const first = Math.min(...needles.map((n) => lower.indexOf(n)).filter((i) => i >= 0), Number.POSITIVE_INFINITY);
    let start = 0;
    if (Number.isFinite(first) && first + LEAD > this.size) {
      start = Math.max(0, first - LEAD);
      const space = flat.lastIndexOf(' ', start);
      if (space >= 0 && start - space < 15) start = space + 1;
    }
    let end = Math.min(flat.length, start + this.size);
    if (end < flat.length) {
      const space = flat.lastIndexOf(' ', end);
      if (space > start + this.size / 2) end = space;
    }
    const before = start > 0 || Boolean(cut.cutBefore);
    const after = end < flat.length || Boolean(cut.cutAfter);
    const body = flat.slice(start, end);
    const offset = before ? ELLIPSIS.length : 0;
    return { text: `${before ? ELLIPSIS : ''}${body}${after ? ELLIPSIS : ''}`, matches: this.ranges(body.toLowerCase(), needles, offset) };
  }

  // Alle Vorkommen aller Begriffe, sortiert und zusammengefasst
  private ranges(lower: string, needles: string[], offset: number): Array<[number, number]> {
    const found: Array<[number, number]> = [];
    for (const n of needles) {
      for (let i = lower.indexOf(n); i >= 0; i = lower.indexOf(n, i + 1)) found.push([i, i + n.length]);
    }
    found.sort((a, b) => a[0] - b[0] || a[1] - b[1]);
    const merged: Array<[number, number]> = [];
    for (const r of found) {
      const last = merged.at(-1);
      if (last && r[0] <= last[1]) last[1] = Math.max(last[1], r[1]);
      else merged.push([r[0], r[1]]);
    }
    return merged.map(([a, b]) => [a + offset, b + offset]);
  }
}
