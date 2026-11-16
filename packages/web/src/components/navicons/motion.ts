// Copyright (c) 2026 Kubai087. Licensed under the Kubai Icons License; see LICENSE.
// Bewegungskern der Kubai Icons für die Linien-Symbole der Navigation: zwei Schlüsselbilder (Ruhe, Überfahren),
// 460 ms mit cubic-bezier(.22,1,.36,1), Zahl für Zahl interpoliert und immer vom aktuellen Bild aus gestartet,
// damit ein Richtungswechsel mitten in der Bewegung weich weiterläuft

export type Params = Record<string, number>;
export type Els = Record<string, SVGElement>;

// Ein Symbol beschreibt nur seine Zahlen je Zustand und wie sie in die markierten Elemente (`data-n`) geschrieben werden
export interface Motion {
  target: (hover: boolean) => Params;
  draw: (p: Params, e: Els) => void;
}

export interface MotionInst {
  motion: Motion;
  e: Els;
  cur: Params;
  raf: number;
  hover: boolean;
}

function bezier(x1: number, y1: number, x2: number, y2: number) {
  const cx = 3 * x1, bx = 3 * (x2 - x1) - cx, ax = 1 - cx - bx;
  const cy = 3 * y1, by = 3 * (y2 - y1) - cy, ay = 1 - cy - by;
  const sx = (t: number) => ((ax * t + bx) * t + cx) * t;
  const sy = (t: number) => ((ay * t + by) * t + cy) * t;
  const dx = (t: number) => (3 * ax * t + 2 * bx) * t + cx;
  return (x: number) => {
    if (x <= 0) return 0;
    if (x >= 1) return 1;
    let t = x;
    for (let i = 0; i < 8; i++) {
      const e = sx(t) - x;
      if (Math.abs(e) < 1e-5) break;
      const d = dx(t);
      if (Math.abs(d) < 1e-6) break;
      t -= e / d;
    }
    return sy(t);
  };
}

export const EASE = bezier(0.22, 1, 0.36, 1);
export const DUR = 460;

// Zahlen kürzen, damit die Pfade nicht mit 15 Nachkommastellen geschrieben werden
export const f = (n: number) => +n.toFixed(3);

function interp(a: Params, b: Params, t: number): Params {
  const o: Params = {};
  for (const k in b) o[k] = (a[k] ?? b[k]) + (b[k] - (a[k] ?? b[k])) * t;
  return o;
}

// Wer weniger Bewegung wünscht oder keine Bildwiederholung hat (Tests, Hintergrund-Rendering), springt ans Ziel
function reduced() {
  if (typeof requestAnimationFrame !== 'function') return true;
  try {
    return typeof matchMedia === 'function' && matchMedia('(prefers-reduced-motion: reduce)').matches;
  } catch {
    return false;
  }
}

export function jump(inst: MotionInst) {
  if (inst.raf) cancelAnimationFrame(inst.raf);
  inst.raf = 0;
  inst.cur = inst.motion.target(inst.hover);
  inst.motion.draw(inst.cur, inst.e);
}

// Startet bei dem, was gerade zu sehen ist
export function go(inst: MotionInst) {
  if (reduced()) return jump(inst);
  const to = inst.motion.target(inst.hover);
  if (inst.raf) cancelAnimationFrame(inst.raf);
  const from = inst.cur, t0 = performance.now();
  const tick = (now: number) => {
    const t = Math.min(1, Math.max(0, (now - t0) / DUR));
    inst.cur = interp(from, to, EASE(t));
    inst.motion.draw(inst.cur, inst.e);
    inst.raf = t < 1 ? requestAnimationFrame(tick) : 0;
  };
  inst.raf = requestAnimationFrame(tick);
}

export function stop(inst: MotionInst) {
  if (inst.raf) cancelAnimationFrame(inst.raf);
  inst.raf = 0;
}
