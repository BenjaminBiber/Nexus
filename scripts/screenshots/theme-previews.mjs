// Erzeugt die Vorschaubilder der Themes (themes/<id>/assets/preview.png) mit erfundenen Demodaten (siehe demo.mjs):
// Board „Nova Shop“ je Theme, 1280×640 aufgenommen und auf 640×320 verkleinert (passt zu den Kacheln im Store
// und unter Einstellungen → Darstellung). Das Manifest nennt das Bild als `"preview": "assets/preview.png"`;
// `scripts/release/theme-index.mjs` legt es am Release neben das Paket und verlinkt es im Store-Index.
// Voraussetzung: gebaute Oberfläche, Debug-Binary und gepackte Themes
// (`npm run build`, `cargo build -p nexus-cli --bin nexus`, `npm run build:themes`).
// Playwright kommt nicht in die package.json:
//   npm i --no-save playwright
//   npx playwright install chromium
//   node scripts/screenshots/theme-previews.mjs [<id> …]   (ohne IDs: alle Pakete aus dist/themes)
import { existsSync, readdirSync, readFileSync, statSync } from 'node:fs';
import { join } from 'node:path';
import { chromium } from 'playwright';
import { packageManifest } from '../release/lib/ThemeIndex.mjs';
import { root, sleep, startDemo } from './demo.mjs';

const packagesDir = join(root, 'dist', 'themes');
const VIEWPORT = { width: 1280, height: 640 };
const SCALE = 0.5;
const MAX_BYTES = 300 * 1024;
const EXPECTED = 'Versandkosten nach Zielland berechnen';

if (!existsSync(packagesDir)) throw new Error('dist/themes fehlt – erst „npm run build:themes“ ausführen');
const wanted = process.argv.slice(2);
const packages = readdirSync(packagesDir)
  .filter((name) => name.endsWith('.nexus'))
  .map((name) => ({ path: join(packagesDir, name), manifest: packageManifest(readFileSync(join(packagesDir, name))) }))
  .filter(({ manifest }) => wanted.length === 0 || wanted.includes(manifest.id))
  // Themes mit fremden Ressourcen (Minecraft: Mojang-Texturen) bekommen kein Bild: mit den Texturen dürfte es nicht
  // ins Repo und ins Paket, ohne sie zeigt es nur die Rückfall-Ansicht mit Offline-Hinweis
  .filter(({ manifest }) => {
    const external = (manifest.permissions?.network ?? []).length > 0;
    if (external) console.log(`${manifest.id}: übersprungen (lädt fremde Ressourcen)`);
    return !external;
  });
if (packages.length === 0) throw new Error(`Keine passenden Pakete in ${packagesDir}`);

const { base, pid, stop } = await startDemo({
  beforeServe: (nexus) => {
    for (const { path } of packages) nexus('theme', 'install', path);
  },
});
try {
  // Ohne heruntergeladenes Chromium das installierte Chrome nehmen
  const browser = await chromium.launch().catch(() => chromium.launch({ channel: 'chrome' }));
  try {
    for (const { manifest } of packages) {
      const { id } = manifest;
      const target = join(root, 'themes', id, 'assets', 'preview.png');
      if (!existsSync(join(root, 'themes', id))) throw new Error(`themes/${id} fehlt`);

      // Gespeichertes Theme setzen; der Kern lädt main.js nur für das Theme im localStorage, daher dort auch
      const saved = await fetch(`${base}/api/user-settings`, {
        method: 'PUT',
        headers: { 'content-type': 'application/json', 'x-harness': '1' },
        body: JSON.stringify({ theme: id }),
      });
      if (!saved.ok) throw new Error(`${id}: Theme setzen schlug fehl: ${saved.status} ${await saved.text()}`);

      const context = await browser.newContext({
        viewport: VIEWPORT,
        deviceScaleFactor: SCALE,
        locale: 'de-DE',
        colorScheme: manifest.colorScheme === 'dark' ? 'dark' : 'light',
        reducedMotion: 'reduce',
      });
      // Windows 95 als Fenster auf dem Desktop statt maximiert, damit Desktop-Symbole und Taskleiste zu sehen sind
      await context.addInitScript((theme) => {
        localStorage.setItem('harness-theme-mode', theme);
        localStorage.setItem('harness-w95-maximized', 'false');
      }, id);
      // Fremde Ressourcen (Asset-Proxy, z. B. Mojang-Texturen für Minecraft) dürfen nicht ins Bild: Pakete und Repo
      // verteilen sie nie mit. Ohne Antwort zeigen Themes ihre Rückfall-Ansicht.
      await context.route('**/api/themes/*/fetch**', (route) => route.fulfill({ status: 502, body: 'gesperrt für Vorschaubilder' }));
      const page = await context.newPage();
      await page.goto(`${base}/p/${pid}/board`);
      await page.getByText(EXPECTED).first().waitFor({ timeout: 15000 });
      await page.waitForLoadState('networkidle');
      await page.evaluate(() => document.fonts.ready);
      await sleep(800);
      const shown = await page.evaluate(() => document.documentElement.dataset.theme);
      if (shown !== id) throw new Error(`${id}: angezeigt wird „${shown}“`);
      const text = await page.locator('body').innerText();
      if (/werden geladen|wird geladen|Lädt/.test(text)) throw new Error(`${id}: Ansicht lädt noch`);
      if (/Standard wird gezeigt/.test(text)) throw new Error(`${id}: Theme wurde nicht geladen`);

      await page.screenshot({ path: target });
      await context.close();
      const size = statSync(target).size;
      if (size > MAX_BYTES) throw new Error(`${id}: preview.png ist ${Math.round(size / 1024)} KB groß (Grenze ${MAX_BYTES / 1024} KB)`);
      console.log(`themes/${id}/assets/preview.png: ${Math.round(size / 1024)} KB`);
    }
  } finally {
    await browser.close();
  }
} finally {
  await stop();
}
