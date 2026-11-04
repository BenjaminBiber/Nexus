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

// Kurze Dauer mit Sekunden: „unter 1 s“, „2 s“, „1 Min. 05 s“, längeres wie formatDuration
export function formatSeconds(ms: number): string {
  if (ms < 1000) return 'unter 1 s';
  const seconds = Math.round(ms / 1000);
  if (seconds < 60) return `${seconds} s`;
  if (seconds < 3600) return `${Math.floor(seconds / 60)} Min. ${String(seconds % 60).padStart(2, '0')} s`;
  return formatDuration(ms);
}

const NUMBER = new Intl.NumberFormat('de-DE');
const COMPACT = new Intl.NumberFormat('de-DE', { notation: 'compact', maximumFractionDigits: 1 });

// Ganze Zahl mit Tausenderpunkten: „12.345“
export function formatNumber(value: number): string {
  return NUMBER.format(value);
}

// Token-Anzahl kurz: „950“, „12.345“, „1,2 Mio.“
export function formatTokens(value: number): string {
  return value >= 1_000_000 ? COMPACT.format(value) : NUMBER.format(value);
}

// Kosten in US-Dollar: „0,42 $“; sehr kleine Beträge mit mehr Stellen („0,004 $“)
export function formatCost(usd: number): string {
  const digits = usd !== 0 && Math.abs(usd) < 0.01 ? 3 : 2;
  return `${usd.toLocaleString('de-DE', { minimumFractionDigits: digits, maximumFractionDigits: digits })} $`;
}

// Dateigröße: „812 B“, „14,2 KB“, „3,1 MB“
export function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  const units = ['KB', 'MB', 'GB'];
  let value = bytes / 1024;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return `${value.toLocaleString('de-DE', { maximumFractionDigits: 1 })} ${units[unit]}`;
}

// Uhrzeit „14:05“; liegt der Tag nicht heute, mit Datum „03.11. 14:05“
export function formatClock(iso: string, now: number = Date.now()): string {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return '';
  const time = date.toLocaleTimeString('de-DE', { hour: '2-digit', minute: '2-digit' });
  const today = new Date(now).toDateString() === date.toDateString();
  return today ? time : `${date.toLocaleDateString('de-DE', { day: '2-digit', month: '2-digit' })} ${time}`;
}

// Datum und Uhrzeit vollständig, z. B. für title-Attribute: „04.11.2026, 14:05:12“
export function formatDateTime(iso: string): string {
  const date = new Date(iso);
  return Number.isNaN(date.getTime()) ? iso : date.toLocaleString('de-DE');
}
