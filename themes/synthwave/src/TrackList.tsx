import { useEffect, useRef, useState } from 'react';
import { NavLink, useNav } from '@nexus/theme-sdk';
import { flipSide, useCassetteSide, type CassetteSide } from './cassette.js';

// Titelnummer wie auf einer Kassettenhülle: Seite und Stelle, also A1, A2 … bzw. B1, B2 …
export const trackNo = (side: CassetteSide, index: number) => `${side}${index + 1}`;

/** Dauer der Drehung beim Umdrehen (ms); passt zu synth-flip in theme.css */
export const FLIP_MS = 500;

// Nur ein echter Seitenwechsel dreht die Liste, nicht das erste Zeichnen mit gemerkter Seite B
function useFlipping(side: CassetteSide): boolean {
  const last = useRef(side);
  const [flipping, setFlipping] = useState(false);
  useEffect(() => {
    if (side === last.current) return;
    last.current = side;
    setFlipping(true);
    const timer = setTimeout(() => setFlipping(false), FLIP_MS);
    return () => clearTimeout(timer);
  }, [side]);
  return flipping;
}

// Navigation als Titelliste der eingelegten Kassette: je Eintrag ein Link des Hosts mit Titelnummer, Beschriftung und
// Zähler; der laufende Titel (aktiver Eintrag) trägt den Equalizer. Nummer und Equalizer sind Schmuck und für
// Screenreader verborgen. „SIDE A“ oben dreht die Kassette um (Seite B: Nachtfahrt).
export function TrackList() {
  const items = useNav();
  const side = useCassetteSide();
  const flipping = useFlipping(side);
  return (
    <nav className="nav synth-tracks" aria-label="Hauptnavigation" data-side={side} data-flipping={flipping ? '' : undefined}>
      <button
        type="button"
        className="synth-side"
        aria-label={`Kassette umdrehen, jetzt Seite ${side}`}
        title="Kassette umdrehen"
        onClick={flipSide}
      >
        SIDE {side}
      </button>
      {items.map((entry, index) => (
        <NavLink key={entry.id} id={entry.id} className={`synth-track${entry.active ? ' active' : ''}`}>
          {(item) => (
            <>
              <span className="synth-track-no" aria-hidden="true">
                {trackNo(side, index)}
              </span>
              <span className="nav-label">{item.label}</span>
              {item.badge ? <span className="nav-badge">{item.badge}</span> : null}
              {item.active && (
                <span className="synth-eq" aria-hidden="true">
                  <i />
                  <i />
                  <i />
                </span>
              )}
            </>
          )}
        </NavLink>
      ))}
    </nav>
  );
}
