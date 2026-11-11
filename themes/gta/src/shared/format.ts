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

/** San Andreas: `09:41` */
export function formatClockSa(date: Date): string {
  return `${two(date.getHours())}:${two(date.getMinutes())}`;
}

/** GTA V: `Mittwoch 09:41` */
export function formatClockV(date: Date): string {
  return `${WEEKDAYS[date.getDay()]} ${formatClockSa(date)}`;
}

/** `0.42` → `42 %` */
export function formatPercent(fraction: number): string {
  return `${whole(fraction * 100)} %`;
}
