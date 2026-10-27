import { useState, type ReactNode, type Ref } from 'react';

// Schlüssel im localStorage für den Zustand eines Bereichs
const STORAGE_PREFIX = 'harness.drawer.';

// Gemerkter Zustand (true = offen) oder null
function remembered(id: string): boolean | null {
  try {
    const value = localStorage.getItem(STORAGE_PREFIX + id);
    return value === null ? null : value === '1';
  } catch {
    return null;
  }
}

// Merkt den Zustand; ohne localStorage bleibt er nur bis zum Neuladen
function remember(id: string, open: boolean): void {
  try {
    localStorage.setItem(STORAGE_PREFIX + id, open ? '1' : '0');
  } catch {
    /* ohne Speicher */
  }
}

export interface DbDrawerProps {
  /** Schlüssel des gemerkten Zustands (je Bereich, nicht je Ticket) */
  id: string;
  title: string;
  /** Kurzinfo neben dem Titel, auch im zugeklappten Zustand sichtbar (z. B. „3/10“) */
  meta?: ReactNode;
  /** Knöpfe rechts im Kopf (klappen nicht auf oder zu) */
  actions?: ReactNode;
  defaultOpen?: boolean;
  className?: string;
  sectionRef?: Ref<HTMLElement>;
  children: ReactNode;
}

// Ein- und ausklappbarer Bereich im Ticket-Panel. Der Inhalt bleibt beim Zuklappen erhalten (nur versteckt), damit
// Eingaben nicht verloren gehen; der Zustand wird je Bereich im Browser gemerkt.
export function DbDrawer({ id, title, meta, actions, defaultOpen = true, className, sectionRef, children }: DbDrawerProps) {
  const [open, setOpen] = useState(() => remembered(id) ?? defaultOpen);
  const bodyId = `db-drawer-${id}`;
  const toggle = () => {
    setOpen((o) => {
      remember(id, !o);
      return !o;
    });
  };
  return (
    <section ref={sectionRef} className={`db-drawer${open ? ' open' : ''}${className ? ` ${className}` : ''}`} aria-label={title}>
      <div className="db-drawer-head">
        <h3 className="db-drawer-title">
          <button type="button" className="db-drawer-toggle" aria-expanded={open} aria-controls={bodyId} onClick={toggle}>
            <span className="db-drawer-chevron" aria-hidden="true" />
            {title}
            {meta !== undefined && meta !== null && meta !== '' && <span className="db-drawer-meta">{meta}</span>}
          </button>
        </h3>
        {actions && <div className="db-drawer-actions">{actions}</div>}
      </div>
      <div id={bodyId} className="db-drawer-body" hidden={!open}>
        {children}
      </div>
    </section>
  );
}
