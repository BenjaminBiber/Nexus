import { useEffect, useRef, useState, type CSSProperties } from 'react';

interface Props {
  disabled: boolean;
  importDisabled: boolean;
  onNew: () => void;
  onImport: () => void;
}

// Schwebender Knopf unten rechts am Board: öffnet ein Menü mit „Neues Ticket“ und „Von anderem Board holen“.
// Die zwei Einträge wachsen als kleine Kreise mit Goo-Effekt (Blur + Alpha-Schwelle) aus dem Knopf nach oben.
// Der Fokus geht vor dem Öffnen eines Dialogs an den Knopf zurück, damit Abbrechen ihn dorthin zurückgeben kann.
export function DbAddFab({ disabled, importDisabled, onNew, onImport }: Props) {
  const [open, setOpen] = useState(false);
  const root = useRef<HTMLDivElement>(null);
  const trigger = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    if (!open) return;
    const outside = (event: PointerEvent) => {
      if (!root.current?.contains(event.target as Node)) setOpen(false);
    };
    const escape = (event: KeyboardEvent) => {
      if (event.key !== 'Escape') return;
      setOpen(false);
      trigger.current?.focus();
    };
    document.addEventListener('pointerdown', outside);
    document.addEventListener('keydown', escape);
    return () => {
      document.removeEventListener('pointerdown', outside);
      document.removeEventListener('keydown', escape);
    };
  }, [open]);

  const choose = (action: () => void) => {
    trigger.current?.focus();
    setOpen(false);
    action();
  };

  return (
    <div className={`db-fab${open ? ' open' : ''}${disabled ? ' is-disabled' : ''}`} ref={root}>
      <svg className="db-fab-defs" width="0" height="0" aria-hidden="true" focusable="false">
        <defs>
          <filter id="db-fab-goo" colorInterpolationFilters="sRGB">
            <feGaussianBlur in="SourceGraphic" stdDeviation="7" result="blur" />
            <feColorMatrix in="blur" type="matrix" values="1 0 0 0 0  0 1 0 0 0  0 0 1 0 0  0 0 0 22 -10" />
          </filter>
        </defs>
      </svg>
      {/* Nur Dekor: die schwarzen Kreise, die durch den Goo-Filter ineinanderfließen. Knöpfe und Beschriftung liegen darüber. */}
      <div className="db-fab-goo" aria-hidden="true">
        <span className="db-fab-dot db-fab-dot-main" />
        <span className="db-fab-dot db-fab-dot-item" style={{ '--i': 1 } as CSSProperties} />
        <span className="db-fab-dot db-fab-dot-item" style={{ '--i': 2 } as CSSProperties} />
      </div>
      {open && (
        <div className="db-fab-menu" role="menu" aria-label="Ticket hinzufügen">
          <button type="button" role="menuitem" className="db-fab-item" style={{ '--i': 1 } as CSSProperties} onClick={() => choose(onNew)}>
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
              <path d="M14 3H7a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8z" />
              <path d="M14 3v5h5M12 12v5M9.5 14.5h5" />
            </svg>
            <span className="db-fab-label">Neues Ticket</span>
          </button>
          <button type="button" role="menuitem" className="db-fab-item" style={{ '--i': 2 } as CSSProperties} disabled={importDisabled} onClick={() => choose(onImport)}>
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
              <path d="M12 4v11M8 11l4 4 4-4M5 19h14" />
            </svg>
            <span className="db-fab-label">Von anderem Board holen</span>
          </button>
        </div>
      )}
      <button
        type="button"
        ref={trigger}
        className={`db-fab-btn${open ? ' open' : ''}`}
        aria-label="Ticket hinzufügen"
        aria-haspopup="menu"
        aria-expanded={open}
        disabled={disabled}
        onClick={() => setOpen((value) => !value)}
      >
        <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.4" strokeLinecap="round" aria-hidden="true">
          <path d="M12 5v14M5 12h14" />
        </svg>
      </button>
    </div>
  );
}
