import type { ReactNode } from 'react';
import type { IconName } from '@nexus/theme-sdk';
import { NEUTRAL, VGlyph } from './VGlyph.js';

const SHEET = 'M6 2.5h8l5 5v14H6z';

// Weitere Symbole des Kerns (Docs-Baum, Textauswahl) im selben Look wie die Navigation
const SHAPES: Record<Exclude<IconName, 'empty'>, ReactNode> = {
  // Blatt mit Zeilen
  doc: <path fillRule="evenodd" d={`${SHEET}m7.2 1.8v4h4zM8.5 12h8v1.6h-8zm0 3.6h8v1.6h-8z`} />,
  // Blatt mit Haken
  plan: <path fillRule="evenodd" d={`${SHEET}m3.2 11.3l1.2-1.2 1.9 1.9 3.9-3.9 1.2 1.2-5.1 5.1z`} />,
  // Buch mit A
  glossary: <path fillRule="evenodd" d="M5 3h12.5A1.5 1.5 0 0 1 19 4.5V21H6.5A1.5 1.5 0 0 1 5 19.5zm4.6 13.5h2l.6-1.8h1.6l.6 1.8h2L13.3 7h-1.6zm3.2-3.6l.6-2.1.6 2.1z" />,
  // Waage
  adr: (
    <>
      <path d="M11 3h2v16h4v2H7v-2h4z" />
      <path d="M4 7h16v1.8H4z" />
      <path d="M6 9.5l-3 6h6zM18 9.5l-3 6h6z" />
    </>
  ),
  // Blatt mit Eselsohr
  file: <path fillRule="evenodd" d={`${SHEET}m7.2 1.8v4h4z`} />,
  // Ordner zu
  folder: <path d="M3 5h6.5l2 2.5H21V19H3z" />,
  // Ordner offen
  folderOpen: (
    <>
      <path d="M3 5h6.5l2 2.5H19V10H7.5L4.6 19H3z" />
      <path d="M8.3 11.5H22L18.8 19H5.2z" />
    </>
  ),
  // Stift
  edit: <path d="M3 17.2V21h3.8L17.8 10l-3.8-3.8zM20.7 7.1a1 1 0 0 0 0-1.4l-2.4-2.4a1 1 0 0 0-1.4 0l-1.8 1.8 3.8 3.8z" />,
  // Sprechblase
  comment: <path fillRule="evenodd" d="M3 4h18v12H9.5L4 20.5V16H3zm4 4v1.6h10V8zm0 3.4V13h7v-1.6z" />,
};

// Leerzustand: eine Minimap ohne Blips, „KEIN SIGNAL“
function EmptyMap() {
  return (
    <svg className="gta-v-icon gta-v-empty" data-icon="empty" viewBox="0 0 120 80" fill="currentColor" aria-hidden="true" focusable="false">
      <rect x="2" y="2" width="116" height="56" fill="none" stroke="currentColor" strokeWidth="3" />
      <path d="M2 24h70l24 14h22M32 2v56M58 24v34" fill="none" stroke="currentColor" strokeWidth="2.5" opacity=".45" />
      <path d="M58 32l4 9-4-2-4 2z" />
      <text x="60" y="74" textAnchor="middle" fontSize="11" fontWeight="700" letterSpacing="2" fontFamily="'Barlow Condensed', 'Arial Narrow', sans-serif">
        KEIN SIGNAL
      </text>
    </svg>
  );
}

/** Weiteres Symbol des Kerns (Slot Icon); unbekannte Namen ergeben ein neutrales Zeichen */
export function VIcon({ name }: { name: IconName }) {
  if (name === 'empty') return <EmptyMap />;
  const known = Object.hasOwn(SHAPES, name);
  return <VGlyph name={known ? name : 'neutral'}>{known ? SHAPES[name as Exclude<IconName, 'empty'>] : NEUTRAL}</VGlyph>;
}
