import type { CSSProperties } from 'react';
import type { ProgressProps } from '@nexus/theme-sdk';

const count = (n: number) => Math.max(0, Math.floor(Number.isFinite(n) ? n : 0));

// Fortschritt als durchgehender Balken mit schwarzem Rand wie die HUD-Balken der Preview, darunter „2/4 Schritte“
export function SaProgress({ done, total, label, caption }: ProgressProps) {
  const all = count(total);
  const value = Math.min(all, count(done));
  const percent = all > 0 ? Math.round((value / all) * 1000) / 10 : 0;
  return (
    <div
      className="gta-sa-progress"
      role="progressbar"
      aria-label={label}
      aria-valuemin={0}
      aria-valuemax={all}
      aria-valuenow={value}
      data-complete={all > 0 && value === all ? '' : undefined}
    >
      <span className="gta-sa-progress-bar" aria-hidden="true" style={{ '--sa-p': `${percent}%` } as CSSProperties} />
      <span className="gta-sa-progress-text">
        {value}/{all} {caption}
      </span>
    </div>
  );
}
