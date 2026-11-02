import type { RuleRow } from './dispatchForm.js';
import { ProfileOptions } from './ProfileOptions.js';

// Regeln: erste passende gewinnt; alle ausgefüllten Bedingungen müssen zutreffen.
export function DispatchRulesEditor({ rows, profiles, onChange }: { rows: RuleRow[]; profiles: string[]; onChange: (rows: RuleRow[]) => void }) {
  const set = (index: number, patch: Partial<RuleRow>) => onChange(rows.map((r, i) => (i === index ? { ...r, ...patch } : r)));
  return (
    <div className="dispatch-table">
      <div className="dispatch-row rule head" aria-hidden="true">
        <span>Labels (eines davon)</span><span>Priorität</span><span>ab Dateien</span><span>ab Zeichen Plan</span><span>Profil</span><span>Accounts</span><span />
      </div>
      {rows.map((row, i) => (
        <div className="dispatch-row rule" key={i}>
          <input aria-label={`Regel ${i + 1} Labels`} value={row.labels} placeholder="doku, klein" onChange={(e) => set(i, { labels: e.target.value })} />
          <input aria-label={`Regel ${i + 1} Priorität`} value={row.priority} placeholder="urgent" onChange={(e) => set(i, { priority: e.target.value })} />
          <input aria-label={`Regel ${i + 1} ab Dateien`} inputMode="numeric" value={row.filesMin} onChange={(e) => set(i, { filesMin: e.target.value })} />
          <input aria-label={`Regel ${i + 1} ab Zeichen`} inputMode="numeric" value={row.planMinChars} onChange={(e) => set(i, { planMinChars: e.target.value })} />
          <select aria-label={`Regel ${i + 1} Profil`} value={row.profile} onChange={(e) => set(i, { profile: e.target.value })}>
            <option value="">–</option>
            <ProfileOptions names={profiles} value={row.profile} />
          </select>
          <input aria-label={`Regel ${i + 1} Accounts`} value={row.accounts} placeholder="wie Profil" onChange={(e) => set(i, { accounts: e.target.value })} />
          <button type="button" className="btn" onClick={() => onChange(rows.filter((_, j) => j !== i))}>Entfernen</button>
        </div>
      ))}
      <button type="button" className="btn" onClick={() => onChange([...rows, { labels: '', priority: '', filesMin: '', planMinChars: '', profile: '', accounts: '' }])}>+ Regel</button>
    </div>
  );
}
