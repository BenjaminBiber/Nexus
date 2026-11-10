// Slot „Progress“: der Fortschritt des Hosts (z. B. erledigte Schritte eines Tickets) als XP-Leiste in voller Breite;
// die Zahl über der Leiste zeigt die erledigten Schritte, die Beschriftung steht daneben in Grau.
import type { ProgressProps } from '@nexus/theme-sdk';
import { XpBar } from '../usage/XpBar.js';

export function McProgress({ done, total, label, caption }: ProgressProps) {
  const percent = total > 0 ? Math.round((done / total) * 100) : 0;
  return (
    <div role="progressbar" aria-valuemin={0} aria-valuenow={done} aria-valuemax={total} aria-label={label} className="mc-progress">
      <XpBar percent={percent} limit={null} width="full" label={label} decorative level={done} />
      <span className="mc-shadow--gray">{caption}</span>
    </div>
  );
}
