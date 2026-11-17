// Gemeinsame Anzeige-Helfer der Fragen-Seite: Herkunft, Datumsformen, Fragenummern.

export const ORIGIN_KIND: Record<string, string> = { human: 'Mensch', session: 'Session', subagent: 'Subagent' };

// Fragenummer wie ARC-F0001 (Gruppe 1: Projektschlüssel); alles andere ist ein Ticket
export const QUESTION_ID = /^([A-Za-z][A-Za-z0-9]{1,5})-F\d+$/;

// 26.10.2026
export function dateDe(iso: string): string {
  return new Date(iso).toLocaleDateString('de-DE', { day: '2-digit', month: '2-digit', year: 'numeric' });
}

// 10:14
export function timeDe(iso: string): string {
  return new Date(iso).toLocaleTimeString('de-DE', { hour: '2-digit', minute: '2-digit' });
}

// 26.10.2026, 10:14
export function dateTimeDe(iso: string): string {
  return `${dateDe(iso)}, ${timeDe(iso)}`;
}

// 26.10.
export function dayMonthDe(iso: string): string {
  return new Date(iso).toLocaleDateString('de-DE', { day: '2-digit', month: '2-digit' }).replace(/\.?$/, '.');
}

// „1 Option“, „3 Optionen“
export function optionsLabel(count: number): string {
  return count === 1 ? '1 Option' : `${count} Optionen`;
}
