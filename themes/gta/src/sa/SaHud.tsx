import type { CSSProperties } from 'react';
import { MAX_WANTED, useGame } from '../data/GameContext.js';
import { formatClockSa, formatMoneySa } from '../shared/format.js';
import { useCountUp } from '../shared/useCountUp.js';

const STAR = 'M12 1.8l3 6.6 7.2.8-5.4 4.9 1.5 7.1L12 17.6l-6.3 3.6 1.5-7.1L1.8 9.2 9 8.4z';
const EMPTY = 'Claude-Verbrauch nicht verfügbar';

// Faust der Preview, selbst gezeichnet: Handgelenk, Handballen, vier Finger, Daumen
function Fist() {
  return (
    <svg viewBox="0 0 64 64" aria-hidden="true" focusable="false">
      <g fill="#e9e4d8" stroke="#000" strokeWidth="3.2" strokeLinejoin="round">
        <rect x="19" y="46" width="27" height="14" rx="2" />
        <rect x="11" y="27" width="43" height="24" rx="7" />
        <rect x="11" y="12" width="11.5" height="27" rx="5" />
        <rect x="21.5" y="9" width="11.5" height="29" rx="5" />
        <rect x="32" y="10" width="11.5" height="28" rx="5" />
        <rect x="42.5" y="14" width="11.5" height="25" rx="5" />
        <path d="M6 37q0-6 6-6h20q6 0 6 6t-6 6H12q-6 0-6-6z" />
      </g>
    </svg>
  );
}

// Waffenfeld: die Faust, darunter offene Aufgaben und Fragen wie die Munitionsanzeige
function Weapon({ tasks, questions }: { tasks: number; questions: number }) {
  return (
    <div className="gta-sa-weapon" title={`${tasks} offene Aufgaben, ${questions} offene Fragen`}>
      <Fist />
      <span className="gta-sa-ammo">
        {tasks}-{questions}
      </span>
    </div>
  );
}

const percent = (value: number) => Math.round(Math.min(1, Math.max(0, value)) * 100);

// HUD-Balken mit schwarzem Rand; ohne Wert leer und gedimmt
function Meter({ kind, name, value }: { kind: 'armor' | 'health'; name: string; value: number | null }) {
  const p = value === null ? null : percent(value);
  return (
    <i
      className={`gta-sa-bar gta-sa-${kind}`}
      role="progressbar"
      aria-label={name}
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={p ?? undefined}
      title={p === null ? EMPTY : `${name}: ${p} % frei`}
      data-empty={p === null ? '' : undefined}
      style={{ '--sa-p': `${p ?? 0}%` } as CSSProperties}
    />
  );
}

// Kontostand in Grün mit acht Stellen; zählt bei Änderung hoch
function Cash({ money }: { money: number }) {
  const { value, counting } = useCountUp(money);
  return (
    <div className="gta-sa-cash" title="Spielgeld: verdient mit erledigten Schritten und Tickets" data-up={counting ? '' : undefined}>
      {formatMoneySa(value)}
    </div>
  );
}

// Sechs Sterne, von rechts gefüllt; bei sechs blinken sie
function Stars({ wanted }: { wanted: number }) {
  const on = Math.max(0, Math.min(wanted, MAX_WANTED));
  return (
    <div
      className="gta-sa-stars"
      role="img"
      aria-label={`Fahndungslevel ${on} von ${MAX_WANTED}`}
      title="Fahndungslevel = blockierte Tickets"
      data-blink={on >= MAX_WANTED ? '' : undefined}
    >
      {Array.from({ length: MAX_WANTED }, (_, i) => (
        <svg key={i} className={i >= MAX_WANTED - on ? 'gta-sa-star on' : 'gta-sa-star'} viewBox="0 0 24 24" aria-hidden="true">
          <path d={STAR} />
        </svg>
      ))}
    </div>
  );
}

// HUD-Block oben rechts wie in der Preview: Faust, Uhr, Panzerung (7-Tage-Kontingent) und Gesundheit
// (5-Stunden-Kontingent), darunter Geld und Sterne
export function SaHud() {
  const game = useGame();
  return (
    <div className="gta-sa-hud" data-gta-part="hud">
      <Weapon tasks={game.tasks} questions={game.questions} />
      <div className="gta-sa-status">
        <span className="gta-sa-clock">{formatClockSa(game.now)}</span>
        <Meter kind="armor" name="Panzerung: Rest des 7-Tage-Kontingents" value={game.quota7d} />
        <Meter kind="health" name="Gesundheit: Rest des 5-Stunden-Kontingents" value={game.quota5h} />
      </div>
      <Cash money={game.moneyShown} />
      <Stars wanted={game.wanted} />
    </div>
  );
}
