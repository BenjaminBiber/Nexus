import type { ReactNode } from 'react';
import type { NavIconName } from '@nexus/theme-sdk';
import { NEUTRAL, VGlyph } from './VGlyph.js';

// Symbole der Navigation wie im Rad der Preview: kräftige, gefüllte Formen
const SHAPES: Record<NavIconName, ReactNode> = {
  // drei Spalten
  board: (
    <>
      <rect x="3" y="4" width="5" height="16" />
      <rect x="9.5" y="4" width="5" height="10" />
      <rect x="16" y="4" width="5" height="13" />
    </>
  ),
  // Balkendiagramm
  dashboard: (
    <>
      <path d="M3 20h18v-2H3z" />
      <rect x="5" y="11" width="3.5" height="6" />
      <rect x="10.25" y="5" width="3.5" height="12" />
      <rect x="15.5" y="8.5" width="3.5" height="8.5" />
    </>
  ),
  // Aktentasche
  inbox: <path fillRule="evenodd" d="M9 4h6a2 2 0 0 1 2 2v1h4v5H3V7h4V6a2 2 0 0 1 2-2zm0 2v1h6V6zM3 13.5h7V15h4v-1.5h7V20H3z" />,
  // zwei Spieler
  sessions: (
    <>
      <circle cx="8.5" cy="8" r="3.3" />
      <path d="M2.5 19.5c0-3.7 2.7-6.2 6-6.2s6 2.5 6 6.2z" />
      <circle cx="16.8" cy="9" r="2.7" />
      <path d="M15.4 13.4c3.3-.5 6.1 1.9 6.1 6.1h-5.2c0-2.3-.3-4.3-.9-6.1z" />
    </>
  ),
  // Fragezeichen
  questions: (
    <>
      <path d="M12 2.5c3.3 0 5.8 2.1 5.8 5.1 0 2.3-1.3 3.5-2.6 4.5-1 .8-1.5 1.3-1.5 2.6h-3.6c0-2.4.9-3.5 2.2-4.5 1-.8 1.7-1.3 1.7-2.4 0-1-.8-1.7-2-1.7-1.3 0-2.2.8-2.3 2.2H6.1c.2-3.5 2.6-5.8 5.9-5.8z" />
      <circle cx="11.9" cy="19.2" r="2.3" />
    </>
  ),
  // Blatt mit Zeilen
  docs: <path fillRule="evenodd" d="M6 2.5h8l5 5v14H6zm7.2 1.8v4h4zM8.5 12h8v1.6h-8zm0 3.6h8v1.6h-8z" />,
  // Lupe
  search: <path fillRule="evenodd" d="M10.5 3a7.5 7.5 0 0 1 6.1 11.9l4.5 4.5-1.9 1.9-4.5-4.5A7.5 7.5 0 1 1 10.5 3zm0 2.6a4.9 4.9 0 1 0 0 9.8 4.9 4.9 0 0 0 0-9.8z" />,
  // Info-Zeichen
  guide: (
    <>
      <circle cx="12" cy="5.2" r="2.4" />
      <path d="M8.5 9.5h5.7V18h2.3v2.5h-8V18h2.2v-6H8.5z" />
    </>
  ),
  // Schraubenschlüssel
  settings: <path d="M20.7 6.4l-3 3-2.6-.5-.5-2.6 3-3a5.2 5.2 0 0 0-6.6 6.6L3.3 17.6a2.2 2.2 0 1 0 3.1 3.1l7.7-7.7a5.2 5.2 0 0 0 6.6-6.6z" />,
};

/** Symbol der Navigation (Slot NavIcon); unbekannte Namen ergeben ein neutrales Zeichen */
export function VNavIcon({ name }: { name: NavIconName }) {
  const known = Object.hasOwn(SHAPES, name);
  return <VGlyph name={known ? name : 'neutral'}>{known ? SHAPES[name] : NEUTRAL}</VGlyph>;
}
