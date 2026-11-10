// Paket: Vite-Build nach dist/ mit main.js (ESM, export default), manifest.json und theme.css; React und SDK bleiben extern
import { existsSync, mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterAll, beforeAll, describe, expect, it } from 'vitest';
import { build } from 'vite';

const theme = join(import.meta.dirname, '..');
let tmp: string;
let dist: string;

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
  tmp = mkdtempSync(join(tmpdir(), 'theme-terminal-'));
  dist = join(tmp, 'dist');
  await build({ configFile: join(theme, 'vite.config.ts'), logLevel: 'silent', build: { outDir: dist } });
});

afterAll(() => rmSync(tmp, { recursive: true, force: true }));

describe('Terminal-Theme-Paket', () => {
  it('baut main.js als ESM mit default-Export, React und SDK bleiben extern', () => {
    const main = readFileSync(join(dist, 'main.js'), 'utf8');
    expect(main).toMatch(/export\s*\{[^}]*\bas default\b[^}]*\}|export default/);
    expect(main).toMatch(/from\s*["']@nexus\/theme-sdk["']/);
    expect(main).toMatch(/from\s*["']react\/jsx-runtime["']/);
    expect(main).not.toContain('react.transitional.element');
  });

  it('legt manifest.json und theme.css neben main.js', () => {
    expect(JSON.parse(readFileSync(join(dist, 'manifest.json'), 'utf8')).id).toBe('terminal');
    expect(readFileSync(join(dist, 'theme.css'), 'utf8')).toBe(readFileSync(join(theme, 'theme.css'), 'utf8'));
    expect(existsSync(join(dist, 'src'))).toBe(false);
  });

  it('ändert in theme.css nur Variablen, die das helle Schema kennt, dazu eigene --term-*; Schrift ist die Festbreitenschrift der App', () => {
    const css = readFileSync(join(theme, 'theme.css'), 'utf8');
    const light = variables(readFileSync(join(theme, '..', '..', 'packages', 'web', 'src', 'styles', 'variables.css'), 'utf8'), ':root {');
    const terminal = variables(css, ":root[data-theme='terminal'] {");
    expect(terminal.get('--term-bg')).toBe('#0d1117');
    expect(terminal.get('--bg')).toBe('var(--term-bg)');
    expect(terminal.get('--font-sans')).toBe('var(--font-mono)');
    expect([...terminal.keys()].filter((name) => !light.has(name) && !name.startsWith('--term-'))).toEqual([]);
  });

  it('jedes Farbschema der Einstellungen setzt alle Grundwerte, und nur diese', () => {
    const css = readFileSync(join(theme, 'theme.css'), 'utf8');
    const base = [...variables(css, ":root[data-theme='terminal'] {").keys()].filter((name) => /^--term-(bg|fg|muted|line|ansi-)/.test(name));
    expect(base.length).toBe(23);
    const manifest = JSON.parse(readFileSync(join(theme, 'manifest.json'), 'utf8'));
    const palettes = manifest.settingsSchema.palette.options.map((o: { value: string }) => o.value).filter((v: string) => v !== 'github');
    expect(palettes).toHaveLength(7);
    for (const palette of palettes) {
      const vars = variables(css, `:root[data-theme='terminal'][data-term-palette='${palette}'] {`);
      expect([...vars.keys()].sort(), palette).toEqual([...base].sort());
    }
  });

  it('gilt nur im Schema „Terminal“: jede Regel hängt an :root[data-theme=\'terminal\'], Schriften nur aus dem Paket', () => {
    const css = readFileSync(join(theme, 'theme.css'), 'utf8').replace(/\/\*[\s\S]*?\*\//g, '');
    // Selektoren vor „{“, ohne @-Regeln und Schlüsselbilder von @keyframes
    const frame = /^(from|to|\d+%)$/;
    const selectors = [...css.matchAll(/([^{}]+)\{/g)]
      .map((m) => m[1]!.trim())
      .filter((s) => !s.startsWith('@') && !topLevel(s).every((p) => frame.test(p.trim())));
    const parts = selectors.flatMap((s) => topLevel(s).map((p) => p.trim()));
    expect(parts.length).toBeGreaterThan(100);
    expect(parts.filter((p) => !p.startsWith(":root[data-theme='terminal']"))).toEqual([]);
    expect(css).not.toMatch(/@import/);
    // url() nur relativ und auf Dateien, die im gebauten Paket liegen
    const urls = [...css.matchAll(/url\(\s*['"]?([^'")]+)['"]?\s*\)/g)].map((m) => m[1]!);
    expect(urls).toHaveLength(5);
    for (const url of urls) {
      expect(url).toMatch(/^assets\/fonts\/[\w-]+\.woff2$/);
      expect(existsSync(join(dist, url)), url).toBe(true);
    }
  });

  it('hat die Version 2.0.0 und die Slots des SDK 1.2', () => {
    const manifest = JSON.parse(readFileSync(join(theme, 'manifest.json'), 'utf8'));
    expect(manifest.version).toBe('2.0.0');
    expect(manifest.slots).toEqual(['Shell', 'NavIcon', 'Icon', 'Progress', 'StartAnimation']);
  });
});
