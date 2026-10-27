// Rechnung für das Diagramm-Vollbild (reine Funktionen). Eine Ansicht ist { scale, x, y }: Maßstab und Lage der linken
// oberen Ecke des Diagramms auf der Bühne in Pixeln; gezeichnet als translate(x, y) scale(scale) mit transform-origin 0 0.

export interface View { scale: number; x: number; y: number }
export interface Size { w: number; h: number }

export const MIN_SCALE = 0.1;
export const MAX_SCALE = 8;
/** Abstand zum Rand der Bühne beim Einpassen, in Pixeln */
export const FIT_PADDING = 24;
/** Ein Schritt über Knopf oder Taste */
export const ZOOM_STEP = 1.25;

// Maßstab innerhalb der Grenzen; Unsinn wird 1
export function clampScale(scale: number): number {
  if (!Number.isFinite(scale)) return 1;
  return Math.min(MAX_SCALE, Math.max(MIN_SCALE, scale));
}

// Diagramm mit diesem Maßstab mittig auf die Bühne legen
export function centerView(content: Size, box: Size, scale: number): View {
  const s = clampScale(scale);
  return { scale: s, x: (box.w - content.w * s) / 2, y: (box.h - content.h * s) / 2 };
}

// Einpassen: ganz sichtbar, mittig, mit Rand; kleine Diagramme werden dabei vergrößert (höchstens MAX_SCALE)
export function fitView(content: Size, box: Size, padding = FIT_PADDING): View {
  const w = Math.max(1, box.w - 2 * padding);
  const h = Math.max(1, box.h - 2 * padding);
  return centerView(content, box, Math.min(w / Math.max(1, content.w), h / Math.max(1, content.h)));
}

// Um einen Punkt der Bühne zoomen: was unter dem Punkt liegt, bleibt dort
export function zoomAt(view: View, factor: number, point: { x: number; y: number }): View {
  const scale = clampScale(view.scale * factor);
  const ratio = scale / view.scale;
  return { scale, x: point.x - (point.x - view.x) * ratio, y: point.y - (point.y - view.y) * ratio };
}

// Radweg in einen Zoomfaktor: eine Raststufe (100 px) sind 15 %; Zeilen (1) und Seiten (2) vorher in Pixel
export function wheelFactor(deltaY: number, deltaMode = 0): number {
  const px = deltaMode === 1 ? deltaY * (100 / 3) : deltaMode === 2 ? deltaY * 100 : deltaY;
  return 1.15 ** (-px / 100);
}

// Natürliche Größe eines SVG: viewBox, sonst width/height, sonst 800 × 600
export function naturalSize(svg: string): Size {
  const box = /viewBox\s*=\s*"\s*[-\d.]+[\s,]+[-\d.]+[\s,]+([\d.]+)[\s,]+([\d.]+)\s*"/.exec(svg);
  if (box) return { w: Number(box[1]) || 1, h: Number(box[2]) || 1 };
  const w = /\swidth\s*=\s*"([\d.]+)"/.exec(svg);
  const h = /\sheight\s*=\s*"([\d.]+)"/.exec(svg);
  return { w: w ? Number(w[1]) : 800, h: h ? Number(h[1]) : 600 };
}
