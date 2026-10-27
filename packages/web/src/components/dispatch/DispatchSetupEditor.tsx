import type { SetupRow } from './dispatchForm.js';

// Einrichtung neuer Worktrees: Befehle, die der Worker nacheinander vor Claude ausführt (Ordner relativ zum Worktree).
export function DispatchSetupEditor({ rows, onChange }: { rows: SetupRow[]; onChange: (rows: SetupRow[]) => void }) {
  const set = (index: number, patch: Partial<SetupRow>) => onChange(rows.map((r, i) => (i === index ? { ...r, ...patch } : r)));
  return (
    <div className="dispatch-table">
      {rows.length > 0 && (
        <div className="dispatch-row setup head" aria-hidden="true">
          <span>Ordner im Worktree</span><span>Befehl</span><span />
        </div>
      )}
      {rows.map((row, i) => (
        <div className="dispatch-row setup" key={i}>
          <input aria-label={`Einrichtung ${i + 1} Ordner`} value={row.cwd} placeholder="BackofficeNextGen (leer = Worktree)" onChange={(e) => set(i, { cwd: e.target.value })} />
          <input aria-label={`Einrichtung ${i + 1} Befehl`} value={row.run} placeholder="npm ci --prefer-offline --no-audit" onChange={(e) => set(i, { run: e.target.value })} />
          <button type="button" className="btn" aria-label={`Einrichtung ${i + 1} entfernen`} onClick={() => onChange(rows.filter((_, j) => j !== i))}>Entfernen</button>
        </div>
      ))}
      <button type="button" className="btn" onClick={() => onChange([...rows, { cwd: '', run: '' }])}>+ Befehl</button>
    </div>
  );
}
