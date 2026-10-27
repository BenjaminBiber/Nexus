import { useState, type ReactNode } from 'react';

// Abschnitt der Werkzeugspalte im Stil der Editor-Seitenleiste: Kopfzeile mit Pfeil, darunter der Inhalt.
// Der Kopf ist ein Knopf, damit sich der Abschnitt auch mit der Tastatur auf- und zuklappen lässt.
export function DocToolSection({ title, children, defaultOpen = true }: { title: string; children: ReactNode; defaultOpen?: boolean }) {
  const [open, setOpen] = useState(defaultOpen);
  return (
    <section className={`doc-tool${open ? ' open' : ''}`}>
      <button type="button" className="doc-tool-title" aria-expanded={open} onClick={() => setOpen(!open)}>
        <svg width="10" height="10" viewBox="0 0 12 12" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
          <path d="M4.5 2.5L8 6l-3.5 3.5" />
        </svg>
        {title}
      </button>
      {open && <div className="doc-tool-body">{children}</div>}
    </section>
  );
}
