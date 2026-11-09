import type { NavIconName } from '@nexus/theme-sdk';

// Pixel-Symbole im Stil von Windows 95: 16×16 Raster, je Zeichen ein Pixel aus den 16 Systemfarben (Füllung in win95.css).
// k schwarz, w weiß, s silber, g grau, y gelb, o oliv, b blau, n marine, r rot, m kastanie, t petrol, c türkis, G grün, p violett
export const WIN95_ICONS = {
  folder: [
    '................',
    '................',
    '..kkkkk.........',
    '.kyyyyyk........',
    'kyyyyyyykkkkkk..',
    'kwwwwwwwwwwwwwk.',
    'kwyyyyyyyyyyyyok',
    'kwyyyyyyyyyyyyok',
    'kwyyyyyyyyyyyyok',
    'kwyyyyyyyyyyyyok',
    'kwyyyyyyyyyyyyok',
    'kwyyyyyyyyyyyyok',
    'kwyyyyyyyyyyyyok',
    'kooooooooooooook',
    'kkkkkkkkkkkkkkkk',
  ],
  inbox: [
    '................',
    '....kkkkkkkk....',
    '....kwwwwwwk....',
    '....kwnnnnwk....',
    '....kwwwwwwk....',
    '....kwnnnwwk....',
    '....kwwwwwwk....',
    '.kkkkwwwwwwkkkk.',
    'kwssskkkkkksssgk',
    'kwssssssssssssgk',
    'kwsskkkkkkkkssgk',
    'kwsswwwwwwwwssgk',
    'kwssssssssssssgk',
    'kggggggggggggggk',
    'kkkkkkkkkkkkkkkk',
  ],
  chart: [
    '................',
    '.kkkkkkkkkkkkkk.',
    '.kwwwwwwwwwwwwk.',
    '.kwwwwwwwwwrrwk.',
    '.kwwwwwwwwwrrwk.',
    '.kwwwwwwbbwrrwk.',
    '.kwwwwwwbbwrrwk.',
    '.kwwGGwwbbwrrwk.',
    '.kwwGGwwbbwrrwk.',
    '.kwwGGwwbbwrrwk.',
    '.kwkkkkkkkkkkwk.',
    '.kwwwwwwwwwwwwk.',
    '.kkkkkkkkkkkkkk.',
  ],
  prompt: [
    '................',
    'kkkkkkkkkkkkkkkk',
    'knnnnnnnnnnnnnnk',
    'kssssssssssssssk',
    'kskkkkkkkkkkkksk',
    'kskwkkkkkkkkkksk',
    'kskkwkkkkkkkkksk',
    'kskwkkwwwkkkkksk',
    'kskkkkkkkkkkkksk',
    'kskkkkkkkkkkkksk',
    'kskkkkkkkkkkkksk',
    'kssssssssssssssk',
    'kkkkkkkkkkkkkkkk',
  ],
  question: [
    '................',
    '...kkkkkkkkkk...',
    '..kwwwwwwwwwwk..',
    '.kwwwwbbbbwwwwk.',
    '.kwwwbbwwbbwwwk.',
    '.kwwwwwwwbbwwwk.',
    '.kwwwwwwbbwwwwk.',
    '.kwwwwwbbwwwwwk.',
    '.kwwwwwwwwwwwwk.',
    '.kwwwwwbbwwwwwk.',
    '..kwwwwwwwwwwk..',
    '...kkkwwkkkkk...',
    '.....kwk........',
    '....kk..........',
  ],
  document: [
    '..kkkkkkkkk.....',
    '..kwwwwwwwkk....',
    '..kwwwwwwwkwk...',
    '..kwnnnnwwkkkk..',
    '..kwwwwwwwwwwk..',
    '..kwnnnnnnnnwk..',
    '..kwwwwwwwwwwk..',
    '..kwnnnnnnnwwk..',
    '..kwwwwwwwwwwk..',
    '..kwnnnnnnnnwk..',
    '..kwwwwwwwwwwk..',
    '..kwnnnnnwwwwk..',
    '..kwwwwwwwwwwk..',
    '..kkkkkkkkkkkk..',
  ],
  help: [
    '................',
    '..kkkkkkkkkkk...',
    '..kbbbbbbbbbkk..',
    '..kbbbyyybbbkwk.',
    '..kbbyybyybbkwk.',
    '..kbbbbbyybbkwk.',
    '..kbbbbyybbbkwk.',
    '..kbbbbybbbbkwk.',
    '..kbbbbbbbbbkwk.',
    '..kbbbbybbbbkwk.',
    '..kbbbbbbbbbkwk.',
    '..knnnnnnnnnkwk.',
    '..kkkkkkkkkkkwk.',
    '...kwwwwwwwwwwk.',
    '....kkkkkkkkkkk.',
  ],
  computer: [
    '................',
    '.kkkkkkkkkkkkkk.',
    '.kwwwwwwwwwwwsk.',
    '.kwkkkkkkkkkksk.',
    '.kwkttttttttksk.',
    '.kwktyyttttrksk.',
    '.kwkttttttttksk.',
    '.kwkGGGGGGGGksk.',
    '.kwkkkkkkkkkksk.',
    '.kwsssssssssssk.',
    '.kggggggggggggk.',
    '.kkkkkkkkkkkkkk.',
    '.....kssssk.....',
    '...kkkkkkkkkk...',
    '...kssssssssgk..',
    '...kkkkkkkkkkk..',
  ],
  magnifier: [
    '................',
    '....kkkkk.......',
    '...kcccwwk......',
    '..kcccccwwk.....',
    '..kccccccwk.....',
    '..kccccccck.....',
    '..kccccccck.....',
    '...kccccckm.....',
    '....kkkkkkmm....',
    '..........kmm...',
    '...........kmm..',
    '............kmm.',
    '.............kk.',
  ],
} satisfies Record<string, string[]>;

export type Win95IconName = keyof typeof WIN95_ICONS;

// Pixel-Symbol je Navigationssymbol
export const WIN95_NAV_ICONS: Record<NavIconName, Win95IconName> = {
  board: 'folder',
  dashboard: 'chart',
  inbox: 'inbox',
  sessions: 'prompt',
  questions: 'question',
  docs: 'document',
  search: 'magnifier',
  guide: 'help',
  settings: 'computer',
};

// Waagrechte Läufe gleicher Farbe einer Zeile als Rechtecke (weniger Elemente als ein Rechteck je Pixel)
function runs(rows: string[]): Array<{ x: number; y: number; w: number; c: string }> {
  const out: Array<{ x: number; y: number; w: number; c: string }> = [];
  rows.forEach((row, y) => {
    let x = 0;
    while (x < row.length) {
      const c = row[x];
      let w = 1;
      while (row[x + w] === c) w++;
      if (c !== '.') out.push({ x, y, w, c });
      x += w;
    }
  });
  return out;
}

// Pixel-Symbol; Größe aus dem CSS (16 px, auf dem Desktop und im Startmenü 32 px)
export function Win95Icon({ name }: { name: Win95IconName }) {
  return (
    <svg className="nav-icon w95-pixel-icon" width="16" height="16" viewBox="0 0 16 16" shapeRendering="crispEdges" aria-hidden="true">
      {runs(WIN95_ICONS[name]).map(({ x, y, w, c }) => (
        <rect key={`${x}-${y}`} x={x} y={y} width={w} height={1} className={`w95-px-${c}`} />
      ))}
    </svg>
  );
}
