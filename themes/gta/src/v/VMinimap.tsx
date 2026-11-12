import type { CSSProperties } from 'react';
import { NavLink } from '@nexus/theme-sdk';
import { useGame } from '../data/GameContext.js';
import type { Blip } from '../data/derive.js';
import { openTickets, percentOf } from '../shared/format.js';

const W = 188;
const H = 106;

interface Bar {
  label: string;
  name: string;
  value: number | null;
  color: string;
  empty: string;
}

// Straßenkarte der Preview: Wasser oben rechts, Häuserblöcke, Straßen, eine markierte Route, Norden oben links
function Streets() {
  return (
    <>
      <rect width={W} height={H} fill="#2a363d" />
      <path d="M120 0h68v44l-30 8-22-14z" fill="#35556f" />
      <g fill="#34424a">
        <rect x="8" y="10" width="38" height="24" />
        <rect x="56" y="10" width="44" height="24" />
        <rect x="8" y="46" width="38" height="50" />
        <rect x="56" y="46" width="30" height="22" />
        <rect x="96" y="60" width="50" height="36" />
        <rect x="56" y="78" width="30" height="18" />
      </g>
      <path d="M0 40h112l40 22h36M51 0v106M91 40v66M0 101h188" stroke="#6c7b83" strokeWidth="4" fill="none" />
      <path d="M94 62V40H51V22" stroke="var(--gta-c-in_planning)" strokeWidth="4" fill="none" strokeLinejoin="round" />
    </>
  );
}

function Blips({ blips }: { blips: Blip[] }) {
  return (
    <g stroke="#000" strokeWidth="1.5">
      {blips.map((blip) => (
        <circle
          key={blip.id}
          className="gta-v-blip"
          data-blip={blip.id}
          data-blink={blip.blocked ? '' : undefined}
          cx={blip.x * W}
          cy={blip.y * H}
          r="4.5"
          fill={`var(--gta-c-${blip.column})`}
        />
      ))}
    </g>
  );
}

// Spieler-Pfeil und Nordmarke
function Marks() {
  return (
    <>
      <path d="M94 55l6 13-6-3-6 3z" fill="#fff" stroke="#000" strokeWidth="1.5" />
      <circle cx="12" cy="12" r="7" fill="#000" opacity=".6" />
      <text x="12" y="15.5" fontSize="9" fontWeight="700" fill="#fff" textAnchor="middle" fontFamily="Arial, sans-serif">
        N
      </text>
    </>
  );
}

function Meter({ bar }: { bar: Bar }) {
  const p = bar.value === null ? null : percentOf(bar.value);
  const style = { '--v-c': bar.color, '--v-p': `${p ?? 0}%` } as CSSProperties;
  return (
    <i
      className="gta-v-bar"
      role="progressbar"
      aria-label={bar.name}
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={p ?? undefined}
      title={p === null ? bar.empty : `${bar.name}: ${p} %`}
      data-empty={p === null ? '' : undefined}
      style={style}
    />
  );
}

// Minimap unten links: eckige Karte mit den offenen Tickets als Blips (blockierte blinken), als Ganzes ein Link zum
// Board. Darunter Fortschritt des Boards und die beiden Claude-Kontingente als Balken.
export function VMinimap() {
  const { blips, open, loaded, boardProgress, quota5h, quota7d } = useGame();
  const noBoard = loaded ? 'Fortschritt des Boards: noch keine Tickets' : 'Fortschritt des Boards: kein Board-Stand';
  const bars: Bar[] = [
    { label: 'BOARD', name: 'Fortschritt des Boards', value: loaded ? boardProgress : null, color: 'var(--gta-ok)', empty: noBoard },
    { label: '5 STD', name: 'Claude: Rest des 5-Stunden-Kontingents', value: quota5h, color: 'var(--gta-c-planned)', empty: 'Claude-Verbrauch nicht verfügbar' },
    { label: '7 T', name: 'Claude: Rest des 7-Tage-Kontingents', value: quota7d, color: 'var(--gta-warn)', empty: 'Claude-Verbrauch nicht verfügbar' },
  ];
  return (
    <div className="gta-v-radar" data-gta-part="radar">
      <NavLink id="board" className="gta-v-map">
        {() => (
          <>
            <svg viewBox={`0 0 ${W} ${H}`} aria-hidden="true" focusable="false">
              <Streets />
              <Blips blips={blips} />
              <Marks />
            </svg>
            <span className="gta-sr">{loaded ? `Minimap: ${openTickets(open)}, zum Board` : 'Minimap, zum Board'}</span>
          </>
        )}
      </NavLink>
      <div className="gta-v-bars">
        {bars.map((bar) => (
          <Meter key={bar.label} bar={bar} />
        ))}
      </div>
      <div className="gta-v-bar-labels" aria-hidden="true">
        {bars.map((bar) => (
          <span key={bar.label} className="gta-v-bar-label">
            {bar.label}
          </span>
        ))}
      </div>
    </div>
  );
}
