export interface HeadingRef {
  id: string;
  text: string;
  level: 2 | 3;
}

// Macht aus einem Überschriftentext einen Anker: Kleinbuchstaben, Umlaute bleiben, Rest wird „-“
export function slugify(text: string): string {
  const slug = text.toLowerCase().replace(/[^\p{L}\p{N}]+/gu, '-').replace(/^-+|-+$/g, '');
  return slug || 'abschnitt';
}

// Vergibt allen h2/h3 im Dokument eindeutige IDs und liefert sie in Dokumentreihenfolge
export function assignHeadingIds(root: HTMLElement): HeadingRef[] {
  const seen = new Map<string, number>();
  return Array.from(root.querySelectorAll('h2, h3')).map((el) => {
    const text = el.textContent ?? '';
    const base = slugify(text);
    const count = seen.get(base) ?? 0;
    seen.set(base, count + 1);
    const id = count === 0 ? base : `${base}-${count + 1}`;
    el.id = id;
    return { id, text, level: el.tagName === 'H2' ? 2 : 3 };
  });
}
