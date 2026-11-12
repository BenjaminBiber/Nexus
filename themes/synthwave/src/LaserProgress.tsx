import type { ProgressProps } from '@nexus/theme-sdk';

/** Bis zu dieser Schrittzahl gibt es ein Segment je Schritt, darüber einen durchgehenden Balken */
export const MAX_SEGMENTS = 12;

// Fortschritt als Laserbalken: „▮▮▮▯▯ 3/5 Schritte erledigt“, bei vielen Schritten ein Strahl in Prozent. Sind alle
// Schritte erledigt, meldet ein Spielhallen-Schild „STAGE CLEAR“ (nur Schmuck, der Text daneben sagt dasselbe).
export function LaserProgress({ done, total, label, caption }: ProgressProps) {
  const all = Math.max(0, Math.floor(Number.isFinite(total) ? total : 0));
  const value = Math.min(all, Math.max(0, Math.floor(Number.isFinite(done) ? done : 0)));
  const percent = all > 0 ? (value / all) * 100 : 0;
  return (
    <div
      className="synth-progress"
      role="progressbar"
      aria-label={label}
      aria-valuemin={0}
      aria-valuemax={all}
      aria-valuenow={value}
      data-complete={all > 0 && value === all ? '' : undefined}
    >
      {all > MAX_SEGMENTS ? (
        <span className="synth-laser synth-laser-beam" aria-hidden="true">
          <i style={{ width: `${percent}%` }} />
        </span>
      ) : (
        <span className="synth-laser" aria-hidden="true">
          {all === 0 ? <i /> : Array.from({ length: all }, (_, index) => <i key={index} data-on={index < value ? '' : undefined} />)}
        </span>
      )}
      <span className="synth-progress-text">
        {value}/{all} {caption}
      </span>
      {all > 0 && value === all && (
        <span className="synth-clear" aria-hidden="true">
          STAGE CLEAR
        </span>
      )}
    </div>
  );
}
