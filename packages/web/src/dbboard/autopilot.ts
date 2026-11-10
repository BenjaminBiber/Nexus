import type { RawDispatchConfigDto } from '../api/dispatchTypes.js';

// Vorgabe des Servers für das Ausnahme-Label, solange die Datei keins nennt
const DEFAULT_EXCEPT_LABEL = 'interaktiv';

// Läuft ein Ticket mit diesen Labels im Autopilot? Gleiche Regel wie autopilot_for im Dispatcher:
// Schalter an und kein Ausnahme-Label (getrimmt, ohne Groß-/Kleinschreibung); null/leer = keine Ausnahme.
export function isAutopilot(config: RawDispatchConfigDto['autopilot'], labels: readonly string[]): boolean {
  if (config?.enabled !== true) return false;
  const except = (config.exceptLabel === undefined ? DEFAULT_EXCEPT_LABEL : config.exceptLabel ?? '').trim().toLowerCase();
  if (!except) return true;
  return !labels.some((label) => label.trim().toLowerCase() === except);
}
