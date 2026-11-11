// Zahl, die bei Änderung in kleinen Schritten zum neuen Wert läuft (Kontostand im HUD); bei reduzierter Bewegung springt sie.
import { useEffect, useRef, useState } from 'react';
import { reducedMotion } from './motion.js';

/** Dauer des Hochzählens (ms) */
export const COUNT_MS = 1200;
const STEP_MS = 40;

/**
 * Angezeigter Wert und ob gerade gezählt wird. `live` = es gibt einen Board-Stand; gezählt wird nur zwischen zwei
 * geladenen Ständen. Der erste Stand springt, ebenso der nach einem Board-Wechsel (der Feed meldet dazwischen keinen
 * Stand) – sonst sähe das Geld des Boards aus wie gerade verdient.
 */
export function useCountUp(target: number, live = true): { value: number; counting: boolean } {
  const [state, setState] = useState({ value: target, counting: false });
  const shown = useRef(target);
  const wasLive = useRef(live);

  useEffect(() => {
    const from = shown.current;
    const counts = live && wasLive.current;
    wasLive.current = live;
    if (from === target) return;
    if (!counts || reducedMotion()) {
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
  }, [target, live]);

  // Springt der Wert, steht er schon in diesem Render da (wasLive ändert erst der Effekt nach dem Zeichnen) – sonst
  // zeigte ein Frame noch den alten Stand. Vor dem ersten Schritt schon als „zählt“ melden, damit die Farbe wechselt.
  if (!live || !wasLive.current || state.value === target) return { value: target, counting: false };
  return { value: state.value, counting: true };
}
