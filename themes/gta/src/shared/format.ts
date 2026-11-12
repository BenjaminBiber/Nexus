// Anzeigeformate der beiden Stile. Negative Werte und NaN gelten als 0.

const WEEKDAYS = ['Sonntag', 'Montag', 'Dienstag', 'Mittwoch', 'Donnerstag', 'Freitag', 'Samstag'];

const whole = (n: number) => (Number.isFinite(n) && n > 0 ? Math.round(n) : 0);
const two = (n: number) => String(n).padStart(2, '0');

/** GTA V: `$184.000` */
export function formatMoneyV(n: number): string {
  return `$${String(whole(n)).replace(/\B(?=(\d{3})+(?!\d))/g, '.')}`;
}

/** San Andreas: `$00184000` – acht Stellen, größere Zahlen ungekürzt */
export function formatMoneySa(n: number): string {
  return `$${String(whole(n)).padStart(8, '0')}`;
}

/** Platzhalter für Zahlen ohne Board-Stand (nicht 0: das sähe aus wie ein leeres Board) */
export const UNKNOWN = '–';
/** Kontostand ohne Board-Stand: `$–` bzw. acht Striche wie die Stellen in San Andreas */
export const MONEY_UNKNOWN_V = `$${UNKNOWN}`;
export const MONEY_UNKNOWN_SA = '$--------';

/** San Andreas als Betrag (Banner, Cheat): `$40000` */
export function formatAmountSa(n: number): string {
  return `$${whole(n)}`;
}

/** San Andreas: `09:41` */
export function formatClockSa(date: Date): string {
  return `${two(date.getHours())}:${two(date.getMinutes())}`;
}

/** GTA V: `Mittwoch 09:41` */
export function formatClockV(date: Date): string {
  return `${WEEKDAYS[date.getDay()]} ${formatClockSa(date)}`;
}

/** Anteil als ganze Prozent, auf 0..100 begrenzt: `0.424` → `42` */
export function percentOf(fraction: number): number {
  return Math.round(Math.min(1, Math.max(0, fraction)) * 100);
}

/** `1 offenes Ticket`, `5 offene Tickets` */
export function openTickets(n: number): string {
  return n === 1 ? '1 offenes Ticket' : `${n} offene Tickets`;
}

/** `0.42` → `42 %` */
export function formatPercent(fraction: number): string {
  return `${whole(fraction * 100)} %`;
}
