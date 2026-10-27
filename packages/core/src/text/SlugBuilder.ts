const MAX_LENGTH = 40;
const UMLAUTS: Record<string, string> = { ä: 'ae', ö: 'oe', ü: 'ue', ß: 'ss' };

// Baut aus einem Titel einen Kurztitel, der als Git-Branch und Ordnername taugt.
export class SlugBuilder {
  // Kleinschreibung, Umlaute umschreiben, Sonderzeichen zu Bindestrichen, höchstens 40 Zeichen
  build(text: string): string {
    const lower = text.toLowerCase().replace(/[äöüß]/g, (c) => UMLAUTS[c]);
    const dashed = lower.replace(/[^a-z0-9]+/g, '-').replace(/^-+|-+$/g, '');
    return this.shorten(dashed) || 'ticket';
  }

  // Kürzt am letzten Bindestrich innerhalb der Höchstlänge, damit kein Wort zerschnitten wird
  private shorten(slug: string): string {
    if (slug.length <= MAX_LENGTH) return slug;
    const cut = slug.slice(0, MAX_LENGTH);
    const lastDash = cut.lastIndexOf('-');
    return (lastDash > 0 ? cut.slice(0, lastDash) : cut).replace(/-+$/, '');
  }
}
