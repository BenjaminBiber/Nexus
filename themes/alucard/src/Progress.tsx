// Gleiche Datei in themes/dracula und themes/alucard (ein Test vergleicht beide)
import { useId } from 'react';
import type { ProgressProps } from '@nexus/theme-sdk';

const RADIUS = 34;
const CIRCUMFERENCE = 2 * Math.PI * RADIUS;

// Fortschritt wie der Standard-Ring im Ticket-Panel (gleiche Klassen, also gleiche Maße und Schrift), der Bogen läuft aber
// von Purple nach Pink der Palette. Der Slot ist zugleich der Grund, warum Nexus main.js lädt: ohne Slot im Manifest gälte
// das Mermaid-Aussehen nicht.
export function DraculaProgress({ done, total, label, caption }: ProgressProps) {
  const gradient = 'dracula-ring-' + useId().replace(/[^a-zA-Z0-9]/g, '');
  const percent = total > 0 ? Math.min(100, Math.max(0, Math.round((done / total) * 100))) : 0;
  return (
    <div className="progress-ring dracula-progress">
      <div className="progress-ring-chart" role="img" aria-label={`${label}: ${percent} %`}>
        <svg viewBox="0 0 84 84" aria-hidden="true">
          <defs>
            <linearGradient id={gradient} x1="0" y1="0" x2="1" y2="1">
              <stop offset="0%" style={{ stopColor: 'var(--dracula-purple)' }} />
              <stop offset="100%" style={{ stopColor: 'var(--dracula-pink)' }} />
            </linearGradient>
          </defs>
          <circle className="progress-ring-track" cx="42" cy="42" r={RADIUS} />
          <circle
            className="progress-ring-value"
            cx="42"
            cy="42"
            r={RADIUS}
            style={{ stroke: `url(#${gradient})` }}
            strokeDasharray={CIRCUMFERENCE}
            strokeDashoffset={CIRCUMFERENCE * (1 - percent / 100)}
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
