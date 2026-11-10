// Die Kassette im Deck: Seite A oder B. Umdrehen geht über „SIDE A“ in der Titelliste oder von selbst per Auto-Reverse
// (Bandzähler mit VHS, nach 45 Minuten wie bei einer C90). Seite B ist die Nachtfahrt: der Himmel wird Nacht, die Titel
// heißen B1, B2 … Die Seite bleibt über das Neuladen hinaus liegen (localStorage), wie eine Kassette im Deck.
import { useEffect, useState } from 'react';

export type CassetteSide = 'A' | 'B';

export const SIDE_KEY = 'nexus-synthwave-side';

/** Spielzeit einer Seite bei einer C90 (ms); danach dreht Auto-Reverse die Kassette */
export const SIDE_MS = 45 * 60 * 1000;

const listeners = new Set<() => void>();

function readSide(): CassetteSide {
  try {
    return window.localStorage.getItem(SIDE_KEY) === 'B' ? 'B' : 'A';
  } catch {
    return 'A';
  }
}

let current: CassetteSide | null = null;

function snapshot(): CassetteSide {
  if (current === null) current = readSide();
  return current;
}

/** Legt die Seite fest und merkt sie sich */
export function setSide(side: CassetteSide): void {
  current = side;
  try {
    window.localStorage.setItem(SIDE_KEY, side);
  } catch {
    // privates Fenster: dann nur für diese Sitzung
  }
  for (const listener of listeners) listener();
}

/** Dreht die Kassette um */
export function flipSide(): void {
  setSide(snapshot() === 'A' ? 'B' : 'A');
}

/** Nur für Tests: vergisst die gemerkte Seite */
export function resetSide(): void {
  current = null;
}

function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

/** Aktuelle Seite der Kassette (folgt jedem Umdrehen) */
export function useCassetteSide(): CassetteSide {
  const [side, setState] = useState<CassetteSide>(snapshot);
  useEffect(() => {
    const update = () => setState(snapshot());
    update();
    return subscribe(update);
  }, []);
  return side;
}

/** Bevorzugt der Nutzer reduzierte Bewegung? (ohne matchMedia: nein) */
export function reducedMotion(): boolean {
  try {
    return window.matchMedia('(prefers-reduced-motion: reduce)').matches;
  } catch {
    return false;
  }
}
