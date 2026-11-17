import { useEffect, useRef, type MouseEvent, type RefObject } from 'react';

// So lange wartet ein Wechsel, solange der Zeiger auf die Zielspalte zuläuft (ms)
const AIM_DELAY = 300;
// Spielraum über und unter der Zielspalte (px), damit auch ein etwas zu steiler Weg noch zählt
const AIM_TOLERANCE = 20;
// Aus so vielen letzten Zeigerpositionen wird die Richtung bestimmt
const TRAIL = 3;

interface Point {
  x: number;
  y: number;
}

// Ob der Weg von `from` nach `to` auf die linke Kante einer Spalte rechts davon zuläuft: `to` liegt dann im Dreieck aus
// `from` und den beiden Ecken der Kante (die Steigung zur oberen Ecke fällt, die zur unteren steigt)
export function headsToward(from: Point, to: Point, target: { left: number; top: number; bottom: number }): boolean {
  if (to.x <= from.x || to.x >= target.left) return false;
  const slope = (a: Point, b: Point) => (b.y - a.y) / (b.x - a.x);
  const upper = { x: target.left, y: target.top - AIM_TOLERANCE };
  const lower = { x: target.left, y: target.bottom + AIM_TOLERANCE };
  return slope(to, upper) < slope(from, upper) && slope(to, lower) > slope(from, lower);
}

// Menü mit Untermenü rechts daneben (Projekte › Boards): Überfährt der Zeiger auf dem Weg zum Untermenü eine andere Zeile,
// wechselt die nicht sofort. Bleibt er auf ihr stehen oder läuft er nicht mehr aufs Untermenü zu, wechselt sie doch;
// verlässt er die Zeilen (`cancel`), bleibt die bisherige stehen. `track` gehört an die Mausbewegung des ganzen Menüs.
export function useMenuAim(target: RefObject<HTMLElement | null>, activate: (key: string) => void) {
  const trail = useRef<Point[]>([]);
  const timer = useRef<ReturnType<typeof setTimeout>>(undefined);
  // Wo der Zeiger beim letzten Aufschub stand: steht er noch dort, ist er angekommen
  const pausedAt = useRef<Point | null>(null);
  const latest = useRef(activate);
  latest.current = activate;
  useEffect(() => () => clearTimeout(timer.current), []);

  const aiming = () => {
    const rect = target.current?.getBoundingClientRect();
    const from = trail.current[0];
    const to = trail.current[trail.current.length - 1];
    if (!rect || !from || !to) return false;
    if (pausedAt.current && pausedAt.current.x === to.x && pausedAt.current.y === to.y) return false;
    pausedAt.current = headsToward(from, to, rect) ? to : null;
    return pausedAt.current !== null;
  };
  const enter = (key: string) => {
    clearTimeout(timer.current);
    if (aiming()) timer.current = setTimeout(() => enter(key), AIM_DELAY);
    else latest.current(key);
  };
  return {
    track: (event: MouseEvent) => {
      trail.current = [...trail.current.slice(1 - TRAIL), { x: event.clientX, y: event.clientY }];
    },
    enter,
    cancel: () => clearTimeout(timer.current),
  };
}
