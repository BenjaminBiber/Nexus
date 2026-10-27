import type { SessionDisplayState } from '../api/types.js';
import { STATE_LABEL } from '../sessions/format.js';

// Gewählte Zustände; leer bedeutet „alle“
export type StateFilter = ReadonlySet<SessionDisplayState>;

// Voreinstellung der Sessions-Seite: läuft + untätig
export const DEFAULT_STATE_FILTER: StateFilter = new Set<SessionDisplayState>(['running', 'idle']);

// Filter nach Zustand der Sessions als Mehrfachauswahl-Chips, dazu „nur Favoriten“.
export function SessionFilter(props: {
  value: StateFilter;
  onChange: (value: StateFilter) => void;
  onlyFavorites: boolean;
  onOnlyFavorites: (value: boolean) => void;
}) {
  const states = Object.keys(STATE_LABEL) as SessionDisplayState[];
  // Zustand in die Auswahl aufnehmen oder daraus entfernen
  const toggle = (state: SessionDisplayState) => {
    const next = new Set(props.value);
    if (!next.delete(state)) next.add(state);
    props.onChange(next);
  };
  const isAll = props.value.size === 0;
  return (
    <div className="filters" role="group" aria-label="Filter">
      <span className="muted">Zustand</span>
      <button type="button" className={`fchip${isAll ? ' on' : ''}`} aria-pressed={isAll} onClick={() => props.onChange(new Set())}>
        alle
      </button>
      {states.map((state) => {
        const on = props.value.has(state);
        return (
          <button key={state} type="button" className={`fchip${on ? ' on' : ''}`} aria-pressed={on} onClick={() => toggle(state)}>
            {STATE_LABEL[state]}
          </button>
        );
      })}
      <label className="check">
        <input type="checkbox" checked={props.onlyFavorites} onChange={(e) => props.onOnlyFavorites(e.target.checked)} /> nur Favoriten
      </label>
    </div>
  );
}
