import type { ConfigPatchOpDto, SuggestionDto } from '../../api/dispatchTypes.js';

const FIELD_LABELS: Record<string, string> = { maxTurns: 'Schritte', timeoutMinutes: 'Zeitlimit (min)', model: 'Modell', effort: 'Effort' };

// Ein Änderungsschritt in Worten der Oberfläche, z. B. „standard: Schritte 30 → 45“
export function describeOp(op: ConfigPatchOpDto): string {
  const value = (v: unknown) => (v === null || v === undefined ? '–' : Array.isArray(v) ? v.join(' › ') : String(v));
  const [head, a = '', b = ''] = op.path.split('.');
  const label = head === 'profiles' ? `${a}: ${FIELD_LABELS[b] ?? b}`
    : head === 'rules' ? `Regel ${Number(a) + 1}: Profil`
      : head === 'defaultProfile' ? 'Standardprofil' : 'Leiter';
  return `${label} ${value(op.from)} → ${value(op.to)}`;
}

interface SuggestionCardProps {
  suggestion: SuggestionDto;
  /** Formular hat ungespeicherte Änderungen – Übernehmen gesperrt */
  locked: boolean;
  busy: boolean;
  dismissed?: boolean;
  onApply: () => void;
  onDismiss: () => void;
  onRestore: () => void;
}

// Ein Optimierungs-Vorschlag: Titel, Begründung, Änderung und Knöpfe.
export function SuggestionCard({ suggestion: s, locked, busy, dismissed = false, onApply, onDismiss, onRestore }: SuggestionCardProps) {
  return (
    <article className={`suggestion-card${s.patch ? '' : ' hint'}`} aria-label={s.title}>
      <header className="suggestion-head">
        <h4>{s.title}</h4>
        {s.source === 'claude' && <span className="suggestion-claude">Claude</span>}
      </header>
      {s.reason && <p className="muted">{s.reason}</p>}
      {s.patch && (
        <ul className="suggestion-diff">
          {s.patch.map((op) => <li key={op.path}><code>{describeOp(op)}</code></li>)}
        </ul>
      )}
      <div className="actions">
        {dismissed ? (
          <button type="button" className="btn" disabled={busy} onClick={onRestore}>Wieder einblenden</button>
        ) : (
          <>
            {s.patch && (
              <button type="button" className="btn primary" disabled={busy || locked} title={locked ? 'Erst die offenen Änderungen speichern' : undefined} onClick={onApply}>
                Übernehmen
              </button>
            )}
            <button type="button" className="btn" disabled={busy} onClick={onDismiss}>Ausblenden</button>
          </>
        )}
      </div>
    </article>
  );
}
