// Pakete „Dracula“ und „Alucard“ gemeinsam: beide teilen Code (mermaid.ts, Progress.tsx) und unterscheiden sich nur in
// Palette und Farben; die Tests liegen deshalb nur hier und prüfen beide Ordner. Mermaid und Progress: Slots.test.tsx.
import { existsSync, mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterAll, beforeAll, describe, expect, it } from 'vitest';
import { build } from 'vite';
import { DRACULA } from '../src/palette.js';
import { ALUCARD } from '../../alucard/src/palette.js';

const themes = join(import.meta.dirname, '..', '..');
const read = (path: string) => readFileSync(join(themes, path), 'utf8');
const COLOR = /^(#|rgba?\()/;

const THEMES = [
  { id: 'dracula', name: 'Dracula', scheme: 'dark', palette: DRACULA, start: '#21222c' },
  { id: 'alucard', name: 'Alucard', scheme: 'light', palette: ALUCARD, start: '#f7f5fc' },
] as const;

// Variablen eines Blocks (vom Selektor bis zur ersten schließenden Klammer)
function variables(css: string, selector: string): Map<string, string> {
  const start = css.indexOf(selector);
  if (start < 0) return new Map();
  const body = css.slice(css.indexOf('{', start) + 1, css.indexOf('}', start));
  return new Map([...body.matchAll(/(--[\w-]+)\s*:\s*([^;]+);/g)].map((m) => [m[1], m[2].trim()]));
}

// Relative Leuchtdichte und Kontrast nach WCAG 2.x
function luminance(hex: string): number {
  const [r, g, b] = [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16) / 255).map((c) => (c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4));
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}
function contrast(a: string, b: string): number {
  const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x);
  return (hi + 0.05) / (lo + 0.05);
}

// Schrift auf Fläche, die mindestens 4,5:1 erreichen muss
const TEXT_PAIRS: [string, string][] = [
  ['--text', '--bg'],
  ['--text', '--surface'],
  ['--text-soft', '--surface'],
  ['--muted', '--bg'],
  ['--muted', '--surface'],
  ['--muted', '--sidebar-bg'],
  ['--on-accent', '--accent'],
  ['--on-primary', '--primary'],
  ['--chip-on-fg', '--chip-on-bg'],
  ['--code-fg', '--code-bg'],
  ['--code-muted', '--code-bg'],
  ['--status-fg', '--status-bg'],
  ['--running-fg', '--running-bg'],
  ['--waiting-fg', '--waiting-bg'],
  ['--idle-fg', '--idle-bg'],
  ['--blocked-fg', '--blocked-bg'],
  ['--orphan-fg', '--orphan-bg'],
  ['--unread-fg', '--unread-bg'],
  ['--note-fg', '--note-bg'],
  ['--research-fg', '--research-bg'],
  ['--connection-fg', '--surface'],
  ['--accent', '--surface'],
  ['--diff-add-fg', '--diff-add-bg'],
  ['--diff-del-fg', '--diff-del-bg'],
];

let tmp: string;

beforeAll(async () => {
  tmp = mkdtempSync(join(tmpdir(), 'theme-dracula-'));
  for (const { id } of THEMES) {
    await build({ configFile: join(themes, id, 'vite.config.ts'), logLevel: 'silent', build: { outDir: join(tmp, id) } });
  }
});

afterAll(() => rmSync(tmp, { recursive: true, force: true }));

describe.each(THEMES)('Paket „$name“', ({ id, name, scheme, palette, start }) => {
  const css = () => read(`${id}/theme.css`);
  const own = () => variables(css(), `:root[data-theme='${id}'] {`);

  it('hat ein Manifest mit Slot Progress, Vorschaubild und ohne Netz', () => {
    const manifest = JSON.parse(read(`${id}/manifest.json`));
    expect(manifest).toMatchObject({ id, name, colorScheme: scheme, startColor: start, sdkVersion: '^1.2', preview: 'assets/preview.png' });
    // Ohne Slot im Manifest lädt Nexus main.js nicht, und das Mermaid-Aussehen gälte nicht
    expect(manifest.slots).toEqual(['Progress']);
    expect(manifest.permissions.network).toEqual([]);
    expect(own().get('--bg')).toBe(start);
    expect(JSON.parse(read(`${id}/package.json`)).version).toBe(manifest.version);
  });

  it('baut main.js als ESM mit default-Export, React und SDK bleiben extern', () => {
    const main = readFileSync(join(tmp, id, 'main.js'), 'utf8');
    expect(main).toMatch(/export\s*\{[^}]*\bas default\b[^}]*\}|export default/);
    expect(main).toMatch(/from\s*["']react\/jsx-runtime["']/);
    expect(main).not.toContain('react.transitional.element');
  });

  it('legt manifest.json und theme.css neben main.js, ohne src/', () => {
    expect(JSON.parse(readFileSync(join(tmp, id, 'manifest.json'), 'utf8')).id).toBe(id);
    expect(readFileSync(join(tmp, id, 'theme.css'), 'utf8')).toBe(css());
    expect(existsSync(join(tmp, id, 'src'))).toBe(false);
  });

  it('gilt in theme.css nur für das eigene data-theme, ohne @import und fremde Adressen', () => {
    const plain = css().replace(/\/\*[\s\S]*?\*\//g, '');
    const selectors = plain
      .replace(/\{[^}]*\}/g, '{}')
      .split('{}')
      .flatMap((s) => s.split(','))
      .map((s) => s.trim())
      .filter(Boolean);
    expect(selectors.filter((selector) => !selector.startsWith(`:root[data-theme='${id}']`))).toEqual([]);
    expect(plain).not.toMatch(/@import|url\(/);
  });

  it('setzt jede Farbvariable des hellen Schemas und sonst nur die Palette --dracula-*', () => {
    const light = variables(read('../packages/web/src/styles/variables.css'), ':root {');
    const colors = [...light].filter(([, value]) => COLOR.test(value)).map(([n]) => n);
    expect(colors.filter((n) => !own().has(n))).toEqual([]);
    expect([...own().keys()].filter((n) => !light.has(n) && !n.startsWith('--dracula-'))).toEqual([]);
  });

  it('hält in theme.css dieselben Palettenwerte wie palette.ts', () => {
    const fromCss = Object.fromEntries(Object.keys(palette).map((key) => [key, own().get(`--dracula-${key}`)]));
    expect(fromCss).toEqual(palette);
  });

  it.each(TEXT_PAIRS)('ist lesbar: %s auf %s mit mindestens 4,5:1', (fg, bg) => {
    const vars = own();
    const ratio = contrast(vars.get(fg)!, vars.get(bg)!);
    expect(ratio, `${fg} ${vars.get(fg)} auf ${bg} ${vars.get(bg)}: ${ratio.toFixed(2)}`).toBeGreaterThanOrEqual(4.5);
  });
});

describe('Alucard: Board-Karten und Navigation', () => {
  const css = read('alucard/theme.css');
  const STATUS: [string, string][] = [
    ['waiting', 'waiting'], ['in_planning', 'in-planning'], ['planned', 'planned'], ['in_progress', 'in-progress'],
    ['blocked', 'blocked'], ['done', 'done'], ['not_needed', 'not-needed'],
  ];

  it('rahmt jede Karte in der Statusfarbe ihrer Spalte, die Spalten selbst haben keine Fläche', () => {
    expect(css).toContain(":root[data-theme='alucard'] .db-column .card { border: 1.5px solid var(--dracula-col); }");
    expect(css).toContain(":root[data-theme='alucard'] .db-column { --dracula-col: var(--status-backlog); background: transparent; }");
    for (const [status, name] of STATUS) {
      expect(css, status).toContain(`.db-column[data-status='${status}'] { --dracula-col: var(--status-${name}); }`);
    }
  });

  // Mischung wie color-mix in srgb: Kanal für Kanal anteilig
  const [, share, base] = /\.column-head::before \{[^}]*color-mix\(in srgb, var\(--dracula-col\) (\d+)%, (#[0-9a-f]{6})\)/.exec(css)!;
  const mix = (a: string) =>
    '#' + [1, 3, 5].map((i) => Math.round(parseInt(a.slice(i, i + 2), 16) * (Number(share) / 100) + parseInt(base.slice(i, i + 2), 16) * (1 - Number(share) / 100)).toString(16).padStart(2, '0')).join('');
  const vars = variables(css, ":root[data-theme='alucard'] {");
  const statuses = ['backlog', ...STATUS.map(([, name]) => name)];

  it.each(statuses)('Spaltenkopf in --status-%s bleibt lesbar (Text und gedämpfte Schrift ≥ 4,5:1)', (name) => {
    const tint = mix(vars.get(`--status-${name}`)!);
    for (const fg of ['--text', '--muted']) {
      expect(contrast(vars.get(fg)!, tint), `${fg} auf ${tint}`).toBeGreaterThanOrEqual(4.5);
    }
  });

  it('der klebende Spaltenkopf deckt die Karten dahinter mit dem Seitenhintergrund ab', () => {
    const rule = /\.db-column \.column-head \{([^}]*)\}/.exec(css)![1];
    expect(rule).toContain('top: -6px');
    expect(rule).toContain('margin: -6px -6px 0');
    expect(rule).toContain('background: var(--bg)');
  });

  it('unterstreicht den aktiven Navigationseintrag nur, ohne Fläche und Schatten', () => {
    const rule = /\.nav a\.active \{([^}]*)\}/.exec(css)![1];
    expect(rule).toContain('background: transparent');
    expect(rule).toContain('box-shadow: none');
    expect(rule).toContain('text-decoration: underline');
  });
});

describe('Gemeinsamer Code', () => {
  it('mermaid.ts und Progress.tsx sind in Dracula und Alucard gleich', () => {
    for (const file of ['src/mermaid.ts', 'src/Progress.tsx']) expect(read(`alucard/${file}`), file).toBe(read(`dracula/${file}`));
  });

  it('beide Paletten haben dieselben Schlüssel', () => {
    expect(Object.keys(ALUCARD).sort()).toEqual(Object.keys(DRACULA).sort());
  });
});
