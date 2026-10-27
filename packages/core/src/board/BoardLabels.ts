import { BoardLimits } from './BoardLimits.js';
import { MAX_LABEL_LENGTH } from './BoardTypes.js';

// Vereinheitlicht Labels für Tickets und Notizen.
export class BoardLabels {
  // An Kommas trennen, trimmen, Leere entfernen, Doppelte ohne Rücksicht auf Groß-/Kleinschreibung
  // entfernen (erste Schreibweise bleibt; der SQL-Schlüssel unterscheidet sie nicht) und Länge prüfen
  static normalize(input: string[]): string[] {
    const result: string[] = [];
    const seen = new Set<string>();
    for (const label of input.flatMap((l) => l.split(',')).map((l) => l.trim())) {
      const key = label.toLowerCase();
      if (!label || seen.has(key)) continue;
      BoardLimits.text(label, MAX_LABEL_LENGTH, `Label „${label.slice(0, 30)}…“`);
      seen.add(key);
      result.push(label);
    }
    return result;
  }
}
