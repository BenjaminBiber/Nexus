// Erzeugt App-Symbol, Favicon, Toast-Logos, Tray-Symbole und Installer-Bilder aus nexus-mark.svg
// und nexus-logo.svg.
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
  installerDeep: '#1e3a8a',
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
// Bilder des NSIS-Installers. NSIS liest nur BMP, sharp schreibt keins: rohe RGB-Zeilen von
// unten nach oben, je Zeile BGR und auf 4 Byte aufgefüllt, hinter einen 54-Byte-Kopf
async function bmp(svg, file) {
  const { data, info } = await sharp(Buffer.from(svg)).flatten().raw().toBuffer({ resolveWithObject: true });
  const { width: w, height: h } = info;
  const row = Math.ceil((w * 3) / 4) * 4;
  const out = Buffer.alloc(54 + row * h);
  out.write('BM', 0);
  out.writeUInt32LE(out.length, 2);
  out.writeUInt32LE(54, 10);
  out.writeUInt32LE(40, 14);
  out.writeInt32LE(w, 18);
  out.writeInt32LE(h, 22);
  out.writeUInt16LE(1, 26);
  out.writeUInt16LE(24, 28);
  out.writeUInt32LE(row * h, 34);
  for (let y = 0; y < h; y++) {
    for (let x = 0; x < w; x++) {
      const s = (y * w + x) * 3;
      const d = 54 + (h - 1 - y) * row + x * 3;
      out[d] = data[s + 2];
      out[d + 1] = data[s + 1];
      out[d + 2] = data[s];
    }
  }
  writeFileSync(join(root, file), out);
}

const logoSvg = readFileSync(join(here, 'nexus-logo.svg'), 'utf8');
const [, lw, lh] = logoSvg.match(/viewBox="0 0 (\d+) (\d+)"/).map(Number);
const logoPath = logoSvg.match(/ d="([^"]+)"/)[1];
const logo = (x, y, w, fill) =>
  `<path transform="translate(${x} ${y}) scale(${+(w / lw).toFixed(5)})" fill="${fill}" fill-rule="evenodd" d="${logoPath}"/>`;
const mark = (x, y, w, fill, opacity = 1) =>
  `<path transform="translate(${x} ${y}) scale(${+(w / mw).toFixed(5)})" fill="${fill}" fill-opacity="${opacity}" d="${markPath}"/>`;

// Seitenbild (164x314, Willkommens- und Abschlussseite): Verlauf im Kachelblau, Schriftzug oben,
// große Bildmarke angeschnitten als Wasserzeichen unten
await bmp(`<svg xmlns="http://www.w3.org/2000/svg" width="164" height="314" viewBox="0 0 164 314">
  <defs><linearGradient id="g" x2="0" y2="1"><stop offset="0" stop-color="${COLORS.tile}"/><stop offset="1" stop-color="${COLORS.installerDeep}"/></linearGradient></defs>
  <rect width="164" height="314" fill="url(#g)"/>
  ${mark(-40, 190, 260, COLORS.mark, 0.1)}
  ${logo(20, 44, 124, COLORS.mark)}
</svg>`, 'apps/desktop/src-tauri/windows/installer-sidebar.bmp');

// Kopfbild (150x57, Kopf der übrigen Seiten): weißer Grund wie der Seitenkopf, Schriftzug in Kachelblau
await bmp(`<svg xmlns="http://www.w3.org/2000/svg" width="150" height="57" viewBox="0 0 150 57">
  <rect width="150" height="57" fill="#ffffff"/>
  ${logo(14, (57 - (120 * lh) / lw) / 2, 120, COLORS.tile)}
</svg>`, 'apps/desktop/src-tauri/windows/installer-header.bmp');

console.log('Markenbilder erzeugt');
