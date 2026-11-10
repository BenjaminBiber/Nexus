import { useEffect, useState } from 'react';
import { reducedMotion } from './cassette.js';

/** Pause bis zur nächsten Sternschnuppe: zufällig zwischen 20 und 50 s */
export const nextShootingStar = (random: number = Math.random()) => 20_000 + random * 30_000;

/** So lange liegt eine Sternschnuppe im DOM (ms); passt zu synth-shoot in theme.css */
export const SHOOTING_MS = 1400;

interface Shot {
  id: number;
  top: number;
  left: number;
}

// Ab und zu eine Sternschnuppe: Position zufällig im oberen Himmel. Sichtbar ist sie nur, wo Sterne leuchten (nachts,
// abgeschwächt in der Dämmerung) – theme.css koppelt sie an --synth-stars-opacity. Ohne Bewegung gibt es keine.
function useShootingStar(): Shot | null {
  const [shot, setShot] = useState<Shot | null>(null);

  useEffect(() => {
    if (reducedMotion()) return;
    let timer = 0;
    let id = 0;
    const schedule = () => {
      timer = window.setTimeout(() => {
        id += 1;
        setShot({ id, top: 4 + Math.random() * 26, left: 15 + Math.random() * 60 });
        timer = window.setTimeout(() => {
          setShot(null);
          schedule();
        }, SHOOTING_MS);
      }, nextShootingStar());
    };
    schedule();
    return () => window.clearTimeout(timer);
  }, []);

  return shot;
}

// Ebene hinter dem Inhalt: Sterne (mit gelegentlicher Sternschnuppe), Mond, Sonne und perspektivisches Neon-Gitter auf
// dem Boden; Aussehen je Phase und Stufe regelt theme.css. Der Boden schneidet das gekippte Gitter an der Horizontlinie ab.
export function Horizon() {
  const shot = useShootingStar();
  return (
    <div className="synth-horizon" aria-hidden="true">
      <div className="synth-stars" />
      {shot && <i key={shot.id} className="synth-shooting-star" style={{ top: `${shot.top}%`, left: `${shot.left}%` }} />}
      <div className="synth-moon" />
      <div className="synth-sun" />
      <div className="synth-ground">
        <div className="synth-grid" />
      </div>
    </div>
  );
}
