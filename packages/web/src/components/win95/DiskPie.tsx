// Fortschritt im Schema „Windows 95“ wie die Belegung in den Laufwerkseigenschaften: schräg liegende 3D-Torte (blau erledigt,
// magenta offen) mit Legende Erledigt/Offen/Gesamt daneben.
const RX = 36;
const RY = 18;
const CX = 40;
const CY = 20;
const DEPTH = 9;

// Punkt auf dem Rand der Oberseite (Winkel im Uhrzeigersinn ab 3 Uhr), dy verschiebt auf den Boden
function at(angle: number, dy = 0): string {
  return `${(CX + RX * Math.cos(angle)).toFixed(2)} ${(CY + dy + RY * Math.sin(angle)).toFixed(2)}`;
}

// Sichtbares Mantelstück zwischen zwei Winkeln der Vorderseite (0 … π)
function side(from: number, to: number): string {
  return `M${at(from)} A${RX} ${RY} 0 0 1 ${at(to)} L${at(to, DEPTH)} A${RX} ${RY} 0 0 0 ${at(from, DEPTH)} Z`;
}

// Tortenstück der Oberseite im Uhrzeigersinn
function wedge(from: number, to: number): string {
  return `M${CX} ${CY} L${at(from)} A${RX} ${RY} 0 ${to - from > Math.PI ? 1 : 0} 1 ${at(to)} Z`;
}

export function DiskPie({ done, total, label }: { done: number; total: number; label: string }) {
  const share = total === 0 ? 0 : Math.min(done / total, 1);
  const percent = Math.round(share * 100);
  // Erledigt beginnt hinten (12 Uhr) und läuft im Uhrzeigersinn; die Vorderseite (0 … π) zeigt den Mantel
  const start = -Math.PI / 2;
  const end = start + 2 * Math.PI * share;
  return (
    <div className="w95-pie">
      <div className="w95-pie-figure">
        <svg className="w95-pie-chart" viewBox="0 0 80 50" role="img" aria-label={`${label}: ${percent} %`}>
          <path className="w95-pie-open-side" d={side(0, Math.PI)} />
          {end > 0 && <path className="w95-pie-done-side" d={side(0, Math.min(end, Math.PI))} />}
          <ellipse className="w95-pie-open" cx={CX} cy={CY} rx={RX} ry={RY} />
          {share >= 1 ? <ellipse className="w95-pie-done" cx={CX} cy={CY} rx={RX} ry={RY} /> : share > 0 && <path className="w95-pie-done" d={wedge(start, end)} />}
        </svg>
        <span className="w95-pie-caption">{percent} % erledigt</span>
      </div>
      <dl className="w95-pie-legend">
        <div>
          <dt>
            <span className="w95-pie-swatch done" aria-hidden="true" />
            Erledigt:
          </dt>
          <dd>{done}</dd>
        </div>
        <div>
          <dt>
            <span className="w95-pie-swatch open" aria-hidden="true" />
            Offen:
          </dt>
          <dd>{Math.max(total - done, 0)}</dd>
        </div>
        <div className="w95-pie-total">
          <dt>Gesamt:</dt>
          <dd>{total}</dd>
        </div>
      </dl>
    </div>
  );
}
