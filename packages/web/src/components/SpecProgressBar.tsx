import type { DbColumnId, DbSpecProgressDto } from '../api/dbTypes.js';

// Segmente des Balkens in Statusfarben (data-status wie die Board-Spalten); „offen“ in der Farbe des Backlogs
const SEGMENTS: Array<{ key: 'done' | 'blocked' | 'waiting' | 'inProgress' | 'open'; status: DbColumnId }> = [
  { key: 'done', status: 'done' },
  { key: 'blocked', status: 'blocked' },
  { key: 'waiting', status: 'waiting' },
  { key: 'inProgress', status: 'in_progress' },
  { key: 'open', status: 'backlog' },
];

// Text des Stands: „x von y erledigt, n blockiert, n wartet, n in Arbeit, n offen“
export function specProgressText(p: DbSpecProgressDto): string {
  if (p.total === 0) return 'keine Tickets';
  return `${p.done} von ${p.total} erledigt, ${p.blocked} blockiert, ${p.waiting} wartet, ${p.inProgress} in Arbeit, ${p.open} offen`;
}

// Kreisdiagramm-Maße (16 px wie die Dateisymbole): ein Kreis mit Strichbreite = Durchmesser füllt die Fläche als Torte
const PIE_R = 3.5;
const PIE_C = 2 * Math.PI * PIE_R;

// Fortschritt einer Spec als kleines Kreisdiagramm (gleiche Segmente und Farben wie der Balken); der Stand steht im Hinweis.
export function SpecProgressPie({ progress }: { progress: DbSpecProgressDto }) {
  const text = specProgressText(progress);
  const dropped = progress.notNeeded ? ` (${progress.notNeeded} entfallen)` : '';
  let offset = 0;
  return (
    <svg className="spec-progress-pie" width="16" height="16" viewBox="0 0 16 16" role="img" aria-label={`Fortschritt ${progress.spec}: ${text}${dropped}`}>
      <title>{`${text}${dropped}`}</title>
      <circle className="spec-progress-pie-ground" cx="8" cy="8" r="7" />
      <g transform="rotate(-90 8 8)">
        {progress.total > 0 &&
          SEGMENTS.filter((s) => progress[s.key] > 0).map((s) => {
            const length = (progress[s.key] / progress.total) * PIE_C;
            const segment = (
              <circle
                key={s.key}
                data-status={s.status}
                data-part={s.key}
                cx="8"
                cy="8"
                r={PIE_R}
                fill="none"
                strokeWidth={PIE_R * 2}
                strokeDasharray={`${length} ${PIE_C}`}
                strokeDashoffset={-offset}
              />
            );
            offset += length;
            return segment;
          })}
      </g>
    </svg>
  );
}

// Fortschritt einer Spec aus dem Board als gestapelter Balken (erledigt, blockiert, wartet, in Arbeit, offen) mit Text.
export function SpecProgressBar({ progress }: { progress: DbSpecProgressDto }) {
  const text = specProgressText(progress);
  const dropped = progress.notNeeded ? ` (${progress.notNeeded} entfallen)` : '';
  return (
    <div className="spec-progress" title={`${progress.spec}: ${text}${dropped}`}>
      <span className="progress-text">{text}</span>
      <div className="spec-progress-bar" role="img" aria-label={`Fortschritt ${progress.spec}: ${text}${dropped}`}>
        {progress.total > 0 &&
          SEGMENTS.filter((s) => progress[s.key] > 0).map((s) => (
            <span key={s.key} data-status={s.status} data-part={s.key} style={{ width: `${(progress[s.key] / progress.total) * 100}%` }} />
          ))}
      </div>
    </div>
  );
}
