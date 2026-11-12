// Gemeinsames der beiden Szenen (Abendszene GTA V, Vorort San Andreas): Fläche, Formen und ein fester Zufallsgenerator,
// damit jede Szene immer gleich aussieht.

export interface Rect {
  x: number;
  y: number;
  w: number;
  h: number;
}

export interface Palm {
  trunk: string;
  width: number;
  leaves: string[];
  crown: { x: number; y: number; r: number };
}

export const SCENE_W = 1440;
export const SCENE_H = 900;

/** mulberry32: kleiner Zufallsgenerator mit festem Startwert */
export function random(seed: number): () => number {
  let a = seed;
  return () => {
    a = (a + 0x6d2b79f5) | 0;
    let t = Math.imul(a ^ (a >>> 15), 1 | a);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}
