import { useEffect, useState } from 'react';
import { reducedMotion } from './cassette.js';

const WORD = 'Nexus';

/** Dauer eines Aussetzers (ms); passt zu synth-tube in theme.css */
export const FLICKER_MS = 900;

/** Pause bis zum nächsten Aussetzer: zufällig zwischen 25 und 60 s */
export const nextFlickerDelay = (random: number = Math.random()) => 25_000 + random * 35_000;

// Schriftzug als alte Neonröhre: Buchstaben einzeln, damit ab und zu einer kurz aussetzt und wieder anspringt – wie ein
// Schild, dessen Röhre schon ein paar Jahre hinter sich hat. Bei reduzierter Bewegung brennt die Röhre einfach durch.
export function NeonLogo() {
  const [flicker, setFlicker] = useState<number | null>(null);

  useEffect(() => {
    if (reducedMotion()) return;
    let timer = 0;
    const schedule = () => {
      timer = window.setTimeout(() => {
        setFlicker(Math.floor(Math.random() * WORD.length));
        timer = window.setTimeout(() => {
          setFlicker(null);
          schedule();
        }, FLICKER_MS);
      }, nextFlickerDelay());
    };
    schedule();
    return () => window.clearTimeout(timer);
  }, []);

  return (
    <span className="synth-logo synth-script" aria-hidden="true">
      {[...WORD].map((letter, index) => (
        <span key={index} className="synth-logo-letter" data-flicker={index === flicker ? '' : undefined}>
          {letter}
        </span>
      ))}
    </span>
  );
}
