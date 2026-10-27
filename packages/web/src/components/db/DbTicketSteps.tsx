import { useCallback, useEffect, useLayoutEffect, useRef, useState } from 'react';
import type { DbStepChange, DbStepDto, DbStepInput, DbTicketDetailDto } from '../../api/dbTypes.js';
import { MarkdownView } from '../../markdown/MarkdownView.js';
import { Progress } from '../Progress.js';
import { DbStepForm } from './DbStepForm.js';

// Aktionen der Schritte; expectText ist der Text, den der Schritt an dieser Nummer in der Anzeige hatte
// (der Server lehnt ab, wenn dort inzwischen etwas anderes steht)
export interface DbStepActions {
  onToggle: (number: number, done: boolean, expectText: string) => void;
  onAdd: (input: DbStepInput) => Promise<boolean>;
  onEdit: (number: number, change: DbStepChange, expectText: string) => Promise<boolean>;
  onRemove: (number: number, expectText: string) => Promise<boolean>;
  onMove: (number: number, to: number, expectText: string) => Promise<boolean>;
}

// Was gerade bearbeitet wird: neuer Schritt, ein Schritt (Nummer und Text beim Öffnen) oder die Rückfrage zum Löschen
type Mode = { kind: 'add' } | { kind: 'edit' | 'remove'; number: number; text: string } | null;

// Nummer des Schritts, auf den sich ein offenes Formular bzw. eine Rückfrage bezieht, im aktuellen Stand:
// dieselbe Nummer, solange dort noch derselbe Text steht, sonst die erste mit diesem Text; 0, wenn es ihn nicht mehr gibt
function locate(steps: DbStepDto[], number: number, text: string): number {
  if (steps[number - 1]?.text === text) return number;
  return steps.findIndex((s) => s.text === text) + 1;
}

// Arbeitsschritte eines DB-Tickets mit Fortschritt (Inhalt des Reiters „Schritte“).Der Bereich erscheint immer (auch ohne Schritte) mit „+ Schritt“;
// jeder Schritt lässt sich abhaken, bearbeiten, löschen (mit Rückfrage) und nach oben oder unten schieben.
// Solange ein Formular oder eine Lösch-Rückfrage offen ist, sind Verschieben, Bearbeiten und Löschen der Schritte
// gesperrt; ein geänderter Entwurf oder eine offene Rückfrage gilt als ungespeichert (onDirtyChange).
export function DbTicketSteps({ ticket, busy, onDirtyChange, ...actions }: {
  ticket: DbTicketDetailDto;
  busy: boolean;
  onDirtyChange?: (dirty: boolean) => void;
} & DbStepActions) {
  const steps = ticket.steps ?? []; // ältere Server liefern die Schritte nicht mit
  const [mode, setMode] = useState<Mode>(null);
  const [formDirty, setFormDirty] = useState(false);
  // Eigene Aktion aus Formular bzw. Rückfrage läuft (so lange folgt das Formular keiner Listenänderung)
  const [saving, setSaving] = useState(false);
  const sectionRef = useRef<HTMLElement | null>(null);
  const addRef = useRef<HTMLButtonElement | null>(null);
  // aria-label des Knopfs, der das Formular bzw. die Rückfrage geöffnet hat (null = „+ Schritt“)
  const returnTo = useRef<string | null>(null);
  // Knopf, der nach dem Schließen den Fokus bekommt (undefined = keiner)
  const focusAfter = useRef<string | null | undefined>(undefined);

  // Öffnet Formular bzw. Rückfrage und merkt sich den auslösenden Knopf
  const open = (next: Mode, trigger: string | null) => {
    returnTo.current = trigger;
    focusAfter.current = undefined;
    setFormDirty(false);
    setMode(next);
  };
  // Schließt Formular bzw. Rückfrage; der Fokus geht an den auslösenden Knopf (back) oder an „+ Schritt“
  const close = (back = true) => {
    focusAfter.current = back ? returnTo.current : null;
    setFormDirty(false);
    setMode(null);
  };
  // Führt eine Aktion aus Formular bzw. Rückfrage aus; true bei Erfolg
  const submit = async (work: () => Promise<boolean>): Promise<boolean> => {
    setSaving(true);
    try {
      return await work();
    } finally {
      setSaving(false);
    }
  };

  // Ändert sich die Liste (409, Live), folgt ein offenes Formular seinem Schritt; gibt es ihn nicht mehr, schließt es
  const target = mode && mode.kind !== 'add' ? locate(steps, mode.number, mode.text) : 0;
  useEffect(() => {
    if (saving || !mode || mode.kind === 'add' || target === mode.number) return;
    if (target === 0) close(false);
    else setMode({ ...mode, number: target });
  });

  // Fokus nach dem Schließen setzen, sobald der Knopf wieder da und bedienbar ist
  useLayoutEffect(() => {
    if (mode || focusAfter.current === undefined) return;
    const label = focusAfter.current;
    const button = label === null
      ? addRef.current
      : Array.from(sectionRef.current?.querySelectorAll<HTMLButtonElement>('button[aria-label]') ?? []).find((b) => b.getAttribute('aria-label') === label) ?? addRef.current;
    if (!button || button.disabled) return;
    focusAfter.current = undefined;
    button.focus();
  });

  const dirty = mode?.kind === 'remove' || (!!mode && formDirty);
  useEffect(() => {
    onDirtyChange?.(dirty);
  }, [dirty, onDirtyChange]);
  useEffect(() => () => onDirtyChange?.(false), [onDirtyChange]);

  const trackDirty = useCallback((value: boolean) => setFormDirty(value), []);
  const locked = busy || !!mode;

  return (
    <section ref={sectionRef} className="db-steps" aria-label="Schritte">
      {steps.length > 0
        ? <Progress done={ticket.stepsDone} total={ticket.stepsTotal} label="Fortschritt der Schritte" />
        : <p className="muted">Noch keine Schritte.</p>}
      {steps.length > 0 && (
        <ol className="db-step-list">
          {steps.map((step, index) => {
            const number = index + 1;
            const name = `Schritt ${number}`;
            if (mode?.kind === 'edit' && mode.number === number) {
              // Fester Schlüssel: das Formular behält seinen Entwurf, auch wenn der Schritt die Nummer wechselt
              return (
                <li key="edit-form" className="db-step">
                  <DbStepForm
                    label={`${name} bearbeiten`}
                    initial={{ text: step.text, detail: step.detail ?? '' }}
                    busy={busy}
                    submitLabel="Speichern"
                    onCancel={() => close()}
                    onDirtyChange={trackDirty}
                    onSave={(value) => submit(async () => {
                      const ok = await actions.onEdit(number, { text: value.text, detail: value.detail.trim() ? value.detail : null }, mode.text);
                      if (ok) {
                        returnTo.current = `${name} bearbeiten`;
                        close();
                      }
                      return ok;
                    })}
                  />
                </li>
              );
            }
            const confirming = mode?.kind === 'remove' && mode.number === number;
            return (
              <li key={`${number}:${step.text}`} className={step.done ? 'db-step done' : 'db-step'}>
                <div className="db-step-line">
                  <label className="db-step-row">
                    <input
                      type="checkbox"
                      checked={step.done}
                      disabled={busy}
                      aria-label={`${name}: ${step.text}`}
                      onChange={(e) => actions.onToggle(number, e.target.checked, step.text)}
                    />
                    <span className="db-step-text">{step.text}</span>
                  </label>
                  <span className="db-step-tools">
                    <button type="button" className="icon-btn" aria-label={`${name} nach oben`} title="Nach oben" disabled={locked || number === 1} onClick={() => void actions.onMove(number, number - 1, step.text)}>↑</button>
                    <button type="button" className="icon-btn" aria-label={`${name} nach unten`} title="Nach unten" disabled={locked || number === steps.length} onClick={() => void actions.onMove(number, number + 1, step.text)}>↓</button>
                    <button type="button" className="icon-btn" aria-label={`${name} bearbeiten`} title="Bearbeiten" disabled={locked} onClick={() => open({ kind: 'edit', number, text: step.text }, `${name} bearbeiten`)}>✎</button>
                    <button type="button" className="icon-btn" aria-label={`${name} löschen`} title="Löschen" disabled={locked} onClick={() => open({ kind: 'remove', number, text: step.text }, `${name} löschen`)}>✕</button>
                  </span>
                </div>
                {confirming && (
                  <div
                    role="alertdialog"
                    aria-label={`${name} löschen`}
                    className="actions db-step-confirm"
                    onKeyDown={(e) => { if (e.key === 'Escape') { e.preventDefault(); e.stopPropagation(); close(); } }}
                  >
                    <span>{name} „{step.text}“ löschen?</span>
                    <button
                      type="button"
                      className="btn danger small"
                      disabled={busy}
                      onClick={() => void submit(async () => {
                        const ok = await actions.onRemove(number, mode.text);
                        if (ok) close(false);
                        return ok;
                      })}
                    >
                      Löschen
                    </button>
                    <button type="button" className="btn small" autoFocus onClick={() => close()}>Abbrechen</button>
                  </div>
                )}
                {step.detail && (
                  <details className="db-step-detail">
                    <summary>Anleitung</summary>
                    <MarkdownView source={step.detail} />
                  </details>
                )}
              </li>
            );
          })}
        </ol>
      )}
      {mode?.kind === 'add' ? (
        <DbStepForm
          label="Neuer Schritt"
          busy={busy}
          submitLabel="Hinzufügen"
          onCancel={() => close(false)}
          onDirtyChange={trackDirty}
          onSave={async (value) => {
            const ok = await actions.onAdd({ text: value.text, detail: value.detail.trim() ? value.detail : undefined });
            if (ok) close(false);
            return ok;
          }}
        />
      ) : (
        <button ref={addRef} type="button" className="btn small db-step-add" disabled={locked} onClick={() => open({ kind: 'add' }, null)}>
          + Schritt
        </button>
      )}
    </section>
  );
}
