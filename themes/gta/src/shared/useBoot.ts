// Ablauf des Ladebildschirms, gleich in beiden Stilen: läuft BOOT_MS lang, jede Taste oder ein Klick beendet ihn
// sofort, und onDone kommt genau einmal. Ausgeschaltet (Schalter aus localStorage, der Ladebildschirm läuft vor dem
// Host) oder bei reduzierter Bewegung endet er sofort und unsichtbar.
import { useEffect, useRef, useState } from 'react';
import { rememberedBoot } from '../settings.js';
import { reducedMotion } from './motion.js';

/** Dauer des Ladebildschirms (ms) */
export const BOOT_MS = 3200;

/** Startet den Ablauf; liefert, ob der Ladebildschirm zu sehen ist */
export function useBoot(onDone: () => void): boolean {
  const [enabled] = useState(() => rememberedBoot() && !reducedMotion());
  const done = useRef(false);

  useEffect(() => {
    const finish = () => {
      if (done.current) return;
      done.current = true;
      onDone();
    };
    if (!enabled) {
      finish();
      return;
    }
    const timer = window.setTimeout(finish, BOOT_MS);
    window.addEventListener('keydown', finish);
    window.addEventListener('pointerdown', finish);
    return () => {
      window.clearTimeout(timer);
      window.removeEventListener('keydown', finish);
      window.removeEventListener('pointerdown', finish);
    };
  }, [enabled, onDone]);

  return enabled;
}

/** Ein zufälliger Tipp, fest für die Lebensdauer der Komponente */
export function useTip(tips: readonly string[]): string {
  const [tip] = useState(() => tips[Math.min(tips.length - 1, Math.floor(Math.random() * tips.length))]!);
  return tip;
}
