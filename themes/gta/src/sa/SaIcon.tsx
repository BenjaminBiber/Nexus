import type { ReactNode } from 'react';
import type { IconName } from '@nexus/theme-sdk';
import { NEUTRAL, SaGlyph } from './SaGlyph.js';

const PAGE = 'M5 2h9.5L20 7.5V22H5z';

// Weitere Symbole des Kerns (Docs-Baum, Textauswahl) im selben Look wie die Menüliste
const SHAPES: Record<Exclude<IconName, 'empty'>, ReactNode> = {
  // Blatt mit zwei Zeilen
  doc: <path fillRule="evenodd" d={`${PAGE}M8 12v2h9v-2zm0 4v2h9v-2z`} />,
  // Klemmbrett mit Haken
  plan: <path fillRule="evenodd" d="M8 2h8v2.5h3.5V22h-15V4.5H8zm-.6 11.2l1.6-1.6 2.2 2.2 4.6-4.6 1.6 1.6-6.2 6.2z" />,
  // Buch mit Lesezeichen
  glossary: <path fillRule="evenodd" d="M4 3h13.5A2.5 2.5 0 0 1 20 5.5V22H6.5A2.5 2.5 0 0 1 4 19.5zm3 13.5v2h10v-2zM13 3v7l2-1.5 2 1.5V3z" />,
  // Richterhammer
  adr: (
    <>
      <path d="M13.6 1.6l6 6-2.4 2.4-6-6zM8.8 6.4l6 6-2.4 2.4-6-6z" />
      <path d="M11.2 9.2l2.4 2.4-8.4 8.4a1.7 1.7 0 0 1-2.4-2.4z" />
      <path d="M12 19.5h10V22H12z" />
    </>
  ),
  // Blatt mit Eselsohr
  file: <path fillRule="evenodd" d={`${PAGE}m8.5 1.5v5h5z`} />,
  // Ordner zu
  folder: <path d="M2 4.5h7.5l2.3 2.5H22V20H2z" />,
  // Ordner offen
  folderOpen: (
    <>
      <path d="M2 4.5h7.5l2.3 2.5H20v2.5H6.5L3.2 20H2z" />
      <path d="M7.6 11H23l-3.4 9H4.4z" />
    </>
  ),
  // dicker Stift
  edit: <path d="M2.5 17.5v4h4L18 10l-4-4zM15.5 4.5l2-2a1.4 1.4 0 0 1 2 0l2 2a1.4 1.4 0 0 1 0 2l-2 2z" />,
  // runde Sprechblase
  comment: <path d="M12 3c5.5 0 10 3.6 10 8s-4.5 8-10 8c-1 0-2-.1-2.9-.3L4 21.5l1.4-4.3C3.3 15.7 2 13.5 2 11c0-4.4 4.5-8 10-8z" />,
};

// Leerzustand: ein rundes Radar ohne Blips, „KEIN SIGNAL“
function EmptyRadar() {
  return (
    <svg className="gta-sa-icon gta-sa-empty" data-icon="empty" viewBox="0 0 120 96" fill="currentColor" aria-hidden="true" focusable="false">
      <circle cx="60" cy="38" r="33" fill="none" stroke="currentColor" strokeWidth="5" />
      <path d="M30 30h60M30 50h60M48 8v62M76 8v62" fill="none" stroke="currentColor" strokeWidth="2.5" opacity=".4" />
      <path d="M60 30l5 11-5-3-5 3z" />
      <text x="60" y="92" textAnchor="middle" fontSize="11" letterSpacing="1.5" fontFamily="Michroma, Arimo, sans-serif">
        KEIN SIGNAL
      </text>
    </svg>
  );
}

/** Weiteres Symbol des Kerns (Slot Icon); unbekannte Namen ergeben ein neutrales Zeichen */
export function SaIcon({ name }: { name: IconName }) {
  if (name === 'empty') return <EmptyRadar />;
  const known = Object.hasOwn(SHAPES, name);
  return <SaGlyph name={known ? name : 'neutral'}>{known ? SHAPES[name as Exclude<IconName, 'empty'>] : NEUTRAL}</SaGlyph>;
}
