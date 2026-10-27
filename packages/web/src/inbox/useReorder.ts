import { useState } from 'react';
import { useApi } from '../api/ApiContext.js';
import type { InboxItemDto } from '../api/inboxTypes.js';

// Verschiebt einen Eintrag von `from` nach `to` (neue Liste)
export function moveItem<T>(items: T[], from: number, to: number): T[] {
  if (from === to || from < 0 || to < 0 || from >= items.length || to >= items.length) return items;
  const next = [...items];
  const [moved] = next.splice(from, 1);
  next.splice(to, 0, moved);
  return next;
}

// Umsortieren von „Meine Aufgaben“: zeigt die neue Reihenfolge sofort (optimistisch), speichert alle Schlüssel und stellt
// bei einem Fehler die vorige Reihenfolge wieder her. `apply` setzt die angezeigte Liste.
export function useReorder(items: InboxItemDto[], apply: (items: InboxItemDto[]) => void) {
  const api = useApi();
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  const move = async (from: number, to: number) => {
    const next = moveItem(items, from, to);
    if (next === items) return;
    const before = items;
    setError(null);
    setSaving(true);
    apply(next);
    try {
      await api.saveInboxOrder(next.map((i) => i.key));
    } catch (e) {
      apply(before);
      setError(`Reihenfolge konnte nicht gespeichert werden: ${(e as Error).message}`);
    } finally {
      setSaving(false);
    }
  };

  return { move, error, saving };
}
