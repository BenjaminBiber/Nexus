/** Farbe des Werts: neutral, gut (grün), Warnung (orange), schlecht (rot) */
export type KpiTone = 'neutral' | 'good' | 'warn' | 'bad';

// Kennzahl-Kachel in einer Zeile: Beschriftung links, Wert rechts; die Erläuterung erscheint als Tooltip
export function KpiTile({ label, value, sub, tone = 'neutral' }: { label: string; value: string; sub?: string; tone?: KpiTone }) {
  return (
    <section className="kpi" aria-label={label} title={sub}>
      <h3 className="kpi-label">{label}</h3>
      <p className={`kpi-value mono tone-${tone}`}>{value}</p>
    </section>
  );
}
