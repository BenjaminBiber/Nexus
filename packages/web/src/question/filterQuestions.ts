import type { OriginKind } from '../api/questionTypes.js';

// Auswahlkriterien der Fragenliste (ohne Zustand, der hat eigene Schaltflächen)
export interface QuestionFilterState {
  tag: string;
  link: string;
  origin: '' | OriginKind;
  waiting: boolean;
  researched: boolean;
  text: string;
}

export const NO_FILTER: QuestionFilterState = { tag: '', link: '', origin: '', waiting: false, researched: false, text: '' };

// Was die Filter an einer Frage der Board-Datenbank prüfen
export interface QuestionFacets {
  tags: string[];
  links: Array<{ target: string; type: string }>;
  origin: OriginKind;
  waiting: boolean;
  researched: boolean;
  /** Durchsuchbare Texte (Nummer, Titel, Beschreibung, …) */
  text: string[];
}

// Ist mindestens ein Kriterium gesetzt?
export function isFiltering(filter: QuestionFilterState): boolean {
  return Boolean(filter.tag || filter.link || filter.origin || filter.waiting || filter.researched || filter.text.trim());
}

// Wählt Fragen beliebiger Form über ihre Merkmale aus: alle genannten Kriterien gelten (und); die Reihenfolge bleibt
export function filterByFacets<T>(questions: T[], filter: QuestionFilterState, facets: (q: T) => QuestionFacets): T[] {
  const needle = filter.text.trim().toLowerCase();
  return questions.filter((item) => {
    const q = facets(item);
    if (filter.tag && !q.tags.includes(filter.tag)) return false;
    if (filter.link && !q.links.some((l) => l.target === filter.link)) return false;
    if (filter.origin && q.origin !== filter.origin) return false;
    if (filter.waiting && !q.waiting) return false;
    if (filter.researched && !q.researched) return false;
    if (needle && !q.text.some((p) => p.toLowerCase().includes(needle))) return false;
    return true;
  });
}

// Alle vorkommenden Tags, sortiert
export function tagsOf(questions: Array<Pick<QuestionFacets, 'tags'>>): string[] {
  return [...new Set(questions.flatMap((q) => q.tags))].sort();
}

// Alle vorkommenden Bezugsziele mit Art, sortiert
export function linksOf(questions: Array<Pick<QuestionFacets, 'links'>>): Array<{ target: string; type: string }> {
  const seen = new Map<string, string>();
  questions.forEach((q) => q.links.forEach((l) => seen.set(l.target, l.type)));
  return [...seen].map(([target, type]) => ({ target, type })).sort((a, b) => a.target.localeCompare(b.target));
}
