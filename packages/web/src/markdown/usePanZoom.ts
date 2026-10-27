import { useCallback, useEffect, useLayoutEffect, useRef, useState, type KeyboardEvent, type PointerEvent } from 'react';
import { ZOOM_STEP, centerView, fitView, wheelFactor, zoomAt, type Size, type View } from './diagramView.js';

// Zoom und Verschieben einer Bühne: eingepasst beim Start und bei Fenstergröße, Mausrad um den Zeiger, +/−/0, Ziehen.
// Liefert die Ansicht und die Handler für das Bühnen-Element (Ref, Tasten, Zeiger).
export function usePanZoom(content: Size) {
  const stage = useRef<HTMLDivElement>(null);
  const drag = useRef<{ x: number; y: number } | null>(null);
  const [view, setView] = useState<View>({ scale: 1, x: 0, y: 0 });

  // Größe der Bühne in Pixeln (ohne Layout, z. B. in Tests, 0 × 0)
  const box = useCallback(() => {
    const rect = stage.current?.getBoundingClientRect();
    return { w: rect?.width ?? 0, h: rect?.height ?? 0 };
  }, []);
  const fit = useCallback(() => setView(fitView(content, box())), [content, box]);

  // Zoomt um die Mitte der Bühne
  const zoomCentered = (factor: number) => {
    const b = box();
    setView((v) => zoomAt(v, factor, { x: b.w / 2, y: b.h / 2 }));
  };

  // Natürliche Größe, mittig
  const actualSize = () => setView(centerView(content, box(), 1));

  // Einpassen vor dem ersten Zeichnen (kein Bild in 1:1 vorab)
  useLayoutEffect(() => {
    fit();
  }, [fit]);

  // Neu einpassen bei geänderter Fenstergröße
  useEffect(() => {
    window.addEventListener('resize', fit);
    return () => window.removeEventListener('resize', fit);
  }, [fit]);

  // Mausrad zoomt um den Zeiger; als eigener, nicht passiver Listener, damit preventDefault den Browser-Zoom (Strg+Mausrad) und das Scrollen verhindert
  useEffect(() => {
    const node = stage.current;
    if (!node) return;
    const onWheel = (event: WheelEvent) => {
      event.preventDefault();
      const rect = node.getBoundingClientRect();
      const factor = wheelFactor(event.deltaY, event.deltaMode);
      setView((v) => zoomAt(v, factor, { x: event.clientX - rect.left, y: event.clientY - rect.top }));
    };
    node.addEventListener('wheel', onWheel, { passive: false });
    return () => node.removeEventListener('wheel', onWheel);
  }, []);

  // Zoom-Tasten; behandelte Tasten gehen nicht an umgebende Elemente (z. B. das Ticket-Panel) weiter
  const onKeyDown = (event: KeyboardEvent<HTMLElement>) => {
    if (event.key === '+' || event.key === '=') zoomCentered(ZOOM_STEP);
    else if (event.key === '-') zoomCentered(1 / ZOOM_STEP);
    else if (event.key === '0') fit();
    else return;
    event.preventDefault();
    event.stopPropagation();
  };

  // Ziehen endet beim Loslassen, beim Abbruch durch den Browser und beim Verlust der Zeigererfassung
  const endDrag = () => {
    drag.current = null;
  };

  const stageProps = {
    ref: stage,
    onPointerDown: (event: PointerEvent<HTMLDivElement>) => {
      drag.current = { x: event.clientX, y: event.clientY };
      event.currentTarget.setPointerCapture?.(event.pointerId);
    },
    onPointerMove: (event: PointerEvent<HTMLDivElement>) => {
      const start = drag.current;
      if (!start) return;
      const dx = event.clientX - start.x;
      const dy = event.clientY - start.y;
      drag.current = { x: event.clientX, y: event.clientY };
      setView((v) => ({ ...v, x: v.x + dx, y: v.y + dy }));
    },
    onPointerUp: endDrag,
    onPointerCancel: endDrag,
    onLostPointerCapture: endDrag,
  };

  return { stage, view, fit, zoomCentered, actualSize, onKeyDown, stageProps };
}
