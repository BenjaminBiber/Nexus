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

beforeAll(async () => {
  tmp = mkdtempSync(join(tmpdir(), 'theme-win95-'));
  dist = join(tmp, 'dist');
  await build({ configFile: join(theme, 'vite.config.ts'), logLevel: 'silent', build: { outDir: dist } });
});

afterAll(() => rmSync(tmp, { recursive: true, force: true }));

describe('Windows-95-Theme-Paket', () => {
  it('baut main.js als ESM mit default-Export, React und SDK bleiben extern', () => {
    const main = readFileSync(join(dist, 'main.js'), 'utf8');
    expect(main).toMatch(/export\s*\{[^}]*\bas default\b[^}]*\}|export default/);
    expect(main).toMatch(/from\s*["']@nexus\/theme-sdk["']/);
    expect(main).toMatch(/from\s*["']react\/jsx-runtime["']/);
    expect(main).not.toContain('react.transitional.element');
  });

  it('legt manifest.json und theme.css neben main.js', () => {
    expect(JSON.parse(readFileSync(join(dist, 'manifest.json'), 'utf8')).id).toBe('win95');
    expect(readFileSync(join(dist, 'theme.css'), 'utf8')).toBe(readFileSync(join(theme, 'theme.css'), 'utf8'));
    expect(existsSync(join(dist, 'src'))).toBe(false);
  });

  it('ändert in theme.css nur Variablen, die das helle Schema kennt, dazu eigene --w95-*', () => {
    const css = readFileSync(join(theme, 'theme.css'), 'utf8');
    const light = variables(readFileSync(join(theme, '..', '..', 'packages', 'web', 'src', 'styles', 'variables.css'), 'utf8'), ':root {');
    const win95 = variables(css, ":root[data-theme='win95'] {");
    expect(win95.get('--w95-desktop')).toBe('#008080');
    expect([...win95.keys()].filter((name) => !light.has(name) && !name.startsWith('--w95-'))).toEqual([]);
  });
});
