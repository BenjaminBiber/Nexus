import type { ReactNode } from 'react';

// Fortschritt einer Spec oder eines Tickets: „x von y erledigt“ mit grünem Balken.
// `text` ersetzt die Beschriftung (z. B. Kontextauslastung), `tone` färbt den Balken bei hoher Auslastung.
export function Progress({ done, total, label = 'Fortschritt der Spec', text, tone = 'ok' }: { done: number; total: number; label?: string; text?: ReactNode; tone?: 'ok' | 'warn' | 'crit' }) {
  const percent = total === 0 ? 0 : Math.min(100, Math.round((done / total) * 100));
  return (
    <div className="progress">
      <span className="progress-text">
        {text ?? (
          <>
            {done} von {total} erledigt
          </>
        )}
      </span>
      <div className={`progress-bar${tone === 'ok' ? '' : ` ${tone}`}`} role="img" aria-label={label}>
        <span style={{ width: `${percent}%` }} />
      </div>
    </div>
  );
}
