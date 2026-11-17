import { useEffect, useId, useRef, useState, type KeyboardEvent as ReactKeyboardEvent } from 'react';
import type { DbQuestionOptionDto } from '../../api/dbTypes.js';
import { splitEffort } from '../../dbquestion/questionSections.js';
import { MarkdownView } from '../../markdown/MarkdownView.js';

const MAC = typeof navigator !== 'undefined' && /Mac|iPhone|iPad/.test(navigator.platform);

// Vor- und Nachteile einer Option (auch in der nur lesbaren Liste der Recherche)
export function OptionProsCons({ option }: { option: Pick<DbQuestionOptionDto, 'pros' | 'cons'> }) {
  if (option.pros.length === 0 && option.cons.length === 0) return null;
  return (
    <div className="qx-proscons">
      {option.pros.length > 0 && (
        <div className="qx-pc pros">
          <div className="qx-pc-label">Vorteile</div>
          <ul>
            {option.pros.map((p) => (
              <li key={p}>{p}</li>
            ))}
          </ul>
        </div>
      )}
      {option.cons.length > 0 && (
        <div className="qx-pc cons">
          <div className="qx-pc-label">Nachteile</div>
          <ul>
            {option.cons.map((c) => (
              <li key={c}>{c}</li>
            ))}
          </ul>
        </div>
      )}
    </div>
  );
}

// Liegt der Fokus in einem Eingabefeld? Dann gehören Zifferntasten dem Feld
function typing(target: EventTarget | null): boolean {
  return target instanceof HTMLElement && target.closest('input, textarea, select, [contenteditable="true"]') !== null;
}

// Entscheidungskarte einer offenen Frage: was zu entscheiden ist, worum es geht, auf Wunsch der Stand heute aus der Recherche,
// die Optionen der neuesten Recherche als Auswahl (Ziffern 1–n), eine eigene Antwort (Ziffer n+1), Begründung, Verwerfen,
// Überspringen und Entscheiden (Strg+Enter).
export function DbQuestionDecide({ options, decideText, aboutText, todayText, busy, serverError, onDecide, onDiscard, onSkip }: {
  /** Optionen der neuesten Recherche */
  options: DbQuestionOptionDto[];
  decideText: string | null;
  aboutText: string | null;
  /** Abschnitt „Heute“ der neuesten Recherche */
  todayText: string | null;
  busy: boolean;
  serverError: string | null;
  onDecide: (input: { option?: string; text?: string; reason: string }) => Promise<boolean>;
  onDiscard: (reason: string) => Promise<boolean>;
  /** Zur nächsten Frage der Liste; null, wenn es keine gibt */
  onSkip: (() => void) | null;
}) {
  const baseId = useId();
  const [picked, setPicked] = useState<string | null>(null);
  const [custom, setCustom] = useState('');
  const [reason, setReason] = useState('');
  const [reasonError, setReasonError] = useState<string | null>(null);
  const [contextOpen, setContextOpen] = useState(false);
  const radios = useRef(new Map<string, HTMLButtonElement>());
  const customRef = useRef<HTMLInputElement>(null);
  const reasonRef = useRef<HTMLTextAreaElement>(null);

  // Eine neue Recherche (Live) ohne die gewählte Option hebt die Auswahl auf
  if (picked !== null && !options.some((o) => o.id === picked)) setPicked(null);

  const answer = custom.trim();
  const canDecide = picked !== null || answer !== '';
  const status = picked !== null ? `Gewählt: ${picked}` : answer ? 'Eigene Antwort' : 'Noch nichts gewählt';
  const label = picked !== null ? `Entscheiden: ${picked}` : 'Entscheiden';

  // Wählt eine Option; eine eigene Antwort entfällt dann
  const pick = (id: string) => {
    setPicked(id);
    setCustom('');
  };

  // Entscheidet mit Option oder eigener Antwort; bei Erfolg wird die Karte geleert
  const decide = async () => {
    if (!canDecide || busy) return;
    setReasonError(null);
    const ok = await onDecide(picked !== null ? { option: picked, reason: reason.trim() } : { text: answer, reason: reason.trim() });
    if (ok) {
      setPicked(null);
      setCustom('');
      setReason('');
    }
  };

  // Verwerfen verlangt eine Begründung
  const discard = async () => {
    if (busy) return;
    if (!reason.trim()) {
      setReasonError('Bitte eine Begründung angeben.');
      reasonRef.current?.focus();
      return;
    }
    setReasonError(null);
    if (await onDiscard(reason.trim())) setReason('');
  };

  // Tastenkürzel der Seite: Strg/⌘+Enter entscheidet, Ziffern wählen eine Option bzw. springen zur eigenen Antwort.
  // Offene Dialoge und Eingabefelder haben Vorrang.
  const keys = useRef({ decide, pick, options });
  keys.current = { decide, pick, options };
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.defaultPrevented || document.querySelector('[aria-modal="true"]')) return;
      if ((event.ctrlKey || event.metaKey) && event.key === 'Enter') {
        event.preventDefault();
        void keys.current.decide();
        return;
      }
      if (event.ctrlKey || event.metaKey || event.altKey || typing(event.target) || !/^[1-9]$/.test(event.key)) return;
      const n = Number(event.key);
      const list = keys.current.options;
      if (n <= list.length) {
        keys.current.pick(list[n - 1].id);
        radios.current.get(list[n - 1].id)?.focus();
      } else if (n === list.length + 1) {
        customRef.current?.focus();
      } else {
        return;
      }
      event.preventDefault();
    };
    document.addEventListener('keydown', onKey);
    return () => document.removeEventListener('keydown', onKey);
  }, []);

  // Pfeiltasten in der Auswahl: nächste bzw. vorige Option (ringsum)
  const onRadioKey = (event: ReactKeyboardEvent<HTMLButtonElement>, index: number) => {
    const step = event.key === 'ArrowDown' || event.key === 'ArrowRight' ? 1 : event.key === 'ArrowUp' || event.key === 'ArrowLeft' ? -1 : 0;
    if (!step) return;
    event.preventDefault();
    const next = options[(index + step + options.length) % options.length];
    pick(next.id);
    radios.current.get(next.id)?.focus();
  };

  const tabStop = picked ?? options[0]?.id;
  const contextId = `${baseId}-context`;
  const reasonId = `${baseId}-reason`;

  return (
    <section className="qx-decide" aria-label="Entscheiden">
      <div className="qx-decide-head">
        {decideText && (
          <div className="qx-decide-q">
            <span className="qx-label">Zu entscheiden</span>
            <div className="qx-decide-text">
              <MarkdownView source={decideText} />
            </div>
          </div>
        )}
        {aboutText && (
          <div className="qx-decide-about">
            <MarkdownView source={aboutText} />
          </div>
        )}
        {todayText && (
          <>
            <button type="button" className="qx-toggle" aria-expanded={contextOpen} aria-controls={contextId} onClick={() => setContextOpen(!contextOpen)}>
              <span className="material-icons" aria-hidden="true">
                chevron_right
              </span>
              So ist es heute (Kontext der Recherche)
            </button>
            <div id={contextId} className="qx-context" hidden={!contextOpen}>
              <MarkdownView source={todayText} />
            </div>
          </>
        )}
      </div>

      <div className="qx-choices">
        {options.length > 0 ? (
          <div role="radiogroup" aria-label="Optionen" className="qx-options">
            {options.map((o, index) => {
              const { text, effort } = splitEffort(o.text);
              const on = picked === o.id;
              const titleId = `${baseId}-o${index}`;
              return (
                <div key={o.id} className={`qx-option${on ? ' on' : ''}`} onClick={() => pick(o.id)}>
                  <button
                    ref={(el) => {
                      if (el) radios.current.set(o.id, el);
                      else radios.current.delete(o.id);
                    }}
                    type="button"
                    role="radio"
                    className="qx-option-pick"
                    aria-checked={on}
                    aria-labelledby={titleId}
                    aria-describedby={text ? `${titleId}-desc` : undefined}
                    tabIndex={tabStop === o.id ? 0 : -1}
                    onKeyDown={(event) => onRadioKey(event, index)}
                  >
                    <span className="qx-radio" aria-hidden="true" />
                    <span className="qx-option-text">
                      <span className="qx-option-title">
                        <span id={titleId}>
                          {o.title}
                          {o.recommended ? ' (empfohlen)' : ''}
                        </span>
                        {effort && <span className="qx-chip">Aufwand: {effort}</span>}
                      </span>
                      {text && (
                        <span id={`${titleId}-desc`} className="qx-option-desc">
                          {text}
                        </span>
                      )}
                    </span>
                    {index < 9 && (
                      <kbd className="qx-key" aria-hidden="true">
                        {index + 1}
                      </kbd>
                    )}
                  </button>
                  <OptionProsCons option={o} />
                </div>
              );
            })}
          </div>
        ) : (
          <p className="qx-muted qx-no-options">Noch keine Optionen. Eine Recherche schlägt welche vor; du kannst auch frei antworten.</p>
        )}
        <div className={`qx-custom${answer ? ' on' : ''}`}>
          <span className="material-icons" aria-hidden="true">
            edit
          </span>
          <input
            ref={customRef}
            type="text"
            aria-label="Eigene Antwort"
            placeholder="Etwas anderes antworten, z. B. „So wie bisher, aber aufgeräumt …“"
            value={custom}
            onChange={(event) => {
              setCustom(event.target.value);
              if (event.target.value.trim()) setPicked(null);
            }}
          />
          {options.length < 9 && (
            <kbd className="qx-key" aria-hidden="true">
              {options.length + 1}
            </kbd>
          )}
        </div>
      </div>

      <div className="qx-reason">
        <label htmlFor={reasonId}>
          Begründung <span>– optional, beim Verwerfen Pflicht</span>
        </label>
        <textarea
          ref={reasonRef}
          id={reasonId}
          rows={2}
          placeholder="Warum diese Entscheidung? Sessions lesen das mit."
          value={reason}
          onChange={(event) => {
            setReason(event.target.value);
            setReasonError(null);
          }}
        />
        {(reasonError ?? serverError) && (
          <p role="alert" className="qx-error">
            {reasonError ?? serverError}
          </p>
        )}
      </div>

      <div className="qx-decide-foot">
        <button type="button" className="qx-btn danger" disabled={busy} onClick={() => void discard()}>
          Verwerfen
        </button>
        <button type="button" className="qx-btn ghost" disabled={!onSkip} onClick={() => onSkip?.()}>
          Überspringen
        </button>
        <div className="qx-decide-go">
          <span role="status" className="qx-decide-status">
            {status}
          </span>
          <button type="button" className="qx-btn accent big" disabled={!canDecide || busy} aria-keyshortcuts={MAC ? 'Meta+Enter' : 'Control+Enter'} onClick={() => void decide()}>
            {label}
            <kbd aria-hidden="true">{MAC ? '⌘ ↵' : 'Strg ↵'}</kbd>
          </button>
        </div>
      </div>
    </section>
  );
}
