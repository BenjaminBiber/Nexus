import { createContext, useContext, useRef, type PointerEvent, type KeyboardEvent } from 'react';

// Breite der Werkzeugspalte. Die Spalte selbst steht im DocViewer (die schwebenden Elemente hängen an dessen Layout),
// der gemerkte Wert aber an der Seite – deshalb reicht die Docs-Seite ihn über diesen Kontext hinunter.
export const DocsToolsContext = createContext<{ width: number; setWidth: (w: number) => void } | null>(null);

// Breite der Werkzeugspalte samt Setzer; null außerhalb der Docs-Werkbank
export function useDocsTools() {
  return useContext(DocsToolsContext);
}

// Ziehgriff zwischen zwei Spalten der Werkbank. `sign` sagt, in welche Richtung die Spalte wächst:
// 1 für eine Spalte links vom Griff (Explorer), -1 für eine Spalte rechts davon (Werkzeuge).
// Mit der Tastatur verschieben die Pfeile links und rechts um 16 Pixel.
export function DocsGutter({ label, width, min, max, sign, onChange }: {
  label: string;
  width: number;
  min: number;
  max: number;
  sign: 1 | -1;
  onChange: (width: number) => void;
}) {
  const drag = useRef<{ x: number; width: number } | null>(null);
  const clamp = (value: number) => Math.min(max, Math.max(min, Math.round(value)));

  const onPointerDown = (e: PointerEvent<HTMLDivElement>) => {
    drag.current = { x: e.clientX, width };
    e.currentTarget.setPointerCapture?.(e.pointerId);
  };

  const onPointerMove = (e: PointerEvent<HTMLDivElement>) => {
    if (drag.current) onChange(clamp(drag.current.width + sign * (e.clientX - drag.current.x)));
  };

  const onPointerUp = (e: PointerEvent<HTMLDivElement>) => {
    drag.current = null;
    e.currentTarget.releasePointerCapture?.(e.pointerId);
  };

  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    const step = e.key === 'ArrowLeft' ? -16 : e.key === 'ArrowRight' ? 16 : 0;
    if (step === 0) return;
    e.preventDefault();
    onChange(clamp(width + sign * step));
  };

  return (
    <div
      role="separator"
      aria-label={label}
      aria-orientation="vertical"
      aria-valuenow={width}
      aria-valuemin={min}
      aria-valuemax={max}
      tabIndex={0}
      className="docs-gutter"
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={onPointerUp}
      onPointerCancel={onPointerUp}
      onKeyDown={onKeyDown}
    />
  );
}
