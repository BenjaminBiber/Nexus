import { useState } from 'react';
import { EFFORT_OPTIONS } from '../../api/dispatchTypes.js';
import { AccountMultiSelect, useAccountChoices } from './AccountMultiSelect.js';
import type { ProfileRow } from './dispatchForm.js';

interface Props {
  rows: ProfileRow[];
  onChange: (rows: ProfileRow[]) => void;
  onMove: (from: number, to: number) => void;
}

// Tabelle der Profile: Name, Modell, Effort, Schritte, Minuten, Accounts (Mehrfachauswahl, leer = jeder aktive); hinzufügen, entfernen und umsortieren (die Reihenfolge ist
// die Eskalationsreihenfolge). Umsortiert wird per Ziehen am Griff oder per Pfeiltasten auf dem Griff.
export function DispatchProfilesEditor({ rows, onChange, onMove }: Props) {
  const [dragFrom, setDragFrom] = useState<number | null>(null);
  const [dragOver, setDragOver] = useState<number | null>(null);
  const accounts = useAccountChoices();
  const set = (index: number, patch: Partial<ProfileRow>) => onChange(rows.map((r, i) => (i === index ? { ...r, ...patch } : r)));
  const endDrag = () => { setDragFrom(null); setDragOver(null); };
  return (
    <div className="dispatch-table">
      <div className="dispatch-row profile head" aria-hidden="true">
        <span /><span>Name</span><span>Modell</span><span>Effort</span><span>Schritte</span><span>Minuten</span><span>Accounts</span><span />
      </div>
      {rows.map((row, i) => (
        <div
          className={`dispatch-row profile${dragFrom === i ? ' dragging' : ''}${dragOver === i && dragFrom !== i ? ' drop-target' : ''}`}
          key={i}
          onDragOver={(e) => {
            if (dragFrom === null) return;
            e.preventDefault();
            e.dataTransfer.dropEffect = 'move';
            if (dragOver !== i) setDragOver(i);
          }}
          onDrop={(e) => {
            if (dragFrom === null) return;
            e.preventDefault();
            onMove(dragFrom, i);
            endDrag();
          }}
        >
          <button
            type="button"
            className="drag-handle"
            draggable
            aria-label={`Profil ${i + 1} verschieben`}
            title="Ziehen oder mit Pfeil hoch/runter verschieben"
            onDragStart={(e) => {
              e.dataTransfer.effectAllowed = 'move';
              e.dataTransfer.setData('text/plain', String(i));
              const rowEl = e.currentTarget.closest('.dispatch-row');
              if (rowEl) e.dataTransfer.setDragImage(rowEl, 0, 0);
              setDragFrom(i);
            }}
            onDragEnd={endDrag}
            onKeyDown={(e) => {
              if (e.key !== 'ArrowUp' && e.key !== 'ArrowDown') return;
              e.preventDefault();
              const to = e.key === 'ArrowUp' ? i - 1 : i + 1;
              if (to < 0 || to >= rows.length) return;
              onMove(i, to);
              // Fokus folgt dem Profil an seine neue Stelle (Zeilen sind nach Position geschlüsselt)
              const handles = e.currentTarget.closest('.dispatch-table')?.querySelectorAll<HTMLElement>('.drag-handle');
              requestAnimationFrame(() => handles?.[to]?.focus());
            }}
          >⋮⋮</button>
          <input aria-label={`Profil ${i + 1} Name`} value={row.name} onChange={(e) => set(i, { name: e.target.value })} />
          <input aria-label={`Profil ${i + 1} Modell`} value={row.model} placeholder="sonnet, opus …" onChange={(e) => set(i, { model: e.target.value })} />
          <select aria-label={`Profil ${i + 1} Effort`} value={row.effort} onChange={(e) => set(i, { effort: e.target.value })}>
            <option value="">Standard</option>
            {EFFORT_OPTIONS.map((e) => <option key={e} value={e}>{e}</option>)}
          </select>
          <input aria-label={`Profil ${i + 1} Schritte`} inputMode="numeric" value={row.maxTurns} onChange={(e) => set(i, { maxTurns: e.target.value })} />
          <input aria-label={`Profil ${i + 1} Minuten`} inputMode="numeric" value={row.timeoutMinutes} onChange={(e) => set(i, { timeoutMinutes: e.target.value })} />
          <AccountMultiSelect label={`Profil ${i + 1} Accounts`} value={row.accounts} accounts={accounts} emptyText="alle" onChange={(accounts) => set(i, { accounts })} />
          <button type="button" className="btn" onClick={() => onChange(rows.filter((_, j) => j !== i))}>Entfernen</button>
        </div>
      ))}
      <button type="button" className="btn" onClick={() => onChange([...rows, { name: '', model: '', effort: '', maxTurns: '', timeoutMinutes: '', accounts: '' }])}>+ Profil</button>
    </div>
  );
}
