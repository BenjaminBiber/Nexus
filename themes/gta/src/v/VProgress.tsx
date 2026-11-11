import type { ProgressProps } from '@nexus/theme-sdk';

/** Bis zu dieser Schrittzahl gibt es ein Segment je Schritt, darüber einen durchgehenden Balken */
export const MAX_SEGMENTS = 12;

const count = (n: number) => Math.max(0, Math.floor(Number.isFinite(n) ? n : 0));

// Fortschritt als Segmentbalken wie auf den Karten der Preview: „▮▮▯▯ 2/4 SCHRITTE“, bei vielen Schritten ein Balken in Prozent
export function VProgress({ done, total, label, caption }: ProgressProps) {
  const all = count(total);
  const value = Math.min(all, count(done));
  const percent = all > 0 ? (value / all) * 100 : 0;
  return (
    <div
      className="gta-v-progress"
      role="progressbar"
      aria-label={label}
      aria-valuemin={0}
      aria-valuemax={all}
      aria-valuenow={value}
      data-complete={all > 0 && value === all ? '' : undefined}
    >
      <span className="gta-v-progress-bar" aria-hidden="true" data-beam={all > MAX_SEGMENTS ? '' : undefined}>
        {all > MAX_SEGMENTS ? (
          <i data-on="" style={{ width: `${percent}%` }} />
        ) : all === 0 ? (
          <i />
        ) : (
          Array.from({ length: all }, (_, index) => <i key={index} data-on={index < value ? '' : undefined} />)
        )}
      </span>
      <span className="gta-v-progress-text">
        {value}/{all} {caption}
      </span>
    </div>
  );
}
