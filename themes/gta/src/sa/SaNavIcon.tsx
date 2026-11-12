import type { ReactNode } from 'react';
import type { NavIconName } from '@nexus/theme-sdk';
import { NEUTRAL, SaGlyph } from './SaGlyph.js';

// Symbole der Menüliste im Look der HUD-Symbole von San Andreas: breite, flächige Formen mit Kontur
const SHAPES: Record<NavIconName, ReactNode> = {
  // drei breite Spalten
  board: (
    <>
      <rect x="2.5" y="3" width="5.5" height="18" rx="1" />
      <rect x="9.25" y="3" width="5.5" height="12" rx="1" />
      <rect x="16" y="3" width="5.5" height="15" rx="1" />
    </>
  ),
  // Tacho
  dashboard: <path fillRule="evenodd" d="M2 18a10 10 0 0 1 20 0zm9.2-1.2l5.3-7.3-3.4 8.1z" />,
  // Briefumschlag
  inbox: <path fillRule="evenodd" d="M2 5h20v14H2zm2.5 2.2v.4L12 13l7.5-5.4v-.4z" />,
  // Crew: drei Köpfe
  sessions: (
    <>
      <circle cx="12" cy="7" r="3.6" />
      <path d="M5 21c0-4.4 3.1-7.3 7-7.3s7 2.9 7 7.3z" />
      <circle cx="4.4" cy="9.6" r="2.4" />
      <circle cx="19.6" cy="9.6" r="2.4" />
    </>
  ),
  // Handy mit Antenne: Fragen kommen als Anruf
  questions: (
    <>
      <path fillRule="evenodd" d="M7 3.5h9a2 2 0 0 1 2 2V21a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V5.5a2 2 0 0 1 2-2zm1.2 2.8v5.4h6.6V6.3z" />
      <rect x="14.6" y="0.6" width="2.4" height="3.4" />
    </>
  ),
  // Notizbuch mit Zeilen
  docs: <path fillRule="evenodd" d="M5 2h13a1.5 1.5 0 0 1 1.5 1.5v17A1.5 1.5 0 0 1 18 22H5zm3.5 5v2.2h8V7zm0 4.4v2.2h8v-2.2z" />,
  // dicke Lupe
  search: <path fillRule="evenodd" d="M10 2a8 8 0 0 1 6.5 12.7l5.4 5.4-2.8 2.8-5.4-5.4A8 8 0 1 1 10 2zm0 3.4a4.6 4.6 0 1 0 0 9.2 4.6 4.6 0 0 0 0-9.2z" />,
  // Info-Kreis
  guide: <path fillRule="evenodd" d="M12 2a10 10 0 1 1 0 20 10 10 0 0 1 0-20zm-1.6 8.4v8h3.2v-8zM12 5.2a1.9 1.9 0 1 0 0 3.8 1.9 1.9 0 0 0 0-3.8z" />,
  // drei Schieberegler
  settings: (
    <>
      <path d="M3 5h18v2.4H3zm0 5.8h18v2.4H3zm0 5.8h18V19H3z" />
      <rect x="6" y="3" width="4" height="6.4" rx="1" />
      <rect x="14" y="8.8" width="4" height="6.4" rx="1" />
      <rect x="8" y="14.6" width="4" height="6.4" rx="1" />
    </>
  ),
};

/** Symbol der Navigation (Slot NavIcon); unbekannte Namen ergeben ein neutrales Zeichen */
export function SaNavIcon({ name }: { name: NavIconName }) {
  const known = Object.hasOwn(SHAPES, name);
  return <SaGlyph name={known ? name : 'neutral'}>{known ? SHAPES[name] : NEUTRAL}</SaGlyph>;
}
