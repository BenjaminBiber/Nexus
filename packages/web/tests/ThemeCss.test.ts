/// <reference types="node" />
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import { THEME_CACHE_KEY } from '../src/theme/theme.js';

// Inhalt einer Stildatei
const styles = (name: string) => readFileSync(fileURLToPath(new URL(`../src/styles/${name}`, import.meta.url)), 'utf8');
const COLOR = /#[0-9a-fA-F]{3,8}\b|rgba?\(/;

// Variablen eines Blocks (vom Selektor bis zur ersten schließenden Klammer)
function variables(css: string, selector: string): Map<string, string> {
  const start = css.indexOf(selector);
  if (start < 0) return new Map();
  const body = css.slice(css.indexOf('{', start) + 1, css.indexOf('}', start));
  return new Map([...body.matchAll(/(--[\w-]+)\s*:\s*([^;]+);/g)].map((m) => [m[1], m[2].trim()]));
}

describe('Farbschema', () => {
  it('definiert jede Farbvariable auch für das dunkle Schema', () => {
    const css = styles('variables.css');
    const light = variables(css, ':root {');
    const dark = variables(css, ":root[data-theme='dark'] {");
    const colors = [...light].filter(([, value]) => COLOR.test(value)).map(([name]) => name);
    expect(colors.length).toBeGreaterThan(30);
    expect(colors.filter((name) => !dark.has(name))).toEqual([]);
  });

  it('ändert im Schema „Windows 95“ nur Variablen, die das helle Schema kennt', () => {
    const css = styles('variables.css');
    const light = variables(css, ':root {');
    const win95 = variables(css, ":root[data-theme='win95'] {");
    expect(win95.get('--w95-desktop')).toBe('#008080');
    // --w95-* gibt es nur im Windows-95-Rahmen
    expect([...win95.keys()].filter((name) => !light.has(name) && !name.startsWith('--w95-'))).toEqual([]);
  });

  it('ändert im Schema „Pastell“ nur Variablen, die das helle Schema kennt', () => {
    const css = styles('variables.css');
    const light = variables(css, ':root {');
    const own = variables(css, ":root[data-theme='pastel'] {");
    expect(own.size).toBeGreaterThan(20);
    // --paper-* gibt es nur im Notizbuch-Stil von „Pastell“ (themes.css)
    expect([...own.keys()].filter((name) => !light.has(name) && !name.startsWith('--paper-'))).toEqual([]);
  });

  it('verwendet außerhalb von variables.css keine fest codierten Farben', () => {
    for (const name of ['base.css', 'questions.css', 'dbboard.css', 'dashboard.css', 'inbox.css', 'sessions.css', 'themes.css', 'win95.css']) {
      expect(styles(name).split('\n').filter((line) => COLOR.test(line)), name).toEqual([]);
    }
  });
});

describe('Frühstart in index.html', () => {
  it('liest das Schema unter demselben Schlüssel wie theme.ts', () => {
    const html = readFileSync(fileURLToPath(new URL('../index.html', import.meta.url)), 'utf8');
    expect(html).toContain(`"${THEME_CACHE_KEY}"`);
  });
});

describe('Board-Karten', () => {
  it('gestaltet das Session-Badge der DB-Karte nur über vorhandene Variablen', () => {
    const css = styles('dbboard.css');
    const rule = css.match(/\.badge\.session\s*\{([^}]*)\}/);
    expect(rule, '.badge.session fehlt').toBeTruthy();
    const defined = variables(styles('variables.css'), ':root {');
    const used = [...rule![1].matchAll(/var\((--[\w-]+)/g)].map((m) => m[1]);
    expect(used.length).toBeGreaterThan(0);
    expect(used.filter((name) => !defined.has(name))).toEqual([]);
  });
});
