// Erzeugt App-Symbol, Favicon, Toast-Logos und Tray-Symbole aus nexus-mark.svg.
// Farben hier anpassen, dann:
//   npm i --no-save sharp
//   node assets/brand/render.mjs
//   npx --prefix apps/desktop tauri icon apps/desktop/app-icon.svg -o apps/desktop/src-tauri/icons
import { readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import sharp from 'sharp';

const COLORS = {
  tile: '#1d4ed8',
  mark: '#ffffff',
  ring: '#ffffff',
  bereit: '#16a34a',
  frage: '#7c3aed',
  wartet: '#f59e0b',
  wartetGlyph: '#111827',
};

const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, '..', '..');

// Pfad und Maße der Bildmarke aus der Quelldatei, damit es nur eine Stelle gibt
const markSvg = readFileSync(join(here, 'nexus-mark.svg'), 'utf8');
const [, mw, mh] = markSvg.match(/viewBox="0 0 (\d+) (\d+)"/).map(Number);
const markPath = markSvg.match(/ d="([^"]+)"/)[1];

// Kachel im 1024er-Raster, Bildmarke auf 62,5 % der Breite zentriert; mit Plakette
// kleiner und nach links oben gerückt, damit das rechte Bein frei bleibt
function tile(badge = '') {
  const w = badge ? 580 : 640;
  const s = w / mw;
  const [cx, cy] = badge ? [452, 440] : [512, 512];
  const x = cx - w / 2;
  const y = cy - (mh * s) / 2;
  return `<svg xmlns="http://www.w3.org/2000/svg" width="1024" height="1024" viewBox="0 0 1024 1024">
  <rect width="1024" height="1024" rx="224" fill="${COLORS.tile}"/>
  <path transform="translate(${x} ${y}) scale(${+s.toFixed(5)})" fill="${COLORS.mark}" d="${markPath}"/>${badge}
</svg>
`;
}

// Zustandsplakette unten rechts wie bisher (Mittelpunkt 788/788, weißer Ring)
function badge(fill, glyph) {
  return `
  <g transform="translate(788 788)">
    <circle r="200" fill="${COLORS.ring}"/>
    <circle r="168" fill="${fill}"/>
    ${glyph}
  </g>`;
}

const BADGES = {
  bereit: badge(COLORS.bereit,
    `<path d="M-68 4l44 44l88-96" fill="none" stroke="#fff" stroke-width="36" stroke-linecap="round" stroke-linejoin="round"/>`),
  frage: badge(COLORS.frage,
    `<path d="M-46-38a46 46 0 1 1 62 43c-12 5-16 13-16 27v6" fill="none" stroke="#fff" stroke-width="34" stroke-linecap="round" stroke-linejoin="round"/>
    <circle cy="80" r="20" fill="#fff"/>`),
  wartet: badge(COLORS.wartet,
    `<path d="M0-78V16" stroke="${COLORS.wartetGlyph}" stroke-width="38" stroke-linecap="round"/>
    <circle cy="74" r="22" fill="${COLORS.wartetGlyph}"/>`),
};

const png = (svg, size, file) => sharp(Buffer.from(svg)).resize(size, size).png().toFile(join(root, file));

writeFileSync(join(root, 'apps/desktop/app-icon.svg'), tile());
writeFileSync(join(root, 'packages/web/public/favicon.svg'), tile());

const toast = 'crates/nexus-server/assets/toast';
await png(tile(), 256, `${toast}/logo-app.png`);
for (const [name, b] of Object.entries(BADGES)) {
  await png(tile(b), 256, `${toast}/logo-${name}.png`);
  await png(tile(b), 32, `apps/desktop/src-tauri/icons/tray/${name}.png`);
  await png(tile(b), 64, `apps/desktop/src-tauri/icons/tray/${name}@2x.png`);
}
console.log('Markenbilder erzeugt');
