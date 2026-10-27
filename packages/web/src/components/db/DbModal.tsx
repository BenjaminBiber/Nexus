import { useEffect, useRef, type FormEvent, type KeyboardEvent, type ReactNode, type RefObject } from 'react';
import { useRestoreFocus } from '../../hooks/useRestoreFocus.js';

const FOCUSABLE = [
  'a[href]',
  'button:not([disabled])',
  'input:not([disabled])',
  'select:not([disabled])',
  'textarea:not([disabled])',
  '[tabindex]:not([tabindex="-1"])',
].join(',');

// Bedienbare Elemente im Dialog in Tab-Reihenfolge des Dokuments
function focusables(node: HTMLElement): HTMLElement[] {
  return Array.from(node.querySelectorAll<HTMLElement>(FOCUSABLE));
}

// Offene Dialoge in Reihenfolge des Öffnens; nur der oberste holt verlorenen Fokus zurück
const openModals: HTMLElement[] = [];

function isTopmost(node: HTMLElement): boolean {
  return openModals[openModals.length - 1] === node;
}

// Fokus liegt nirgends (body oder gar kein Element)
function lostFocus(): boolean {
  const active = document.activeElement;
  return !active || active === document.body;
}

export type DbModalProps = {
  // id der Überschrift, die den Dialog benennt
  labelledBy: string;
  // Esc im Dialog; die Dialoge entscheiden selbst, was zuerst abgebrochen wird
  onEscape: () => void;
  children: ReactNode;
  role?: 'dialog' | 'alertdialog';
  className?: string;
  backdropClassName?: string;
  // Mit onSubmit wird der Dialog selbst zum Formular
  onSubmit?: (event: FormEvent<HTMLFormElement>) => void;
  // Element für den ersten Fokus; sonst Autofokus im Inhalt, das erste bedienbare Element oder der Dialog
  initialFocus?: RefObject<HTMLElement | null>;
  // Ref auf den Dialog selbst (z. B. um ihm den Fokus zu geben)
  dialogRef?: RefObject<HTMLElement | null>;
  // Klick auf den Hintergrund (Drücken und Loslassen beide außerhalb des Dialogs); ohne bleibt der Dialog offen
  onBackdrop?: () => void;
};

// Gemeinsame Hülle der DB-Board-Dialoge: Hintergrund, Rolle und Name, Esc, Fokusfalle (Tab/Umschalt+Tab bleiben im Dialog),
// erster Fokus und Rückgabe des Fokus beim echten Schließen (StrictMode-fest).
export function DbModal({ labelledBy, onEscape, children, role = 'dialog', className = 'panel db-modal', backdropClassName = 'db-modal-backdrop', onSubmit, initialFocus, dialogRef, onBackdrop }: DbModalProps) {
  const ownRef = useRef<HTMLElement | null>(null);
  const ref = dialogRef ?? ownRef;
  useRestoreFocus(ref);
  // Begann der Klick auf dem Hintergrund? Ein Ziehen aus dem Dialog heraus (z. B. Text markieren) schließt nicht
  const pressedOnBackdrop = useRef(false);

  // Erster Fokus: gewünschtes Element; liegt der Fokus (trotz Autofokus) nicht im Dialog, das erste bedienbare Element
  useEffect(() => {
    const node = ref.current;
    if (!node) return;
    if (initialFocus?.current) {
      initialFocus.current.focus();
      return;
    }
    if (node.contains(document.activeElement)) return;
    (focusables(node)[0] ?? node).focus();
    // nur beim Öffnen
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Verlässt der Fokus den Dialog ins Leere (relatedTarget null, danach body), holt ihn der oberste offene Dialog zurück,
  // damit Esc und Tab weiter greifen. Ein bewusst gewähltes Element außerhalb behält den Fokus.
  const focusInside = useRef(false);
  useEffect(() => {
    const node = ref.current;
    if (!node) return;
    openModals.push(node);
    focusInside.current = node.contains(document.activeElement);
    const onFocusIn = () => {
      focusInside.current = true;
    };
    const onFocusOut = (event: FocusEvent) => {
      const next = event.relatedTarget as Node | null;
      if (next && node.contains(next)) return;
      focusInside.current = false;
      if (next) return;
      // Erst nach dem Ereignis steht fest, wo der Fokus landet
      queueMicrotask(() => {
        if (node.isConnected && isTopmost(node) && lostFocus()) node.focus();
      });
    };
    node.addEventListener('focusin', onFocusIn);
    node.addEventListener('focusout', onFocusOut);
    return () => {
      node.removeEventListener('focusin', onFocusIn);
      node.removeEventListener('focusout', onFocusOut);
      openModals.splice(openModals.lastIndexOf(node), 1);
    };
  }, [ref]);

  // Nach dem Rendern nur für Fokus, der im Dialog war und ohne focusout verloren ging: gesperrtes oder entferntes Element
  // (z. B. während und nach dem Senden). Fokus außerhalb des Dialogs bleibt unberührt.
  useEffect(() => {
    const node = ref.current;
    if (!node || !focusInside.current || !isTopmost(node)) return;
    const active = document.activeElement;
    const disabledInside = !!active && node.contains(active) && (active as HTMLButtonElement).disabled === true;
    if (disabledInside || lostFocus()) node.focus();
  });

  const onKeyDown =(event: KeyboardEvent<HTMLElement>) => {
    const node = ref.current;
    if (!node) return;
    if (event.key === 'Escape') {
      event.stopPropagation();
      onEscape();
      return;
    }
    if (event.key !== 'Tab') return;
    const items = focusables(node);
    const active = document.activeElement;
    if (!items.length) {
      event.preventDefault();
      node.focus();
      return;
    }
    const first = items[0]!;
    const last = items[items.length - 1]!;
    const outside = !active || !node.contains(active);
    if (event.shiftKey && (active === first || active === node || outside)) {
      event.preventDefault();
      last.focus();
    } else if (!event.shiftKey && (active === last || outside)) {
      event.preventDefault();
      first.focus();
    }
  };

  const shared = {
    role,
    'aria-modal': true,
    'aria-labelledby': labelledBy,
    tabIndex: -1,
    className,
    onKeyDown,
  } as const;

  return (
    <div
      className={backdropClassName}
      onMouseDown={(event) => {
        pressedOnBackdrop.current = event.target === event.currentTarget;
      }}
      onClick={(event) => {
        const fromBackdrop = pressedOnBackdrop.current && event.target === event.currentTarget;
        pressedOnBackdrop.current = false;
        if (fromBackdrop) onBackdrop?.();
      }}
    >
      {onSubmit ? (
        <form ref={ref as RefObject<HTMLFormElement | null>} {...shared} onSubmit={onSubmit}>
          {children}
        </form>
      ) : (
        <div ref={ref as RefObject<HTMLDivElement | null>} {...shared}>
          {children}
        </div>
      )}
    </div>
  );
}
