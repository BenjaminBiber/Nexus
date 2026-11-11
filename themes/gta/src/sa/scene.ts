// Vorort bei Sonnenuntergang im Stil San Andreas als Daten: Hochhäuser in der Ferne, Häuser mit Dächern und Fenstern,
// Strommasten mit Leitungen, Fahrbahnmarkierung und Palmen auf einer Fläche 1440 × 900. Selbst gezeichnet nach der
// Preview; ein fester Zufallsstartwert sorgt dafür, dass die Szene immer gleich aussieht.
import { random, type Palm, type Rect } from '../shared/scene.js';

export interface SaSceneData {
  far: Rect[];
  houses: string[]; // Pfade: Hauskörper und Dach
  windows: Rect[];
  poles: { d: string; width: number }[];
  dashes: Rect[];
  palms: Palm[];
}

const int = (n: number) => n | 0;
const fix = (n: number) => Math.round(n);

// Hochhäuser der Innenstadt hinter dem Hügel, in der Mitte am höchsten
function farRow(next: () => number): Rect[] {
  const out: Rect[] = [];
  for (let x = 150; x < 720; ) {
    const w = 24 + next() * 40;
    const h = 34 + next() * 60 + 210 * Math.exp(-(((x - 420) / 120) ** 2)) * (0.55 + next() * 0.45);
    out.push({ x: int(x), y: int(640 - h), w: int(w), h: int(h + 10) });
    x += w + next() * 9;
  }
  return out;
}

// Flache Häuser, gut die Hälfte mit Satteldach, je bis zu drei erleuchtete Fenster
function houseRow(next: () => number): { houses: string[]; windows: Rect[] } {
  const houses: string[] = [];
  const windows: Rect[] = [];
  for (let x = -40; x < 1480; ) {
    const w = 118 + next() * 96;
    const h = 44 + next() * 24;
    const top = 718 - h;
    const xi = int(x);
    houses.push(`M${xi} ${int(top)}h${int(w)}v${int(h + 4)}h-${int(w)}z`);
    houses.push(
      next() < 0.62
        ? `M${xi - 12} ${int(top + 2)}L${int(x + w / 2)} ${int(top - 30 - next() * 16)}L${int(x + w + 12)} ${int(top + 2)}Z`
        : `M${xi - 7} ${int(top - 8)}h${int(w + 14)}v11h-${int(w + 14)}z`,
    );
    for (let i = 0; i < 3; i++) if (next() < 0.5) windows.push({ x: int(x + 14 + (i * (w - 44)) / 2), y: int(top + 14), w: 17, h: 14 });
    x += w + 16 + next() * 70;
  }
  return { houses, windows };
}

// Strommasten mit zwei Querträgern und durchhängenden Leitungen dazwischen
function poleRow(): { d: string; width: number }[] {
  const xs = [118, 452, 800, 1150, 1496];
  const out: { d: string; width: number }[] = [];
  xs.forEach((x, i) => {
    out.push({ d: `M${x} 724V446`, width: 7 });
    out.push({ d: `M${x - 36} 466H${x + 36}M${x - 26} 488H${x + 26}`, width: 5 });
    const prev = xs[i - 1];
    if (prev === undefined) return;
    for (const [dx, y, sag] of [[-34, 466, 44], [34, 466, 40], [-24, 488, 50], [24, 488, 46]] as const) {
      out.push({ d: `M${prev + dx} ${y}Q${(prev + x) / 2 + dx} ${y + sag} ${x + dx} ${y}`, width: 1.7 });
    }
  });
  out.push({ d: 'M-200 506Q-40 520 84 466M-200 520Q-40 540 94 488', width: 1.7 });
  return out;
}

// Palme wie in der Preview: gebogener Stamm, acht Wedel, Krone als Punkt
function palm(x: number, y: number, h: number, lean: number, L: number): Palm {
  const tx = x + lean;
  const ty = y - h;
  const leaves = [-180, -158, -134, -110, -70, -46, -22, 0].map((deg) => {
    const r = (deg * Math.PI) / 180;
    const c = Math.cos(r);
    const s = Math.sin(r);
    const ex = tx + c * L;
    const ey = ty + s * L * 0.45 + L * 0.62;
    const cx = tx + c * L * 0.55;
    const cy = ty + s * L * 0.85 - L * 0.3;
    return `M${tx} ${ty}Q${fix(cx)} ${fix(cy)} ${fix(ex)} ${fix(ey)}Q${fix(cx + c * L * 0.04)} ${fix(cy + L * 0.32)} ${tx} ${ty}Z`;
  });
  return {
    trunk: `M${x} ${y}Q${x + lean * 0.2} ${y - h * 0.55} ${tx} ${ty}`,
    width: Math.round(Math.max(4, h / 40) * 10) / 10,
    leaves,
    crown: { x: tx, y: ty, r: Math.round((L / 9) * 10) / 10 },
  };
}

function build(): SaSceneData {
  const next = random(29);
  const far = farRow(next);
  const { houses, windows } = houseRow(next);
  const dashes: Rect[] = [];
  for (let x = 20; x < 1440; x += 150) dashes.push({ x, y: 812, w: 76, h: 6 });
  const palms = [
    palm(250, 724, 470, 22, 74),
    palm(322, 724, 380, -16, 62),
    palm(1296, 724, 520, -26, 80),
    palm(1372, 724, 400, 14, 64),
    palm(596, 724, 250, 8, 46),
    palm(1010, 724, 290, -10, 50),
  ];
  return { far, houses, windows, poles: poleRow(), dashes, palms };
}

let cached: SaSceneData | null = null;

/** Die Szene; einmal berechnet, danach aus dem Speicher */
export function saSceneData(): SaSceneData {
  cached ??= build();
  return cached;
}
