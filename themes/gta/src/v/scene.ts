// Abendszene im Stil GTA V als Daten: zwei Reihen Hochhäuser, Fenster und Palmen auf einer Fläche 1440 × 900.
// Selbst gezeichnet nach der Preview; ein fester Zufallsstartwert sorgt dafür, dass die Szene immer gleich aussieht.

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

export interface SceneData {
  far: Rect[];
  near: Rect[];
  windows: Rect[];
  palms: Palm[];
}

export const SCENE_W = 1440;
export const SCENE_H = 900;

// mulberry32: kleiner, fester Zufallsgenerator
function random(seed: number): () => number {
  let a = seed;
  return () => {
    a = (a + 0x6d2b79f5) | 0;
    let t = Math.imul(a ^ (a >>> 15), 1 | a);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

const int = (n: number) => n | 0;
const fix = (n: number) => Math.round(n);

function farRow(next: () => number): Rect[] {
  const out: Rect[] = [];
  for (let x = -10; x < SCENE_W; ) {
    const w = 36 + next() * 60;
    const h = 60 + next() * 230 * (0.35 + 0.65 * Math.sin(x / 260 + 0.6) ** 2);
    out.push({ x: int(x), y: int(705 - h), w: int(w), h: int(h + 45) });
    x += w + next() * 10;
  }
  return out;
}

function nearRow(next: () => number): { near: Rect[]; windows: Rect[] } {
  const near: Rect[] = [];
  const windows: Rect[] = [];
  for (let x = -20; x < SCENE_W; ) {
    const w = 46 + next() * 86;
    const h = 40 + next() * 190 * (0.3 + 0.7 * Math.cos(x / 300 + 0.4) ** 2);
    const top = 745 - h;
    near.push({ x: int(x), y: int(top), w: int(w), h: int(h + 4) });
    for (let wy = top + 10; wy < 734; wy += 14) {
      for (let wx = x + 8; wx < x + w - 10; wx += 13) if (next() < 0.2) windows.push({ x: int(wx), y: int(wy), w: 5, h: 7 });
    }
    x += w + next() * 18;
  }
  return { near, windows };
}

// Palme: gebogener Stamm, acht Wedel als gefüllte Bögen, Krone als Punkt
function palm(x: number, y: number, h: number, lean: number): Palm {
  const tx = x + lean;
  const ty = y - h;
  const L = h * 0.36;
  const leaves = [-180, -160, -138, -116, -64, -42, -20, 0].map((deg) => {
    const r = (deg * Math.PI) / 180;
    const c = Math.cos(r);
    const s = Math.sin(r);
    const ex = tx + c * L;
    const ey = ty + s * L * 0.45 + L * 0.58;
    const cx = tx + c * L * 0.55;
    const cy = ty + s * L * 0.85 - L * 0.3;
    return `M${tx} ${ty}Q${fix(cx)} ${fix(cy)} ${fix(ex)} ${fix(ey)}Q${fix(cx + c * L * 0.04)} ${fix(cy + L * 0.3)} ${tx} ${ty}Z`;
  });
  return {
    trunk: `M${x} ${y}Q${x + lean * 0.15} ${y - h * 0.55} ${tx} ${ty}`,
    width: Math.round((h / 17) * 10) / 10,
    leaves,
    crown: { x: tx, y: ty, r: Math.round((h / 34) * 10) / 10 },
  };
}

function build(): SceneData {
  const next = random(11);
  const far = farRow(next);
  const { near, windows } = nearRow(next);
  const palms = [
    palm(96, 900, 560, 46),
    palm(232, 900, 400, -34),
    palm(1338, 900, 600, -56),
    palm(1210, 900, 350, 24),
    palm(640, 748, 150, 12),
    palm(704, 748, 118, -10),
    palm(420, 748, 132, 8),
  ];
  return { far, near, windows, palms };
}

let cached: SceneData | null = null;

/** Die Szene; einmal berechnet, danach aus dem Speicher */
export function sceneData(): SceneData {
  cached ??= build();
  return cached;
}
