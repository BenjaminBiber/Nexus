// XP-Leiste wie im HUD: Hintergrund und Füllung aus experience_bar_background/progress (182×5 Spielpixel), Füllung per clip-path
// beschnitten (Textur bleibt ungestreckt), Level-Zahl darüber in XP-Grün mit schwarzem Rand, Grenzstrich bei `limit` %.
import type { ReactNode } from 'react';

/** Farbzustand der Leiste: innerhalb der Grenze, darüber, kritisch */
export type XpTone = 'ok' | 'over' | 'crit';

export function XpBar({
  percent,
  limit,
  width,
  label,
  tone = 'ok',
  decorative = false,
  level,
}: {
  /** Füllung in Prozent; `null` heißt unbekannt (leere Leiste, „–“ als Zahl) */
  percent: number | null;
  /** Grenzstrich in Prozent; `null` lässt ihn weg */
  limit: number | null;
  /** Breite in Spielpixeln (×2) oder `'full'` für die Breite des Elternteils */
  width: number | 'full';
  label: string;
  tone?: XpTone;
  /** Für Leisten, deren Bedeutung schon außen steht (z. B. im Slot „Progress“): kein eigener Eintrag im Barrierefreiheitsbaum */
  decorative?: boolean;
  /** Eigene Anzeige über der Leiste; Standard ist die Prozentzahl */
  level?: ReactNode;
}) {
  const shown = percent === null ? 0 : Math.min(100, Math.max(0, percent));
  const style = width === 'full' ? undefined : { width: `${width * 2}px` };
  // Dekorative Leisten tragen weder Rolle noch Beschriftung, sondern sind ganz verborgen
  const aria = decorative
    ? ({ 'aria-hidden': true } as const)
    : ({ role: 'img', 'aria-label': `${label}: ${percent === null ? 'unbekannt' : `${percent} %`}` } as const);
  return (
    <span className={`mc-xp${tone === 'ok' ? '' : ` mc-xp--${tone}`}`} style={style} {...aria}>
      <span className="mc-xp-level" aria-hidden="true">
        {level ?? (percent === null ? '–' : percent)}
      </span>
      <span className="mc-xp-bar" aria-hidden="true">
        <span className="mc-xp-fill" style={{ clipPath: `inset(0 ${100 - shown}% 0 0)` }} />
        {limit !== null && <span className="mc-xp-limit" style={{ left: `${limit}%` }} />}
      </span>
    </span>
  );
}
