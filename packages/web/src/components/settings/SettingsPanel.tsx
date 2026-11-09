import { createContext, useCallback, useContext, useState, type ReactNode } from 'react';
import { createPortal } from 'react-dom';
import { Panel } from '../ui/Panel.js';

export type SaveState = { kind: 'idle' } | { kind: 'saving' } | { kind: 'saved' } | { kind: 'error'; message: string };

// Platz im Seitenkopf der Einstellungen für den Speicherstand; ohne Seite (Tests, eingebettet) steht er im Panel
export const SaveStatusSlot = createContext<HTMLElement | null>(null);

// Ein Bereich der Einstellungen (Baustein „Panel“, Theme darf das Innere gestalten). status: Bereich speichert jede Änderung sofort und meldet den Stand im Seitenkopf.
export function SettingsPanel({ label, status, className, children }: { label: string; status?: SaveState; className?: string; children: ReactNode }) {
  const slot = useContext(SaveStatusSlot);
  const line = status && <SaveStatus state={status} />;
  return (
    <Panel as="section" className={className ? `settings-panel ${className}` : 'settings-panel'} aria-label={label}>
      {line && (slot ? createPortal(line, slot) : line)}
      {children}
    </Panel>
  );
}

// Speichern mit Rückmeldung: run(fn) setzt „Wird gespeichert …“, danach „Gespeichert“ oder den Fehler
export function useSaveStatus() {
  const [state, setState] = useState<SaveState>({ kind: 'idle' });
  const run = useCallback(async <T,>(fn: () => Promise<T>): Promise<T | undefined> => {
    setState({ kind: 'saving' });
    try {
      const result = await fn();
      setState({ kind: 'saved' });
      return result;
    } catch (e) {
      setState({ kind: 'error', message: (e as Error).message });
      return undefined;
    }
  }, []);
  return { state, run, setState };
}

// Speicherstand als kurze Zeile; Fehler als Meldung, damit Screenreader ihn sofort vorlesen
function SaveStatus({ state }: { state: SaveState }) {
  if (state.kind === 'error') return <p className="save-status error" role="alert">{state.message}</p>;
  const text = state.kind === 'saving' ? 'Wird gespeichert …' : state.kind === 'saved' ? 'Gespeichert' : 'Änderungen werden sofort gespeichert';
  return (
    <p className={`save-status ${state.kind}`} role="status">
      {state.kind === 'saved' && (
        <svg viewBox="0 0 24 24" aria-hidden="true">
          <path d="M5 12.5l4.5 4.5L19 7.5" />
        </svg>
      )}
      {text}
    </p>
  );
}

// Fußleiste für Bereiche, die erst auf Knopfdruck speichern: links der Stand, rechts die Knöpfe; bleibt unten sichtbar
export function SettingsFooter({ state, children }: { state: ReactNode; children: ReactNode }) {
  return (
    <div className="settings-footer">
      <div className="settings-footer-state">{state}</div>
      <div className="actions">{children}</div>
    </div>
  );
}
