import type { IconName, NavIconName } from '@nexus/theme-sdk';

// Neon-Symbole: nur Linien auf einem Raster 24×24, wie gebogene Neonröhren mit kleinen Lücken an den Stößen
const PATHS: Record<Exclude<NavIconName | IconName, 'empty'>, string[]> = {
  // drei Spalten
  board: ['M4 4.5v15', 'M10 4.5v10', 'M16 4.5v12.5', 'M20 4.5v6'],
  // Tacho mit Zeiger
  dashboard: ['M4 17a8 8 0 0 1 16 0', 'M12 17l4-5', 'M3 20h18'],
  // Ablage mit Pfeil
  inbox: ['M12 3v8', 'M8.5 8l3.5 3.5 3.5-3.5', 'M3 14h5l1.5 2.5h5L16 14h5', 'M3 15.5V20h18v-4.5'],
  // Kassette
  sessions: ['M3 6.5h18v11H3z', 'M7 15l1.5-2.5h7L17 15', 'M8.5 10h0.01', 'M15.5 10h0.01', 'M10.5 10h3'],
  // Fragezeichen im Kreis
  questions: ['M12 3a9 9 0 1 1-0.01 0', 'M9.5 9.5a2.5 2.5 0 1 1 3.5 2.3c-0.7 0.3-1 0.9-1 1.7', 'M12 17h0.01'],
  // Buch
  docs: ['M12 6.5C10 5 7 4.5 4 5v13c3-0.5 6 0 8 1.5', 'M12 6.5c2-1.5 5-2 8-1.5v13c-3-0.5-6 0-8 1.5', 'M12 8v10'],
  // Lupe
  search: ['M10.5 4a6.5 6.5 0 1 1-0.01 0', 'M15.5 15.5L20 20'],
  // Kompass
  guide: ['M12 3a9 9 0 1 1-0.01 0', 'M15.5 8.5l-2 5-5 2 2-5z'],
  // Schieberegler
  settings: ['M4 7h9', 'M17 7h3', 'M15 5v4', 'M4 17h3', 'M11 17h9', 'M9 15v4', 'M4 12h12', 'M19 12h1'],
  // Blatt
  doc: ['M6 3h8l4 4v14H6z', 'M9 11h6', 'M9 14.5h6', 'M9 18h4'],
  // Blatt mit Haken
  plan: ['M6 3h8l4 4v14H6z', 'M9 13l2 2 4-4.5'],
  // A–Z-Buch
  glossary: ['M5 4h13v16H5z', 'M8 13l2-6 2 6', 'M8.6 11h2.8', 'M13 8.5h3l-3 4.5h3'],
  // Blatt mit Waage
  adr: ['M6 3h8l4 4v14H6z', 'M12 9v8', 'M8.5 10.5h7', 'M8.5 10.5l-1 3h2z', 'M15.5 10.5l-1 3h2z', 'M10 17h4'],
  // Blatt mit Ecke
  file: ['M6 3h8l4 4v14H6z', 'M14 3v4h4'],
  // Ordner zu
  folder: ['M3 6h6l2 2h10v11H3z'],
  // Ordner offen
  folderOpen: ['M3 18V6h6l2 2h8v3', 'M3 18l3-7h16l-3 7z'],
  // Stift
  edit: ['M4 20l1-4.5L16 4.5l3.5 3.5L8.5 19z', 'M14 7l3.5 3.5'],
  // Sprechblase
  comment: ['M4 5h16v11h-9l-4.5 3.5V16H4z', 'M8 9.5h8', 'M8 12.5h5'],
};

const LINE = { fill: 'none', stroke: 'currentColor', strokeWidth: 1.6, strokeLinecap: 'round', strokeLinejoin: 'round' } as const;

// Leerzustand: Palme vor der gestreiften Sonne, darunter „NO SIGNAL“
function EmptyScreen() {
  return (
    <svg className="synth-icon synth-empty" viewBox="0 0 120 80" aria-hidden="true" focusable="false" {...LINE}>
      <defs>
        <linearGradient id="synth-empty-sun" x1="0" y1="0" x2="0" y2="1">
          <stop offset="0" className="synth-empty-sun-top" />
          <stop offset="1" className="synth-empty-sun-bottom" />
        </linearGradient>
      </defs>
      <path className="synth-empty-sun" d="M36 54a24 24 0 0 1 48 0z" fill="url(#synth-empty-sun)" stroke="none" />
      <path className="synth-empty-stripes" d="M37 44h46 M36 48.5h48 M36 52.5h48" />
      <path
        className="synth-empty-palm"
        d="M66 56c-1-9 0-17 3-24 M69 32c-6-3-12-1-15 3 M69 32c-4-6-10-7-14-5 M69 32c3-6 9-8 14-6 M69 32c6-1 11 2 13 7 M69 32c0 5-3 9-6 11"
      />
      <path className="synth-empty-ground" d="M20 56h80" />
      <text x="60" y="72" textAnchor="middle" stroke="none" fill="currentColor" fontFamily="Orbitron, sans-serif" fontSize="9" letterSpacing="2">
        NO SIGNAL
      </text>
    </svg>
  );
}

/** Symbol für Navigation und Kern (Slots NavIcon und Icon); unbekannte Namen ergeben nichts */
export function NeonIcon({ name }: { name: NavIconName | IconName }) {
  if (name === 'empty') return <EmptyScreen />;
  const paths = Object.hasOwn(PATHS, name) ? PATHS[name] : null;
  if (!paths) return null;
  return (
    <svg className="synth-icon" viewBox="0 0 24 24" aria-hidden="true" focusable="false" {...LINE}>
      {paths.map((d) => (
        <path key={d} d={d} />
      ))}
    </svg>
  );
}
