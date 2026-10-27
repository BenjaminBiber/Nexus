import { useEffect, useId, useRef, type ReactNode } from 'react';

// Abstand des Popovers zum Knopf und zum Fensterrand
const GAP = 8;
const EDGE = 16;
const MAX_WIDTH = 340;

// Info-Knopf (i) mit einer Erklärung im Popover: öffnet per Klick oder Tastatur, schließt mit Esc, Klick daneben oder Scrollen.
// Das Popover liegt in der obersten Ebene (Popover-API) und wird am Knopf ausgerichtet, damit kein Container es abschneidet.
export function InfoTip({ label, children }: { label: string; children: ReactNode }) {
  const id = useId();
  const button = useRef<HTMLButtonElement>(null);
  const pop = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const el = pop.current;
    if (!el) return;
    // Scrollen außerhalb des Popovers schließt es, sonst stünde es neben dem Knopf
    const close = (e: Event) => {
      if (!el.contains(e.target as Node)) el.hidePopover?.();
    };
    // Unter dem Knopf ausrichten; reicht der Platz nicht, darüber
    const onToggle = (e: Event) => {
      if ((e as Event & { newState?: string }).newState !== 'open') return window.removeEventListener('scroll', close, true);
      const anchor = button.current?.getBoundingClientRect();
      if (anchor) {
        const width = Math.min(MAX_WIDTH, window.innerWidth - 2 * EDGE);
        el.style.width = `${width}px`;
        el.style.left = `${Math.max(EDGE, Math.min(anchor.left - 12, window.innerWidth - width - EDGE))}px`;
        const below = anchor.bottom + GAP;
        el.style.top = `${below + el.offsetHeight > window.innerHeight - EDGE ? Math.max(EDGE, anchor.top - GAP - el.offsetHeight) : below}px`;
      }
      window.addEventListener('scroll', close, true);
    };
    el.addEventListener('toggle', onToggle);
    return () => {
      el.removeEventListener('toggle', onToggle);
      window.removeEventListener('scroll', close, true);
    };
  }, []);

  return (
    <>
      <button ref={button} type="button" className="info-tip" aria-label={`Erklärung: ${label}`} popoverTarget={id}>
        <svg viewBox="0 0 24 24" aria-hidden="true">
          <circle cx="12" cy="12" r="9" />
          <path d="M12 11v5M12 8h.01" />
        </svg>
      </button>
      <div ref={pop} id={id} popover="auto" className="info-pop">
        {children}
      </div>
    </>
  );
}
