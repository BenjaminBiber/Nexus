import { SlotBoundary } from '../theme/SlotBoundary.js';
import { useDisplayedTheme } from '../theme/useDisplayedTheme.js';
import { useSlot } from '../theme/useSlot.js';

// Fortschritt als Ring mit Prozentzahl und „x / y Schritte erledigt“ daneben (Seitenleiste des Ticket-Panels).
// `caption` ersetzt „Schritte erledigt“, etwa für die Tickets eines Dokuments. Der Slot „Progress“ des Themes darf ihn ersetzen.
export function ProgressRing({ done, total, label = 'Fortschritt der Schritte', caption = 'Schritte erledigt' }: {
  done: number;
  total: number;
  label?: string;
  caption?: string;
}) {
  const theme = useDisplayedTheme();
  const Custom = useSlot('Progress');
  const standard = <StandardRing done={done} total={total} label={label} caption={caption} />;
  if (!Custom) return standard;
  return (
    <SlotBoundary fallback={standard} resetKey={theme}>
      <Custom done={done} total={total} label={label} caption={caption} />
    </SlotBoundary>
  );
}

// Ring mit Prozentzahl und Zähler
function StandardRing({ done, total, label, caption }: { done: number; total: number; label: string; caption: string }) {
  const percent = total === 0 ? 0 : Math.round((done / total) * 100);
  const radius = 34;
  const circumference = 2 * Math.PI * radius;
  return (
    <div className="progress-ring">
      <div className="progress-ring-chart" role="img" aria-label={`${label}: ${percent} %`}>
        <svg viewBox="0 0 84 84" aria-hidden="true">
          <circle className="progress-ring-track" cx="42" cy="42" r={radius} />
          <circle
            className="progress-ring-value"
            cx="42"
            cy="42"
            r={radius}
            strokeDasharray={circumference}
            strokeDashoffset={circumference * (1 - percent / 100)}
          />
        </svg>
        <span className="progress-ring-percent">{percent}%</span>
      </div>
      <div className="progress-ring-text">
        <span className="progress-ring-count">
          {done} <span className="progress-ring-total">/ {total}</span>
        </span>
        <span className="muted">{caption}</span>
      </div>
    </div>
  );
}
