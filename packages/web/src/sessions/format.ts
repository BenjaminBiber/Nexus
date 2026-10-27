import type { SessionDisplayState } from '../api/types.js';

export const STATE_LABEL: Record<SessionDisplayState, string> = {
  running: 'läuft',
  waiting: 'wartet auf Eingabe',
  idle: 'untätig',
  ended: 'beendet',
  stale: 'veraltet',
};

// Dauer in Minuten, Stunden oder Tagen: „2 Min.“, „1 Std. 05 Min.“, „2 Tg. 3 Std.“
export function formatDuration(ms: number): string {
  const minutes = Math.floor(ms / 60000);
  if (minutes < 1) return 'weniger als 1 Min.';
  if (minutes < 60) return `${minutes} Min.`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours} Std. ${String(minutes % 60).padStart(2, '0')} Min.`;
  return `${Math.floor(hours / 24)} Tg. ${hours % 24} Std.`;
}

// Abstand zu einem Zeitpunkt: „vor weniger als einer Minute“, „vor 5 Min.“, „vor 2 Std.“, „vor 3 Tg.“
export function formatAgo(iso: string, now: number = Date.now()): string {
  const minutes = Math.floor((now - new Date(iso).getTime()) / 60000);
  if (minutes < 1) return 'vor weniger als einer Minute';
  if (minutes < 60) return `vor ${minutes} Min.`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `vor ${hours} Std.`;
  return `vor ${Math.floor(hours / 24)} Tg.`;
}
