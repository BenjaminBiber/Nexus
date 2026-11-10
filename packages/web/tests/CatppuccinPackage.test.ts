/// <reference types="node" />
import { existsSync, readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

// Pfade relativ zum Repo
const repo = (path: string) => fileURLToPath(new URL(`../../../${path}`, import.meta.url));
const read = (path: string) => readFileSync(repo(path), 'utf8');
const COLOR = /^(#|rgba?\()/;

// Variablen eines Blocks (vom Selektor bis zur ersten schließenden Klammer)
function variables(css: string, selector: string): Map<string, string> {
  const start = css.indexOf(selector);
  if (start < 0) return new Map();
  const body = css.slice(css.indexOf('{', start) + 1, css.indexOf('}', start));
  return new Map([...body.matchAll(/(--[\w-]+)\s*:\s*([^;]+);/g)].map((m) => [m[1], m[2].trim()]));
}

describe('Theme „Catppuccin“ als Paket', () => {
  it('liegt als gültiges reines CSS-Paket unter themes/catppuccin', () => {
    const manifest = JSON.parse(read('themes/catppuccin/manifest.json'));
    expect(manifest.id).toBe('catppuccin');
    expect(manifest.name).toBe('Catppuccin');
    expect(manifest.colorScheme).toBe('dark');
    expect(manifest.startColor).toBe('#11111b');
    expect(manifest.slots ?? []).toEqual([]);
    expect(existsSync(repo('themes/catppuccin/main.js'))).toBe(false);
  });

  it('gilt in theme.css nur für data-theme=catppuccin', () => {
    const css = read('themes/catppuccin/theme.css');
    const selectors = css
      .replace(/\/\*[\s\S]*?\*\//g, '')
      .replace(/\{[^}]*\}/g, '{}')
      .split('{}')
      .flatMap((s) => s.split(','))
      .map((s) => s.trim())
      .filter(Boolean);
    expect(selectors.filter((selector) => !selector.startsWith(":root[data-theme='catppuccin']"))).toEqual([]);
  });

  it('setzt jede Farbvariable des hellen Schemas, nur Variablen, die es kennt, aus der Mocha-Palette', () => {
    const light = variables(read('packages/web/src/styles/variables.css'), ':root {');
    const own = variables(read('themes/catppuccin/theme.css'), ":root[data-theme='catppuccin'] {");
    const colors = [...light].filter(([, value]) => COLOR.test(value)).map(([name]) => name);
    expect(colors.filter((name) => !own.has(name))).toEqual([]);
    expect([...own.keys()].filter((name) => !light.has(name))).toEqual([]);
    expect(own.get('--surface')).toBe('#1e1e2e');
    expect(own.get('--accent')).toBe('#cba6f7');
    expect(own.get('--logo-plate')).toBe('#ffffff');
  });
});
