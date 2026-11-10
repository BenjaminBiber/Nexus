// „Overdrive“: Wer im Rahmen den Konami-Code tippt (↑ ↑ ↓ ↓ ← → ← → B A), schaltet für ein paar Sekunden in den Turbo.
// Das Gitter rast, die Sterne ziehen Streifen, ein Wagen fährt durch die Deck-Leiste und das Display meldet „OVERDRIVE“.
// Während des Turbos steht data-synth-overdrive auf <html>; theme.css macht daraus das Bild. Eingaben in Feldern zählen
// nicht mit, bei reduzierter Bewegung passiert nichts.
import { useEffect, useState } from 'react';
import { reducedMotion } from './cassette.js';

export const KONAMI = ['ArrowUp', 'ArrowUp', 'ArrowDown', 'ArrowDown', 'ArrowLeft', 'ArrowRight', 'ArrowLeft', 'ArrowRight', 'b', 'a'];

/** Dauer des Turbos (ms); die Fahrt des Wagens (synth-car-drive in theme.css, 0,4 s + 3,2 s) endet vorher */
export const OVERDRIVE_MS = 6000;

const ATTRIBUTE = 'data-synth-overdrive';

function editable(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  return target.isContentEditable || ['INPUT', 'TEXTAREA', 'SELECT'].includes(target.tagName);
}

/** Nimmt die nächste Taste in die Folge auf; liefert die neue Position (KONAMI.length = Code komplett) */
export function advance(position: number, key: string): number {
  const normalized = key.length === 1 ? key.toLowerCase() : key;
  if (normalized === KONAMI[position]) return position + 1;
  // ein ↑ zu viel am Anfang bricht die Folge nicht ab
  if (normalized === 'ArrowUp' && position === 2) return 2;
  return normalized === KONAMI[0] ? 1 : 0;
}

/** Lauscht auf den Konami-Code und hält den Turbo OVERDRIVE_MS lang an; liefert, ob er gerade läuft */
export function useOverdrive(): boolean {
  const [active, setActive] = useState(false);

  useEffect(() => {
    let position = 0;
    const onKey = (event: KeyboardEvent) => {
      if (editable(event.target)) {
        position = 0;
        return;
      }
      position = advance(position, event.key);
      if (position < KONAMI.length) return;
      position = 0;
      if (!reducedMotion()) setActive(true);
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, []);

  useEffect(() => {
    if (!active) return;
    const root = document.documentElement;
    root.setAttribute(ATTRIBUTE, '');
    const timer = setTimeout(() => setActive(false), OVERDRIVE_MS);
    return () => {
      clearTimeout(timer);
      root.removeAttribute(ATTRIBUTE);
    };
  }, [active]);

  return active;
}
