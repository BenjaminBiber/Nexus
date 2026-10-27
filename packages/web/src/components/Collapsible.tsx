import { useState, type ReactNode } from 'react';

// Ab dieser Länge (Zeichen oder Zeilen) wird ein Text zunächst gekürzt gezeigt
const MAX_CHARS = 280;
const MAX_LINES = 5;

// Ist der Text so lang, dass er eingeklappt starten soll?
export function isLong(text: string): boolean {
  return text.length > MAX_CHARS || text.split('\n').length > MAX_LINES;
}

// Langer Inhalt startet eingeklappt (wenige Zeilen sichtbar) mit „Mehr anzeigen“ / „Weniger anzeigen“; kurzer bleibt offen.
export function Collapsible({ text, children, className = '' }: { text: string; children: ReactNode; className?: string }) {
  const [open, setOpen] = useState(false);
  if (!isLong(text)) return <div className={className}>{children}</div>;
  return (
    <div className={className}>
      <div className={open ? 'collapsible' : 'collapsible clamped'}>{children}</div>
      <button type="button" className="collapsible-toggle" aria-expanded={open} onClick={() => setOpen(!open)}>
        {open ? 'Weniger anzeigen' : 'Mehr anzeigen'}
      </button>
    </div>
  );
}
