import type { DbCardDto } from '../api/dbTypes.js';

// Karten, deren ID, Titel, Zuständiger oder Label den Suchtext enthält (ohne Groß-/Kleinschreibung)
export function filterDbCards(cards: DbCardDto[], query: string): DbCardDto[] {
  const q = query.trim().toLowerCase();
  if (!q) return cards;
  return cards.filter((c) => [c.id, c.title, c.assignee ?? '', ...c.labels].join(' ').toLowerCase().includes(q));
}
