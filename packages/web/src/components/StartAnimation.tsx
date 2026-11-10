import { useCallback, useEffect, useLayoutEffect, useState } from 'react';
import type { StartAnimationProps } from '@nexus/theme-sdk';
import { getTheme } from '../theme/registry.js';
import { displayedTheme } from '../theme/theme.js';
import { SlotBoundary } from '../theme/SlotBoundary.js';

/** Spätestens nach dieser Zeit endet die Startanimation eines Themes, auch ohne `onDone` */
export const START_ANIMATION_MAX_MS = 8000;

// Startanimation des Themes (Slot „StartAnimation“) als Vollbild über der App; ohne Slot bleibt das Overlay aus index.html.
// Gilt nur für das Theme beim Laden der Seite; ein späterer Wechsel zeigt keine Startanimation. Bei reduzierter Bewegung entfällt sie.
export function StartAnimation() {
  const [start] = useState(() => {
    const theme = displayedTheme();
    const Custom = prefersReducedMotion() ? undefined : getTheme(theme)?.slots.StartAnimation;
    return Custom ? { theme, Custom } : null;
  });
  const [done, setDone] = useState(false);
  const finish = useCallback(() => setDone(true), []);

  useEffect(() => {
    if (!start) return;
    const timer = setTimeout(finish, START_ANIMATION_MAX_MS);
    return () => clearTimeout(timer);
  }, [start, finish]);

  if (!start || done) return null;
  const { theme, Custom } = start;
  return (
    <div className="theme-start-anim" aria-hidden="true">
      <SlotBoundary fallback={<Finish onFinish={finish} />} resetKey={theme}>
        <Custom onDone={finish} />
        <TakeOver />
      </SlotBoundary>
    </div>
  );
}

// Entfernt das Standard-Overlay; wird nur committet, wenn die Theme-Komponente fehlerfrei rendert
function TakeOver() {
  useLayoutEffect(() => {
    document.getElementById('start-anim')?.remove();
  }, []);
  return null;
}

// Rückfall bei einem Fehler: beendet die Startphase; wurde noch nicht übernommen, läuft das Standard-Overlay normal zu Ende
function Finish({ onFinish }: { onFinish: StartAnimationProps['onDone'] }) {
  useEffect(() => onFinish(), [onFinish]);
  return null;
}

// Bevorzugt das System reduzierte Bewegung? (ohne matchMedia, z. B. in Tests: nein)
function prefersReducedMotion(): boolean {
  return typeof window.matchMedia === 'function' && window.matchMedia('(prefers-reduced-motion: reduce)').matches;
}
