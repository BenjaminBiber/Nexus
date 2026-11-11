// Paket: Vite-Build nach dist/ mit main.js (ESM, export default), manifest.json, theme.css und den Schriften; React und SDK
// bleiben extern. Dazu der Aufbau von theme.css: gemeinsamer Teil, GTA V und San Andreas mit denselben Tokens.
import { existsSync, mkdtempSync, readdirSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterAll, beforeAll, describe, expect, it } from 'vitest';
import { build } from 'vite';
import theme from '../src/main.js';
import { GTA_SETTINGS } from '../src/settings.js';

const root = join(import.meta.dirname, '..');
let tmp: string;
let dist: string;

const ROOT = ":root[data-theme='gta']";
const V = ":root[data-theme='gta']:not([data-gta-style='sa'])";
const SA = ":root[data-theme='gta'][data-gta-style='sa']";
const MARKS = ['/* ==== Gemeinsam ==== */', '/* ==== GTA V ==== */', '/* ==== San Andreas ==== */'];

// Tokens, die jeder Stil setzt (spätere Pakete dürfen weitere ergänzen, aber immer in beiden Stilen)
const TOKENS = [
  '--gta-font-ui',
  '--gta-font-title',
  '--gta-font-hud',
  '--gta-font-script',
  '--gta-bg',
  '--gta-panel',
  '--gta-panel-2',
  '--gta-line',
  '--gta-text',
  '--gta-text-dim',
  '--gta-accent',
  '--gta-accent-ink',
  '--gta-ok',
  '--gta-warn',
  '--gta-danger',
  '--gta-money',
  '--gta-c-backlog',
  '--gta-c-waiting',
  '--gta-c-in_planning',
  '--gta-c-planned',
  '--gta-c-in_progress',
  '--gta-c-blocked',
  '--gta-c-done',
  '--gta-c-not_needed',
];

const css = () => readFileSync(join(root, 'theme.css'), 'utf8');
const stripComments = (text: string) => text.replace(/\/\*[\s\S]*?\*\//g, '');

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
  return parts.map((p) => p.trim());
}

// Alle Selektoren von Regeln (ohne @-Regeln und Keyframe-Schritte), aufgeteilt in ihre Teile
function selectorParts(text: string): string[] {
  const frame = /^(from|to|\d+(\.\d+)?%)$/;
  return [...stripComments(text).matchAll(/([^{}]+)\{/g)]
    .map((m) => m[1]!.trim())
    .filter((s) => !s.startsWith('@') && !topLevel(s).every((p) => frame.test(p)))
    .flatMap(topLevel);
}

// Abschnitt zwischen zwei Kommentarzeilen (ohne Ende: bis zum Dateiende)
function section(text: string, start: string, end?: string): string {
  const from = text.indexOf(start);
  return text.slice(from + start.length, end ? text.indexOf(end) : undefined);
}

const tokens = (text: string) => new Set([...stripComments(text).matchAll(/(--gta-[\w-]+)\s*:/g)].map((m) => m[1]!));

beforeAll(async () => {
  tmp = mkdtempSync(join(tmpdir(), 'theme-gta-'));
  dist = join(tmp, 'dist');
  await build({ configFile: join(root, 'vite.config.ts'), logLevel: 'silent', build: { outDir: dist } });
});

afterAll(() => rmSync(tmp, { recursive: true, force: true }));

describe('GTA-Theme-Paket', () => {
  it('baut main.js als ESM mit default-Export, React und SDK werden nicht mitgebündelt', () => {
    const main = readFileSync(join(dist, 'main.js'), 'utf8');
    expect(main).toMatch(/export\s*\{[^}]*\bas default\b[^}]*\}|export default/);
    expect(main).not.toContain('react.transitional.element');
    expect(main).not.toContain('useSyncExternalStore');
  });

  it('legt manifest.json, theme.css und die Schriften neben main.js', () => {
    expect(JSON.parse(readFileSync(join(dist, 'manifest.json'), 'utf8')).id).toBe('gta');
    expect(readFileSync(join(dist, 'theme.css'), 'utf8')).toBe(css());
    expect(readdirSync(join(dist, 'assets', 'fonts')).sort()).toEqual(readdirSync(join(root, 'assets', 'fonts')).sort());
    expect(existsSync(join(dist, 'src'))).toBe(false);
  });

  it('das Manifest beschreibt ein dunkles Theme mit Rahmen, ohne fremde Domains und noch ohne Vorschaubild', () => {
    const manifest = JSON.parse(readFileSync(join(root, 'manifest.json'), 'utf8'));
    expect(manifest).toMatchObject({
      id: 'gta',
      name: 'GTA',
      version: '1.0.0',
      author: 'Nexus',
      sdkVersion: '^1.2',
      colorScheme: 'dark',
      startColor: '#0b0d0f',
      permissions: { network: [] },
    });
    expect(manifest).not.toHaveProperty('preview');
  });

  it('Schema und Slots stehen gleich im Manifest und im Code', () => {
    const manifest = JSON.parse(readFileSync(join(root, 'manifest.json'), 'utf8'));
    expect(manifest.settingsSchema).toEqual(GTA_SETTINGS);
    expect(theme.settingsSchema).toEqual(GTA_SETTINGS);
    expect(theme.id).toBe('gta');
    expect(manifest.slots).toEqual(['Shell']);
    expect(Object.keys(theme.slots)).toEqual(manifest.slots);
  });

  it('jede Regel hängt an :root[data-theme=\'gta\'], kein @import', () => {
    const parts = selectorParts(css());
    expect(parts.length).toBeGreaterThan(0);
    expect(parts.filter((p) => !p.startsWith(ROOT))).toEqual([]);
    expect(stripComments(css())).not.toMatch(/@import/);
  });

  it('gliedert sich in Schriften, Gemeinsam, GTA V und San Andreas; die Stil-Abschnitte tragen ihr Präfix', () => {
    const text = css();
    const at = MARKS.map((mark) => text.indexOf(mark));
    expect(at.every((i) => i > 0)).toBe(true);
    expect([...at].sort((a, b) => a - b)).toEqual(at);
    expect(text.lastIndexOf('@font-face')).toBeLessThan(at[0]!);
    const v = selectorParts(section(text, MARKS[1]!, MARKS[2]));
    const sa = selectorParts(section(text, MARKS[2]!));
    expect(v.length).toBeGreaterThan(0);
    expect(sa.length).toBeGreaterThan(0);
    expect(v.filter((p) => !p.startsWith(V))).toEqual([]);
    expect(sa.filter((p) => !p.startsWith(SA))).toEqual([]);
  });

  it('GTA V und San Andreas definieren dieselben --gta-*-Tokens', () => {
    const text = css();
    const v = tokens(section(text, MARKS[1]!, MARKS[2]));
    const sa = tokens(section(text, MARKS[2]!));
    for (const token of TOKENS) expect(v.has(token), token).toBe(true);
    expect([...sa].sort()).toEqual([...v].sort());
  });

  it('beide Stile färben den Akzent für Grün und Orange um', () => {
    const text = stripComments(css());
    for (const prefix of [V, SA]) {
      for (const accent of ['gruen', 'orange']) {
        const selector = `${prefix}[data-gta-accent='${accent}']`;
        const at = text.indexOf(`${selector} {`);
        expect(at, selector).toBeGreaterThan(0);
        const body = text.slice(text.indexOf('{', at), text.indexOf('}', at));
        expect(body, selector).toMatch(/--gta-accent\s*:/);
        expect(body, selector).toMatch(/--gta-accent-ink\s*:/);
      }
    }
  });

  it('url() zeigt nur auf Schriften im Paket (oder data:), jede Schrift steckt in einem @font-face', () => {
    const text = stripComments(css());
    const urls = [...text.matchAll(/url\(\s*['"]?([^'")]+)['"]?\s*\)/g)].map((m) => m[1]!);
    for (const url of urls.filter((u) => !u.startsWith('data:'))) {
      expect(url).toMatch(/^assets\/fonts\/[\w-]+\.woff2$/);
      expect(existsSync(join(dist, url)), url).toBe(true);
    }
    const faces = text.match(/@font-face\s*\{[^}]*\}/g) ?? [];
    const fonts = readdirSync(join(root, 'assets', 'fonts'));
    expect(fonts).toHaveLength(12);
    for (const font of fonts) {
      const face = faces.filter((f) => f.includes(`url('assets/fonts/${font}')`));
      expect(face, font).toHaveLength(1);
      expect(face[0], font).toMatch(/font-display:\s*swap/);
      expect(face[0], font).toMatch(/unicode-range:\s*U\+0000-00FF/);
    }
  });

  it('der Kopfkommentar nennt jede Schriftfamilie, ihre Lizenz und jedes data-gta-*-Attribut', () => {
    const text = css();
    const head = text.match(/^\/\*[\s\S]*?\*\//)?.[0] ?? '';
    const families = new Set([...stripComments(text).matchAll(/@font-face\s*\{[^}]*font-family:\s*'([^']+)'/g)].map((m) => m[1]!));
    expect(families.size).toBe(8);
    for (const family of families) expect(head, family).toContain(family);
    expect(head).toContain('SIL Open Font License 1.1');
    // Yellowtail steht laut den Namensdaten der Schrift unter der Apache-Lizenz, nicht unter der OFL
    expect(head).toMatch(/Yellowtail[^\n]*\n?[^\n]*Apache License 2\.0/);
    expect(head).toContain('Google Fonts');
    const attributes = new Set([...text.matchAll(/data-gta-[a-z]+/g)].map((m) => m[0]));
    for (const attribute of ['data-gta-style', 'data-gta-accent', 'data-gta-radar']) attributes.add(attribute);
    for (const attribute of attributes) expect(head, attribute).toContain(attribute);
  });

  it('am Ende des gemeinsamen Abschnitts stehen Animationen und Übergänge bei reduzierter Bewegung still', () => {
    const shared = stripComments(section(css(), MARKS[0]!, MARKS[1]));
    const at = shared.lastIndexOf('@media (prefers-reduced-motion: reduce)');
    expect(at).toBeGreaterThan(0);
    const reduced = shared.slice(at);
    expect(reduced).toMatch(/animation:\s*none\s*!important/);
    expect(reduced).toMatch(/transition:\s*none\s*!important/);
    // Danach folgt im gemeinsamen Abschnitt keine Regel mehr
    expect(reduced.replace(/\s+/g, ' ').trim()).toMatch(/\}\s*\}$/);
    expect(reduced.split('@media').length).toBe(2);
  });

  it('@keyframes animieren nur transform, opacity, filter und clip-path', () => {
    const text = stripComments(css());
    for (const block of text.match(/@keyframes [\w-]+\s*\{(?:[^{}]*\{[^}]*\})*\s*\}/g) ?? []) {
      const props = [...block.matchAll(/([\w-]+)\s*:/g)].map((m) => m[1]!);
      expect(props.filter((p) => !['transform', 'opacity', 'filter', 'clip-path'].includes(p)), block).toEqual([]);
    }
  });

  it('vite.config.ts gleicht der Vorlage', () => {
    expect(readFileSync(join(root, 'vite.config.ts'), 'utf8')).toBe(
      readFileSync(join(root, '..', '_template', 'vite.config.ts'), 'utf8'),
    );
  });

  it('src/ importiert nur react, react/jsx-runtime, @nexus/theme-sdk und relative Pfade mit .js', () => {
    const allowed = new Set(['react', 'react/jsx-runtime', '@nexus/theme-sdk']);
    const files = readdirSync(join(root, 'src'), { recursive: true, encoding: 'utf8' }).filter((f) => /\.tsx?$/.test(f));
    expect(files.length).toBeGreaterThan(0);
    for (const file of files) {
      const source = readFileSync(join(root, 'src', file), 'utf8');
      const specifiers = [...source.matchAll(/(?:^|\n)\s*(?:import|export)\s[^'";]*?from\s*['"]([^'"]+)['"]|(?:^|\n)\s*import\s*['"]([^'"]+)['"]/g)].map(
        (m) => (m[1] ?? m[2])!,
      );
      for (const spec of specifiers) {
        const ok = allowed.has(spec) || (/^\.\.?\//.test(spec) && spec.endsWith('.js'));
        expect(ok, `${file}: ${spec}`).toBe(true);
      }
    }
  });
});
