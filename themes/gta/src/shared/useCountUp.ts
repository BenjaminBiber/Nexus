// Zahl, die bei Änderung in kleinen Schritten zum neuen Wert läuft (Kontostand im HUD); bei reduzierter Bewegung springt sie.
import { useEffect, useRef, useState } from 'react';
import { reducedMotion } from './motion.js';

/** Dauer des Hochzählens (ms) */
export const COUNT_MS = 1200;
const STEP_MS = 40;

/** Angezeigter Wert und ob gerade gezählt wird */
export function useCountUp(target: number): { value: number; counting: boolean } {
  const [state, setState] = useState({ value: target, counting: false });
  const shown = useRef(target);

  useEffect(() => {
    const from = shown.current;
    if (from === target) return;
    if (reducedMotion()) {
      shown.current = target;
      setState({ value: target, counting: false });
      return;
    }
    const start = Date.now();
    const timer = setInterval(() => {
      const k = Math.min(1, (Date.now() - start) / COUNT_MS);
      shown.current = from + (target - from) * k;
      setState({ value: shown.current, counting: k < 1 });
      if (k >= 1) clearInterval(timer);
    }, STEP_MS);
    return () => clearInterval(timer);
  }, [target]);

  // Vor dem ersten Schritt schon als „zählt“ melden, damit die Farbe sofort wechselt
  return state.value === target ? { value: target, counting: false } : { value: state.value, counting: true };
}
