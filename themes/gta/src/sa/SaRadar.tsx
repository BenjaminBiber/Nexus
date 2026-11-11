import { useId } from 'react';
import { NavLink } from '@nexus/theme-sdk';
import { useGame } from '../data/GameContext.js';
import type { Blip } from '../data/derive.js';
import { openTickets } from '../shared/format.js';

/** Seitenlänge des Quadrats im Kreis, auf das die Blip-Positionen (0..1) fallen; passt ganz in den Kreis (r 71) */
export const RADAR_INNER = 100;
const C = 80;
const BLIP = 9;

// Karte der Preview: Wasser oben, drei Viertel, Straßen im Raster und eine Diagonale
function Streets() {
  return (
    <g className="gta-sa-radar-map">
      <rect width="160" height="160" fill="#6f725e" />
      <path d="M0 0H160V30C124 42 98 26 62 36S22 28 0 42Z" fill="#4f7396" />
      <rect x="40" y="60" width="42" height="38" fill="#3f9a3a" opacity=".6" />
      <rect x="88" y="104" width="34" height="26" fill="#a060c8" opacity=".6" />
      <rect x="88" y="60" width="34" height="38" fill="#d8c040" opacity=".5" />
      <path d="M0 56H160M0 101H160M0 134H160M36 38V160M85 32V160M125 36V160" stroke="#26292c" strokeWidth="5" />
      <path d="M-10 152L170 66" stroke="#26292c" strokeWidth="7" />
    </g>
  );
}

function Blips({ blips }: { blips: Blip[] }) {
  return (
    <g stroke="#000" strokeWidth="1.6">
      {blips.map((blip) => (
        <rect
          key={blip.id}
          className="gta-sa-blip"
          data-blip={blip.id}
          data-blink={blip.blocked ? '' : undefined}
          x={C + (blip.x - 0.5) * RADAR_INNER - BLIP / 2}
          y={C + (blip.y - 0.5) * RADAR_INNER - BLIP / 2}
          width={BLIP}
          height={BLIP}
          fill={`var(--gta-c-${blip.column})`}
        />
      ))}
    </g>
  );
}

// Rundes Radar unten in der Menüspalte: Karte im Kreis, je offenem Ticket ein Blip in der Spaltenfarbe (blockierte
// blinken), Spieler-Pfeil in der Mitte, schwarzer Ring mit „N“. Als Ganzes ein Link zum Board.
export function SaRadar() {
  const { blips, open } = useGame();
  const clip = `${useId()}-clip`;
  return (
    <div className="gta-sa-radar" data-gta-part="radar">
      <NavLink id="board" className="gta-sa-radar-link">
        {() => (
          <>
            <svg viewBox="0 0 160 160" aria-hidden="true" focusable="false">
              <defs>
                <clipPath id={clip}>
                  <circle cx={C} cy={C} r="71" />
                </clipPath>
              </defs>
              <circle cx={C} cy={C} r="71" fill="rgba(0,0,0,.5)" />
              <g clipPath={`url(#${clip})`}>
                <Streets />
                <Blips blips={blips} />
                <path d="M80 68l7.5 18-7.5-4.5-7.5 4.5z" fill="#fff" stroke="#000" strokeWidth="1.6" strokeLinejoin="round" />
              </g>
              <circle className="gta-sa-radar-ring" cx={C} cy={C} r="73" fill="none" stroke="#000" strokeWidth="7" />
              <g transform="translate(80 8)">
                <circle r="8.5" fill="#000" />
                <text y="3.7" textAnchor="middle" fontSize="10.5" fontWeight="700" fill="#e1e1e1" fontFamily="Arimo, Arial, sans-serif">
                  N
                </text>
              </g>
            </svg>
            <span className="gta-sr">Radar: {openTickets(open)}, zum Board</span>
          </>
        )}
      </NavLink>
    </div>
  );
}
