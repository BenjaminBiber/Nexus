import { useId, useRef, useState, type KeyboardEvent } from 'react';

export interface BranchComboProps {
  value: string;
  /** Vorschläge (Branches des Board-Repos); leer heißt: reines Textfeld */
  options: string[];
  readOnly?: boolean;
  title?: string;
  placeholder?: string;
  onChange: (value: string) => void;
  /** Speichert; ohne Wert gilt der zuletzt über onChange gemeldete */
  onCommit: (value?: string) => void;
  /** Mit Angabe zeigt das Feld bei einem neuen Namen rechts einen Speichern-Knopf, der den Branch anlegen lässt */
  onCreate?: (name: string) => void;
}

// Leerzeichen sind in Branch-Namen verboten; beim Tippen fallen sie am Anfang weg und werden sonst zu „-“
function branchName(value: string): string {
  return value.replace(/^\s+/, '').replace(/\s/g, '-');
}

// Höchstens so viele Vorschläge werden gezeigt; mehr hilft beim Suchen nicht und macht die Liste unübersichtlich
const MAX_VISIBLE = 20;

// Vorschläge zur Eingabe: Treffer mit passendem Anfang zuerst, Groß-/Kleinschreibung egal
function matches(options: string[], value: string): string[] {
  const needle = value.trim().toLowerCase();
  if (needle === '') return options.slice(0, MAX_VISIBLE);
  const starts = options.filter((o) => o.toLowerCase().startsWith(needle));
  const rest = options.filter((o) => !o.toLowerCase().startsWith(needle) && o.toLowerCase().includes(needle));
  return [...starts, ...rest].slice(0, MAX_VISIBLE);
}

// Eingabefeld für einen Branch-Namen mit Vorschlagsliste (Combobox): filtert beim Tippen, lässt aber auch einen
// Namen zu, den es noch nicht gibt – angelegt wird der Branch beim Start des Tickets oder vorher über den Speichern-Knopf.
export function BranchCombo({ value, options, readOnly, title, placeholder, onChange, onCommit, onCreate }: BranchComboProps) {
  const id = useId();
  const listRef = useRef<HTMLDivElement | null>(null);
  const [open, setOpen] = useState(false);
  const [active, setActive] = useState(-1);
  const hits = open && !readOnly ? matches(options, value) : [];
  const isNew = !readOnly && value.trim() !== '' && !options.some((o) => o.toLowerCase() === value.trim().toLowerCase());
  const unknown = open && isNew;
  // Rechts im Feld: bei einem neuen Namen der Speichern-Knopf, sonst bei einem Wert das X zum Abwählen
  const showSave = !!onCreate && isNew;
  const showClear = !readOnly && !showSave && value.trim() !== '';
  const optionId = (index: number) => `${id}-option-${index}`;

  // Übernimmt einen Vorschlag und speichert ihn sofort (der Entwurf im Panel ist erst nach dem Rendern aktuell)
  const choose = (name: string) => {
    setOpen(false);
    setActive(-1);
    onChange(name);
    onCommit(name);
  };

  const onKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key === 'Escape' && open) {
      event.preventDefault();
      setOpen(false);
      setActive(-1);
      return;
    }
    if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
      if (readOnly || hits.length === 0) {
        if (!readOnly && !open) setOpen(true);
        return;
      }
      event.preventDefault();
      const step = event.key === 'ArrowDown' ? 1 : -1;
      setActive((a) => (a + step + hits.length + (a < 0 && step < 0 ? 1 : 0)) % hits.length);
      return;
    }
    if (event.key === 'Enter') {
      event.preventDefault();
      if (active >= 0 && hits[active]) choose(hits[active]);
      else event.currentTarget.blur();
    }
  };

  return (
    <div className={`branch-combo${showSave || showClear ? ' with-action' : ''}`}>
      <input
        role="combobox"
        aria-expanded={hits.length > 0}
        aria-controls={`${id}-list`}
        aria-autocomplete="list"
        aria-activedescendant={active >= 0 && hits[active] ? optionId(active) : undefined}
        value={value}
        placeholder={placeholder}
        readOnly={readOnly}
        title={title}
        onChange={(e) => {
          onChange(branchName(e.target.value));
          setOpen(true);
          setActive(-1);
        }}
        onFocus={() => !readOnly && setOpen(true)}
        onKeyDown={onKeyDown}
        onBlur={(e) => {
          // Klick auf einen Vorschlag soll noch ankommen; der Eintrag liegt in der Liste
          if (e.relatedTarget && listRef.current?.contains(e.relatedTarget as Node)) return;
          setOpen(false);
          setActive(-1);
          onCommit();
        }}
      />
      {showClear && (
        <button
          type="button"
          className="icon-btn branch-combo-action"
          aria-label="Zielbranch abwählen"
          title="Zielbranch abwählen"
          onMouseDown={(e) => e.preventDefault()}
          onClick={() => choose('')}
        >
          <span className="material-icons" aria-hidden="true">close</span>
        </button>
      )}
      {showSave && onCreate && (
        <button
          type="button"
          className="icon-btn branch-combo-action"
          aria-label="Branch erstellen"
          title={`Branch ${value.trim()} erstellen`}
          onMouseDown={(e) => e.preventDefault()}
          onClick={() => {
            setOpen(false);
            setActive(-1);
            onCreate(value.trim());
          }}
        >
          <span className="material-icons" aria-hidden="true">save</span>
        </button>
      )}
      <div ref={listRef} role="listbox" id={`${id}-list`} aria-label="Branches" className="branch-combo-list" hidden={hits.length === 0 && !unknown}>
        {hits.map((name, index) => (
          <div
            key={name}
            id={optionId(index)}
            role="option"
            aria-selected={index === active}
            className={`branch-combo-option${index === active ? ' active' : ''}`}
            onMouseMove={() => setActive(index)}
            onMouseDown={(e) => e.preventDefault()}
            onClick={() => choose(name)}
          >
            {name}
          </div>
        ))}
        {unknown && <div className="branch-combo-hint muted">Neuer Branch: {value.trim()}</div>}
      </div>
    </div>
  );
}
