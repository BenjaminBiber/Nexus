// Paket: Vite-Build nach dist/ mit main.js (ESM, export default), manifest.json und theme.css; React und SDK bleiben extern
import { existsSync, mkdtempSync, readFileSync, rmSync } from 'node:fs';
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

  it('gilt nur im Schema „Synthwave“: jede Regel hängt an :root[data-theme=\'synthwave\'], kein @import, kein url()', () => {
    const css = readFileSync(join(theme, 'theme.css'), 'utf8').replace(/\/\*[\s\S]*?\*\//g, '');
    const frame = /^(from|to|\d+%)$/;
    const selectors = [...css.matchAll(/([^{}]+)\{/g)]
      .map((m) => m[1]!.trim())
      .filter((s) => !s.startsWith('@') && !topLevel(s).every((p) => frame.test(p.trim())));
    const parts = selectors.flatMap((s) => topLevel(s).map((p) => p.trim()));
    expect(parts.length).toBeGreaterThan(0);
    expect(parts.filter((p) => !p.startsWith(":root[data-theme='synthwave']"))).toEqual([]);
    expect(css).not.toMatch(/@import/);
    expect(css).not.toMatch(/url\(/);
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
});
