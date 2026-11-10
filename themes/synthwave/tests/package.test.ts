// Paket: Vite-Build nach dist/ mit main.js (ESM, export default), manifest.json und theme.css; React und SDK bleiben extern
import { existsSync, mkdtempSync, readdirSync, readFileSync, rmSync, statSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterAll, beforeAll, describe, expect, it } from 'vitest';
import { build } from 'vite';

const theme = join(import.meta.dirname, '..');
let tmp: string;
let dist: string;

// Die 14 Grundwerte jedes Farbschemas (Handbuch)
const BASE = [
  '--synth-bg',
  '--synth-bg-deep',
  '--synth-surface',
  '--synth-fg',
  '--synth-muted',
  '--synth-line',
  '--synth-magenta',
  '--synth-cyan',
  '--synth-violet',
  '--synth-sun-top',
  '--synth-sun-bottom',
  '--synth-ok',
  '--synth-warn',
  '--synth-danger',
];

// Variablen eines Blocks (vom Selektor bis zur ersten schließenden Klammer)
function variables(css: string, selector: string): Map<string, string> {
  const start = css.indexOf(selector);
  if (start < 0) return new Map();
  const body = css.slice(css.indexOf('{', start) + 1, css.indexOf('}', start));
  return new Map([...body.matchAll(/(--[\w-]+)\s*:\s*([^;]+);/g)].map((m) => [m[1], m[2].trim()]));
}

// Teile einer Selektorliste: nur Kommas außerhalb von Klammern trennen
function topLevel(selector: string): string[] {
  const parts = [''];
  let depth = 0;
  for (const ch of selector) {
    if (ch === '(') depth++;
    if (ch === ')') depth--;
    if (ch === ',' && depth === 0) parts.push('');
    else parts[parts.length - 1] += ch;
  }
  return parts;
}

beforeAll(async () => {
  tmp = mkdtempSync(join(tmpdir(), 'theme-synthwave-'));
  dist = join(tmp, 'dist');
  await build({ configFile: join(theme, 'vite.config.ts'), logLevel: 'silent', build: { outDir: dist } });
});

afterAll(() => rmSync(tmp, { recursive: true, force: true }));

describe('Synthwave-Theme-Paket', () => {
  it('baut main.js als ESM mit default-Export, React und SDK werden nicht mitgebündelt', () => {
    const main = readFileSync(join(dist, 'main.js'), 'utf8');
    expect(main).toMatch(/export\s*\{[^}]*\bas default\b[^}]*\}|export default/);
    expect(main).not.toContain('react.transitional.element');
    expect(main).not.toContain('useSyncExternalStore');
  });

  it('legt manifest.json und theme.css neben main.js', () => {
    expect(JSON.parse(readFileSync(join(dist, 'manifest.json'), 'utf8')).id).toBe('synthwave');
    expect(readFileSync(join(dist, 'theme.css'), 'utf8')).toBe(readFileSync(join(theme, 'theme.css'), 'utf8'));
    expect(existsSync(join(dist, 'src'))).toBe(false);
  });

  it('ändert in theme.css nur Variablen, die das helle Schema kennt, dazu eigene --synth-*', () => {
    const css = readFileSync(join(theme, 'theme.css'), 'utf8');
    const light = variables(readFileSync(join(theme, '..', '..', 'packages', 'web', 'src', 'styles', 'variables.css'), 'utf8'), ':root {');
    const synth = variables(css, ":root[data-theme='synthwave'] {");
    expect(synth.get('--synth-bg')).toBe('#120a24');
    expect(synth.get('--bg')).toBe('var(--synth-bg)');
    expect([...synth.keys()].filter((name) => !light.has(name) && !name.startsWith('--synth-'))).toEqual([]);
  });

  it('setzt genau die 14 Grundwerte als feste Farben, alles andere ist abgeleitet', () => {
    const css = readFileSync(join(theme, 'theme.css'), 'utf8');
    const synth = variables(css, ":root[data-theme='synthwave'] {");
    const literal = [...synth].filter(([, value]) => /^#[0-9a-f]{3,8}$/i.test(value)).map(([name]) => name);
    expect(literal.sort()).toEqual([...BASE].sort());
  });

  it('gilt nur im Schema „Synthwave“: jede Regel hängt an :root[data-theme=\'synthwave\'], kein @import, url() nur auf Schriften im Paket', () => {
    const css = readFileSync(join(theme, 'theme.css'), 'utf8').replace(/\/\*[\s\S]*?\*\//g, '');
    const frame = /^(from|to|\d+%)$/;
    const selectors = [...css.matchAll(/([^{}]+)\{/g)]
      .map((m) => m[1]!.trim())
      .filter((s) => !s.startsWith('@') && !topLevel(s).every((p) => frame.test(p.trim())));
    const parts = selectors.flatMap((s) => topLevel(s).map((p) => p.trim()));
    expect(parts.length).toBeGreaterThan(0);
    expect(parts.filter((p) => !p.startsWith(":root[data-theme='synthwave']"))).toEqual([]);
    expect(css).not.toMatch(/@import/);
    // url() nur relativ und auf Dateien, die im gebauten Paket liegen
    const urls = [...css.matchAll(/url\(\s*['"]?([^'")]+)['"]?\s*\)/g)].map((m) => m[1]!);
    expect(urls).toHaveLength(5);
    for (const url of urls) {
      expect(url).toMatch(/^assets\/fonts\/[\w-]+\.woff2$/);
      expect(existsSync(join(dist, url)), url).toBe(true);
    }
  });

  it('die Schriften sind zusammen kleiner als 150 KB, der Kopfkommentar nennt Schriften und Lizenz', () => {
    const fonts = join(theme, 'assets', 'fonts');
    const total = readdirSync(fonts).reduce((sum, file) => sum + statSync(join(fonts, file)).size, 0);
    expect(total).toBeLessThan(150 * 1024);
    const head = readFileSync(join(theme, 'theme.css'), 'utf8').match(/^\/\*[\s\S]*?\*\//)?.[0] ?? '';
    for (const text of ['SIL Open Font License', 'Orbitron', 'Exo 2', 'Mr Dafoe']) expect(head, text).toContain(text);
  });

  it('Fließtext leuchtet nicht: keine Regel setzt text-shadow auf body', () => {
    const css = readFileSync(join(theme, 'theme.css'), 'utf8').replace(/\/\*[\s\S]*?\*\//g, '');
    const rules = [...css.matchAll(/([^{}]+)\{([^{}]*)\}/g)];
    const body = rules.filter(([, sel]) => topLevel(sel!).some((p) => /(^|\s)body(\s*$|[\s.:[])/.test(p.trim())));
    expect(body.filter(([, , decl]) => /text-shadow/.test(decl!)).map(([, sel]) => sel!.trim())).toEqual([]);
  });

  it('jedes Farbschema außer Outrun setzt genau die 14 Grundwerte', () => {
    const css = readFileSync(join(theme, 'theme.css'), 'utf8');
    for (const palette of ['miami', 'nightcall']) {
      const block = variables(css, `:root[data-theme='synthwave'][data-synth-palette='${palette}'] {`);
      expect([...block.keys()].sort(), palette).toEqual([...BASE].sort());
      expect([...block.values()].every((value) => /^#[0-9a-f]{6}$/i.test(value)), palette).toBe(true);
    }
  });

  it('die Glow-Stufen setzen --synth-glow, der Grundblock bietet Leuchtschatten', () => {
    const css = readFileSync(join(theme, 'theme.css'), 'utf8');
    expect(variables(css, ":root[data-theme='synthwave'][data-synth-glow='low'] {").get('--synth-glow')).toBe('0.45');
    expect(variables(css, ":root[data-theme='synthwave'][data-synth-glow='high'] {").get('--synth-glow')).toBe('1.7');
    const synth = variables(css, ":root[data-theme='synthwave'] {");
    expect(synth.get('--synth-glow')).toBe('1');
    expect(synth.get('--synth-glow-magenta')).toContain('var(--synth-glow)');
    expect(synth.get('--synth-glow-cyan')).toContain('var(--synth-glow)');
  });

  it('hat die Version 1.0.0 und das SDK 1.2', () => {
    const manifest = JSON.parse(readFileSync(join(theme, 'manifest.json'), 'utf8'));
    expect(manifest.version).toBe('1.0.0');
    expect(manifest.sdkVersion).toBe('^1.2');
    expect(manifest.colorScheme).toBe('dark');
  });

  it('nennt das Vorschaubild im Manifest, es liegt als PNG unter 300 KB im gebauten Paket', () => {
    const manifest = JSON.parse(readFileSync(join(theme, 'manifest.json'), 'utf8'));
    expect(manifest.preview).toBe('assets/preview.png');
    const preview = join(dist, manifest.preview);
    expect(existsSync(preview)).toBe(true);
    expect(readFileSync(preview).subarray(1, 4).toString()).toBe('PNG');
    expect(statSync(preview).size).toBeLessThan(300 * 1024);
  });

  it('der Kopfkommentar von theme.css beschreibt jedes data-synth-*-Attribut', () => {
    const css = readFileSync(join(theme, 'theme.css'), 'utf8');
    const head = css.match(/^\/\*[\s\S]*?\*\//)?.[0] ?? '';
    const attributes = new Set([...css.matchAll(/data-synth-[a-z]+/g)].map((m) => m[0]));
    for (const attribute of attributes) expect(head, attribute).toContain(attribute);
  });

  it('meldet im Manifest genau die Slots an, die main.tsx liefert', () => {
    const manifest = JSON.parse(readFileSync(join(theme, 'manifest.json'), 'utf8'));
    expect(manifest.slots).toEqual(['Shell', 'NavIcon', 'Icon', 'Progress', 'StartAnimation']);
  });

  it('der Laserbalken blitzt bei 100 % einmal auf und steht bei reduzierter Bewegung still', () => {
    const css = readFileSync(join(theme, 'theme.css'), 'utf8').replace(/\/\*[\s\S]*?\*\//g, '');
    expect(css).toMatch(/@keyframes synth-flash\b/);
    expect(css).toMatch(/\[data-complete\][^{]*\{[^}]*animation:[^;]*synth-flash[^;]*\b1\b/);
    const reduced = css.slice(css.indexOf('@media (prefers-reduced-motion: reduce)'));
    expect(reduced).toMatch(/synth-progress\[data-complete\][^{]*\{[^}]*animation: none/);
  });

  it('setzt für jeden Status des Boards eine eigene Neonfarbe über --synth-status', () => {
    const css = readFileSync(join(theme, 'theme.css'), 'utf8').replace(/\/\*[\s\S]*?\*\//g, '');
    for (const status of ['backlog', 'in_planning', 'planned', 'in_progress', 'blocked', 'done', 'not_needed']) {
      const rule = new RegExp(`\\[data-status='${status}'\\][^{]*\\{[^}]*--synth-status:`);
      expect(css, status).toMatch(rule);
    }
  });

  it('In Arbeit pulsiert, Blockiert flackert, beides steht bei reduzierter Bewegung still', () => {
    const css = readFileSync(join(theme, 'theme.css'), 'utf8').replace(/\/\*[\s\S]*?\*\//g, '');
    expect(css).toMatch(/@keyframes synth-pulse\b/);
    expect(css).toMatch(/@keyframes synth-flicker\b/);
    expect(css).toMatch(/\[data-status='in_progress'\][^{]*\{[^}]*animation:[^;]*synth-pulse/);
    expect(css).toMatch(/\[data-status='blocked'\][^{]*\{[^}]*animation:[^;]*synth-flicker/);
    const reduced = css.slice(css.indexOf('@media (prefers-reduced-motion: reduce)'));
    expect(reduced).toMatch(/synth-pulse|in_progress/);
    expect(reduced).toMatch(/\[data-status='in_progress'\][^{]*\{[^}]*animation: none/);
    expect(reduced).toMatch(/\[data-status='blocked'\][^{]*\{[^}]*animation: none/);
  });

  it('gestaltet Knöpfe, Felder und Dialoge, der Fokus hat einen Rahmen statt nur Leuchten', () => {
    const css = readFileSync(join(theme, 'theme.css'), 'utf8').replace(/\/\*[\s\S]*?\*\//g, '');
    const selectors = [...css.matchAll(/([^{}]+)\{/g)].map((m) => m[1]!.trim());
    for (const part of ['button', 'input', "[data-slot='Dialog']"]) {
      expect(selectors.some((s) => s.includes(part)), part).toBe(true);
    }
    const rules = [...css.matchAll(/([^{}]+)\{([^{}]*)\}/g)];
    expect(rules.some(([, sel, decl]) => sel!.includes(':focus-visible') && /(^|[\s;])outline\s*:/.test(decl!))).toBe(true);
  });

  it('Knöpfe haben den Laser-Sweep, der bei reduzierter Bewegung stillsteht; kein backdrop-filter', () => {
    const css = readFileSync(join(theme, 'theme.css'), 'utf8').replace(/\/\*[\s\S]*?\*\//g, '');
    expect(css).toMatch(/@keyframes synth-sweep\b/);
    const reduced = css.slice(css.indexOf('@media (prefers-reduced-motion: reduce)'));
    expect(reduced).toMatch(/\.btn[^{]*::after[^{]*\{[^}]*animation: none/);
    expect(css).not.toContain('backdrop-filter');
  });

  it('VHS: die Ebene fängt keine Klicks ab, Bildstörung und Flimmern stehen bei reduzierter Bewegung still', () => {
    const css = readFileSync(join(theme, 'theme.css'), 'utf8').replace(/\/\*[\s\S]*?\*\//g, '');
    expect(css).toMatch(/\[data-synth-vhs='on'\] \.synth-screen::after\s*\{[^}]*pointer-events: none/);
    expect(css).toMatch(/@keyframes synth-tracking\b/);
    const reduced = css.slice(css.indexOf('@media (prefers-reduced-motion: reduce)'));
    expect(reduced).toMatch(/synth-tracking[^{]*\{[^}]*animation: none/);
    expect(reduced).toMatch(/\.synth-screen::after[^{]*\{[^}]*animation: none/);
  });

  it('keine @keyframes-Regel animiert box-shadow, width, height, top oder left', () => {
    const css = readFileSync(join(theme, 'theme.css'), 'utf8').replace(/\/\*[\s\S]*?\*\//g, '');
    for (const block of css.match(/@keyframes [\w-]+\s*\{(?:[^{}]*\{[^}]*\})*\s*\}/g) ?? []) {
      expect(block).not.toMatch(/(?:^|[\s;{])(?:box-shadow|width|height|top|left)\s*:/);
    }
  });
});
