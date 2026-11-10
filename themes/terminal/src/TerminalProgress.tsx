import type { ProgressProps } from '@nexus/theme-sdk';

/** Breite des Balkens in Zeichen */
export const PROGRESS_WIDTH = 10;

// Fortschritt wie in einer Shell: „[#####·····] 5/10 Schritte erledigt“
export function TerminalProgress({ done, total, label, caption }: ProgressProps) {
  const ratio = total > 0 ? Math.min(1, Math.max(0, done / total)) : 0;
  const filled = Math.round(ratio * PROGRESS_WIDTH);
  return (
    <div className="term-progress" role="progressbar" aria-label={label} aria-valuemin={0} aria-valuemax={total} aria-valuenow={done}>
      <span className="term-progress-bar" aria-hidden="true">
        <span className="term-progress-edge">[</span>
        <span className="term-progress-fill">{'#'.repeat(filled)}</span>
        <span className="term-progress-rest">{'·'.repeat(PROGRESS_WIDTH - filled)}</span>
        <span className="term-progress-edge">]</span>
      </span>
      <span className="term-progress-count">
        {done}/{total}
      </span>
      <span className="term-progress-caption">{caption}</span>
    </div>
  );
}
