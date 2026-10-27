// Darstellung der Kennzahlen des Dashboards; fehlende Werte erscheinen als „–“.

export const NONE = '–';

// Dauer kurz: „45 min“, „3,5 h“, „2,1 Tage“
export function formatDuration(ms: number | null): string {
  if (ms === null) return NONE;
  const minutes = ms / 60_000;
  if (minutes < 90) return `${Math.round(minutes)} min`;
  const hours = minutes / 60;
  if (hours < 36) return `${decimal(hours)} h`;
  return `${decimal(hours / 24)} Tage`;
}

// Kosten in US-Dollar: zwei Nachkommastellen, unter 1 Cent drei
export function formatCost(usd: number | null): string {
  if (usd === null) return NONE;
  const digits = usd !== 0 && Math.abs(usd) < 0.01 ? 3 : 2;
  return `${usd.toLocaleString('de-DE', { minimumFractionDigits: digits, maximumFractionDigits: digits })} $`;
}

// Anteil 0–1 als ganze Prozent
export function formatPercent(rate: number | null): string {
  return rate === null ? NONE : `${Math.round(rate * 100)} %`;
}

// Große Zahlen kurz: 950, 12,3 Tsd., 4,1 Mio.
export function formatCount(n: number | null): string {
  if (n === null) return NONE;
  if (n < 1000) return String(n);
  if (n < 1_000_000) return `${decimal(n / 1000)} Tsd.`;
  return `${decimal(n / 1_000_000)} Mio.`;
}

// Zahl mit einer Nachkommastelle (ohne „,0“)
export function decimal(n: number): string {
  return n.toLocaleString('de-DE', { maximumFractionDigits: 1 });
}

// JJJJ-MM-TT als „TT.MM.“ (Wochenbeginn) bzw. „TT.MM.JJJJ“
export function formatDay(day: string, withYear = false): string {
  const [y, m, d] = day.slice(0, 10).split('-');
  return withYear ? `${d}.${m}.${y}` : `${d}.${m}.`;
}

// Anzahl mit Einzahl oder Mehrzahl: „1 Lauf“, „3 Läufe“
export function plural(n: number, one: string, many: string): string {
  return `${n} ${n === 1 ? one : many}`;
}

// Tag (Ortszeit) als JJJJ-MM-TT
export function isoDay(date: Date): string {
  return `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, '0')}-${String(date.getDate()).padStart(2, '0')}`;
}
