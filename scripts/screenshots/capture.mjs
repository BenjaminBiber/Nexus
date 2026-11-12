// Erzeugt die Screenshots der README (assets/screenshots/*.png) mit erfundenen Demodaten (siehe demo.mjs).
// Voraussetzung: gebaute Oberfläche und Debug-Binary (`npm run build`, `cargo build -p nexus-cli --bin nexus`).
// Playwright kommt nicht in die package.json:
//   npm i --no-save playwright
//   npx playwright install chromium
//   node scripts/screenshots/capture.mjs
// Anderes Binary: NEXUS_BIN=<pfad> node scripts/screenshots/capture.mjs
import { mkdirSync, statSync } from 'node:fs';
import { join } from 'node:path';
import { chromium } from 'playwright';
import { root, sleep, startDemo } from './demo.mjs';

const outDir = join(root, 'assets', 'screenshots');
const VIEWPORT = { width: 1440, height: 900 };
const MAX_BYTES = 400 * 1024;

mkdirSync(outDir, { recursive: true });
const { base, pid, stop } = await startDemo();
try {
  const browser = await chromium.launch();
  try {
    const context = await browser.newContext({ viewport: VIEWPORT, locale: 'de-DE', colorScheme: 'light', reducedMotion: 'reduce' });
    const page = await context.newPage();
    // Aufnahme: warten, bis nichts mehr lädt, dann prüfen, dass der erwartete Text da und kein Ladehinweis mehr zu sehen ist
    const shoot = async (name, path, expected, prepare) => {
      await page.goto(`${base}/p/${pid}/${path}`);
      await prepare?.();
      await page.getByText(expected).first().waitFor({ timeout: 15000 });
      await page.waitForLoadState('networkidle');
      await sleep(500);
      const text = await page.locator('body').innerText();
      if (/werden geladen|wird geladen|Lädt/.test(text)) throw new Error(`${name}: Ansicht lädt noch`);
      const target = join(outDir, name);
      await page.screenshot({ path: target });
      const size = statSync(target).size;
      if (size > MAX_BYTES) throw new Error(`${name} ist ${Math.round(size / 1024)} KB groß (Grenze ${MAX_BYTES / 1024} KB)`);
      console.log(`${name}: ${Math.round(size / 1024)} KB`);
    };
    await shoot('board.png', 'board', 'Versandkosten nach Zielland berechnen');
    // Ticket-Panel mit geöffnetem Tab „Plan“
    await shoot('ticket.png', 'board/t/NOVA-0005', 'Der Preis im Checkout richtet sich', () =>
      page.locator('button, [role=tab]').filter({ hasText: /^Plan/ }).first().click());
    await shoot('aufgaben.png', 'inbox', 'Soll der Versand in die Schweiz');
    await shoot('frage.png', 'questions/NOVA-F0001', 'Worum es geht');
  } finally {
    await browser.close();
  }
} finally {
  await stop();
}
