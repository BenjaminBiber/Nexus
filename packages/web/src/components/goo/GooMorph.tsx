import type { CSSProperties } from 'react';
import { GooFilter } from './GooFilter.js';

const PAD = 48;
const DROP = 18;

// Goo-Ebene hinter einem Popover: Beim Öffnen wächst eine Fläche vom Auslöser (`from`) auf die Größe des Popovers (`to`),
// erst in die Breite, dann (langsamer, mit Überschwingen) in die Höhe; drei Tropfen laufen voraus und werden vom Rand
// eingeholt. Mit `closing` läuft es rückwärts: erst die Höhe, dann die Breite, die Tropfen fließen zum Auslöser zurück.
// Der Popover bleibt dabei durchsichtig (CSS `bp-surface-in`/`bp-surface-out`); die Ebene blendet am Ende aus und meldet
// dann `onDone`. Reine Dekoration.
export function GooMorph({ from, to, closing = false, onDone }: { from: DOMRect; to: DOMRect; closing?: boolean; onDone: () => void }) {
  const left = Math.min(from.left, to.left) - PAD;
  const top = Math.min(from.top, to.top) - PAD;
  const width = Math.max(from.right, to.right) + PAD - left;
  const height = Math.max(from.bottom, to.bottom) + PAD - top;
  const below = to.top >= from.top;
  const startX = from.left + from.width / 2 - DROP / 2 - left;
  const startY = from.top + from.height / 2 - DROP / 2 - top;
  // Die Tropfen zielen auf das Ende der Fläche, das noch nachwächst, und werden dort eingeholt
  const endY = (below ? to.bottom - 28 - DROP : to.top + 28) - top;
  const drops = [0.25, 0.5, 0.75].map((at, i) => ({
    '--fx': `${startX}px`,
    '--fy': `${startY}px`,
    '--tx': `${to.left + to.width * at - DROP / 2 - left}px`,
    '--ty': `${endY}px`,
    '--delay': `${i * 50}ms`,
  })) as CSSProperties[];
  const blob = {
    left: to.left - left,
    top: to.top - top,
    width: to.width,
    height: to.height,
    '--dx': `${from.left - to.left}px`,
    '--dy': `${from.top - to.top}px`,
    '--sx': from.width / to.width,
    '--sy': from.height / to.height,
  } as CSSProperties;
  return (
    <div className={`goo-morph${closing ? ' is-closing' : ''}`} aria-hidden="true" style={{ left, top, width, height }} onAnimationEnd={(event) => event.target === event.currentTarget && onDone()}>
      <GooFilter id="goo-morph" blur={8} />
      <div className="goo-morph-shape">
        <span className="goo-morph-blob" style={blob}>
          <span className="goo-morph-body" />
        </span>
        {drops.map((style, i) => (
          <span key={i} className="goo-morph-drop" style={style} />
        ))}
      </div>
    </div>
  );
}
