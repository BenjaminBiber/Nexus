// Vergleich zweier Messungen: ist eine Messgröße schlechter geworden als in contract/baseline.json?
// Reine Funktionen ohne Zustand, damit der Vergleich prüfbar bleibt.
import { round1 } from './Stats.mjs';

// Toleranz, solange keine andere genannt ist (Prozent)
export const DEFAULT_TOLERANCE_PCT = 10;
// Felder einer Messgröße in der Reihenfolge, in der sie als maßgeblicher Wert gelten
const FIELDS = ['p95', 'median', 'value'];

// Maßgeblicher Wert einer Messgröße: p95, sonst median, sonst value; null heißt „nicht vergleichbar“
export function metricValue(metric) {
  if (typeof metric === 'number') return Number.isFinite(metric) ? metric : null;
  if (!metric || typeof metric !== 'object') return null;
  for (const field of FIELDS) {
    const value = metric[field];
    if (typeof value === 'number' && Number.isFinite(value)) return value;
  }
  return null;
}

// Abweichung von alt zu neu in Prozent; war alt 0, gibt es keinen Prozentwert (0 → 0, mehr → Infinity)
export function deltaPercent(alt, neu) {
  if (alt === 0) return neu === 0 ? 0 : Infinity;
  return ((neu - alt) / alt) * 100;
}

// Vergleicht die Messgrößen zweier Läufe. Maßgeblich ist p95, sonst median, sonst value; größer heißt schlechter.
// Jede Messgröße eines der beiden Läufe ergibt eine Zeile, erst die des neuen Laufs. Fehlt ein Wert auf einer Seite,
// nennt `fehlt` sie und die Zeile ist nur eine Warnung – nie `schlechter`.
export function compareMetrics(alt, neu, tolerancePct = DEFAULT_TOLERANCE_PCT) {
  const before = asMetrics(alt);
  const after = asMetrics(neu);
  const names = [...new Set([...Object.keys(after), ...Object.keys(before)])];
  return names.map((name) => {
    const a = metricValue(before[name]);
    const b = metricValue(after[name]);
    if (a === null || b === null) {
      return { name, alt: a, neu: b, deltaPct: null, schlechter: false, fehlt: a === null ? 'alt' : 'neu' };
    }
    const delta = deltaPercent(a, b);
    return { name, alt: a, neu: b, deltaPct: Number.isFinite(delta) ? round1(delta) : delta, schlechter: delta > tolerancePct, fehlt: null };
  });
}

// Zeilen, die die Toleranz überschreiten (Grundlage des Rückgabecodes)
export function worseRows(rows) {
  return rows.filter((row) => row.schlechter);
}

// Hinweis, wenn die Messungen von verschiedenen Rechnern stammen; dann sagt der Vergleich wenig aus
export function machineWarning(alt, neu) {
  const name = (machine) => [machine?.os, machine?.arch, machine?.cpu, machine?.node].filter(Boolean).join(' / ');
  const before = name(alt);
  const after = name(neu);
  if (!before || before === after) return null;
  return `Achtung: Die Messungen stammen von verschiedenen Rechnern (alt: ${before}; neu: ${after}).`;
}

// Tabelle alt/neu/Abweichung als Text (für stderr)
export function formatComparison(rows, tolerancePct = DEFAULT_TOLERANCE_PCT) {
  const head = ['Messgröße', 'alt', 'neu', 'Abweichung', 'Urteil'];
  const body = rows.map((row) => [
    row.name,
    number(row.alt),
    number(row.neu),
    row.deltaPct === null ? '–' : `${row.deltaPct > 0 ? '+' : ''}${number(row.deltaPct)} %`,
    verdict(row, tolerancePct),
  ]);
  const widths = head.map((_, column) => Math.max(...[head, ...body].map((line) => line[column].length)));
  const line = (cells) => cells.map((cell, column) => (column === 0 ? cell.padEnd(widths[column]) : cell.padStart(widths[column]))).join('  ').trimEnd();
  return [line(head), widths.map((width) => '-'.repeat(width)).join('  '), ...body.map(line)].join('\n');
}

// Urteil einer Zeile im Klartext
function verdict(row, tolerancePct) {
  if (row.fehlt === 'alt') return 'nur im neuen Lauf';
  if (row.fehlt === 'neu') return 'fehlt im neuen Lauf';
  if (row.schlechter) return `schlechter (über ${tolerancePct} %)`;
  return row.deltaPct > 0 ? 'in der Toleranz' : 'besser oder gleich';
}

// Zahl für die Tabelle; fehlende Werte als Strich
function number(value) {
  if (value === null) return '–';
  if (!Number.isFinite(value)) return '∞';
  return String(value);
}

// Messgrößen eines Laufs; alles andere gilt als „keine Messgrößen“
function asMetrics(value) {
  return value && typeof value === 'object' && !Array.isArray(value) ? value : {};
}
