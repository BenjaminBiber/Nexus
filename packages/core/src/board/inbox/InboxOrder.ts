import type { InboxItem } from './InboxTypes.js';

// Wendet die gespeicherte, manuelle Reihenfolge auf die Einträge an.
export class InboxOrder {
  // Einträge ohne gespeicherte Position kommen zuerst (in der gegebenen Reihenfolge, also nach Dringlichkeit), damit
  // Neues sichtbar ist; danach die einsortierten Einträge in gespeicherter Reihenfolge. Unbekannte Schlüssel fallen weg.
  apply(items: InboxItem[], keys: readonly string[]): InboxItem[] {
    const position = new Map<string, number>();
    keys.forEach((key, index) => { if (!position.has(key)) position.set(key, index); });
    const fresh = items.filter((item) => !position.has(item.key));
    const placed = items.filter((item) => position.has(item.key)).sort((a, b) => position.get(a.key)! - position.get(b.key)!);
    return [...fresh, ...placed];
  }
}
