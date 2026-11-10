import { useEffect, useRef, useState } from 'react';
import type { StartAnimationProps } from '@nexus/theme-sdk';

// Startanimation läuft vor dem Rahmen und außerhalb des ThemeHostProviders: Den Schalter „Startanimation“ und das
// Farbschema spiegelt die Shell deshalb nach localStorage. Beides darf fehlen.
export const BOOT_KEY = 'nexus-synthwave-boot';
export const PALETTE_KEY = 'nexus-synthwave-palette';

/** Dauer der ganzen Szene (ms); alle Keyframes in theme.css enden spätestens hier */
export const SUNRISE_MS = 1800;

function read(key: string): string | null {
  try {
    return window.localStorage.getItem(key);
  } catch {
    return null;
  }
}

function write(key: string, value: string): void {
  try {
    window.localStorage.setItem(key, value);
  } catch {
    // privates Fenster oder gesperrter Speicher: dann eben ohne
  }
}

/** Merkt sich, ob die Startanimation laufen soll (die Shell ruft das mit der Einstellung „boot“) */
export function rememberBoot(enabled: boolean): void {
  write(BOOT_KEY, enabled ? 'on' : 'off');
}

/** Merkt sich das Farbschema, damit die Szene vor der Shell schon in den richtigen Farben läuft */
export function rememberPalette(palette: string): void {
  write(PALETTE_KEY, palette);
}

function reducedMotion(): boolean {
  try {
    return window.matchMedia('(prefers-reduced-motion: reduce)').matches;
  } catch {
    return false;
  }
}

// Startanimation „Sunrise Drive“: Das Gitter fährt heran, die Sonne steigt auf, der Schriftzug erscheint in Chrom.
// Jede Taste oder ein Klick beendet sie sofort; ausgeschaltet oder bei reduzierter Bewegung endet sie unsichtbar.
export function SunriseDrive({ onDone }: StartAnimationProps) {
  const [enabled] = useState(() => read(BOOT_KEY) !== 'off' && !reducedMotion());
  const [palette] = useState(() => read(PALETTE_KEY) ?? undefined);
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
    const timer = window.setTimeout(finish, SUNRISE_MS);
    window.addEventListener('keydown', finish);
    window.addEventListener('pointerdown', finish);
    return () => {
      window.clearTimeout(timer);
      window.removeEventListener('keydown', finish);
      window.removeEventListener('pointerdown', finish);
    };
  }, [enabled, onDone]);

  if (!enabled) return null;
  return (
    <div className="synth-sunrise" data-testid="synth-sunrise" data-synth-palette={palette}>
      <div className="synth-sunrise-sky" />
      <div className="synth-sunrise-sun" />
      <div className="synth-sunrise-grid" />
      <div className="synth-sunrise-title">
        <span className="synth-sunrise-word">NEXUS</span>
      </div>
      <div className="synth-sunrise-skip">PRESS START</div>
    </div>
  );
}
