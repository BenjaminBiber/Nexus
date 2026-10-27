// Kennzahlen einer Messreihe: Median, p95, Mittelwert. Reine Funktionen ohne Zustand, damit sie prüfbar bleiben.

// Wirft, wenn die Reihe leer ist oder keine endlichen Zahlen enthält
function requireValues(values, what) {
  if (!Array.isArray(values) || values.length === 0) throw new Error(`${what} braucht mindestens einen Messwert.`);
  if (values.some((value) => typeof value !== 'number' || !Number.isFinite(value))) {
    throw new Error(`${what} braucht endliche Zahlen.`);
  }
  return [...values].sort((a, b) => a - b);
}

// Median (bei gerader Anzahl das Mittel der beiden mittleren Werte)
export function median(values) {
  const sorted = requireValues(values, 'Der Median');
  const middle = Math.floor(sorted.length / 2);
  return sorted.length % 2 === 1 ? sorted[middle] : (sorted[middle - 1] + sorted[middle]) / 2;
}

// 95. Perzentil mit linearer Interpolation (wie bei Messreihen üblich); bei einem Wert ist es dieser Wert
export function p95(values) {
  return percentile(values, 95);
}

// Perzentil zwischen 0 und 100 mit linearer Interpolation zwischen den Nachbarwerten
export function percentile(values, percent) {
  if (typeof percent !== 'number' || !Number.isFinite(percent) || percent < 0 || percent > 100) {
    throw new Error('Das Perzentil muss zwischen 0 und 100 liegen.');
  }
  const sorted = requireValues(values, `Das ${percent}. Perzentil`);
  const position = ((sorted.length - 1) * percent) / 100;
  const below = Math.floor(position);
  const above = Math.ceil(position);
  if (below === above) return sorted[below];
  return sorted[below] + (sorted[above] - sorted[below]) * (position - below);
}

// Arithmetisches Mittel
export function mean(values) {
  const sorted = requireValues(values, 'Der Mittelwert');
  return sorted.reduce((sum, value) => sum + value, 0) / sorted.length;
}

// Rundet auf eine Nachkommastelle; so stehen die Werte in contract/baseline.json
export function round1(value) {
  if (typeof value !== 'number' || !Number.isFinite(value)) throw new Error('Runden braucht eine endliche Zahl.');
  return Math.round(value * 10) / 10;
}
