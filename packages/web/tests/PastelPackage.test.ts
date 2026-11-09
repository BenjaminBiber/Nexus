/// <reference types="node" />
import { existsSync, readdirSync, readFileSync, statSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

// Pfade relativ zum Repo
const repo = (path: string) => fileURLToPath(new URL(`../../../${path}`, import.meta.url));
const read = (path: string) => readFileSync(repo(path), 'utf8');
const PASTEL = /pastel/i;

function files(dir: string): string[] {
  return readdirSync(dir).flatMap((name) => {
    const path = join(dir, name);
    return statSync(path).isDirectory() ? files(path) : [path];
  });
}

// Variablen eines Blocks (vom Selektor bis zur ersten schließenden Klammer)
function variables(css: string, selector: string): Map<string, string> {
  const start = css.indexOf(selector);
  if (start < 0) return new Map();
  const body = css.slice(css.indexOf('{', start) + 1, css.indexOf('}', start));
  return new Map([...body.matchAll(/(--[\w-]+)\s*:\s*([^;]+);/g)].map((m) => [m[1], m[2].trim()]));
}

describe('Theme „Pastell“ als Paket', () => {
  it('steckt nicht mehr im Kern', () => {
    const sources = [...files(repo('packages/web/src')), repo('packages/web/index.html')];
    const hits = sources.flatMap((path) =>
      readFileSync(path, 'utf8')
        .split('\n')
        .filter((line) => PASTEL.test(line) || (path.endsWith('.css') && /--(paper|torn)-/.test(line)))
        .map((line) => `${path}: ${line.trim()}`),
    );
    expect(hits).toEqual([]);
    expect(existsSync(repo('packages/web/src/styles/themes.css'))).toBe(false);
    expect(read('crates/nexus-core/src/settings/user_settings.rs')).not.toMatch(/BUILTIN_THEME_IDS[^;]*"pastel"/);
  });

  it('liegt als gültiges reines CSS-Paket unter themes/pastel', () => {
    const manifest = JSON.parse(read('themes/pastel/manifest.json'));
    expect(manifest.id).toBe('pastel');
    expect(manifest.name).toBe('Pastell');
    expect(manifest.colorScheme).toBe('light');
    expect(manifest.startColor).toBe('#f3eadf');
    expect(manifest.slots ?? []).toEqual([]);
    expect(existsSync(repo('themes/pastel/main.js'))).toBe(false);
  });

  it('gilt in theme.css nur für data-theme=pastel und ändert nur Variablen, die das helle Schema kennt', () => {
    const css = read('themes/pastel/theme.css');
    const rules = css.replace(/\/\*[\s\S]*?\*\//g, '').replace(/\{[^}]*\}/g, '{}').split('{}').map((s) => s.trim()).filter(Boolean);
    expect(rules.length).toBeGreaterThan(5);
    expect(rules.flatMap((s) => s.replace(/\([^)]*\)/g, '()').split(',').map((p) => p.trim())).filter((selector) => !/^(:root)?\[data-theme='pastel'\]/.test(selector))).toEqual([]);
    const light = variables(read('packages/web/src/styles/variables.css'), ':root {');
    const own = variables(css, ":root[data-theme='pastel'] {");
    expect(own.size).toBeGreaterThan(20);
    // --paper-* und --torn-* gibt es nur im Papier-Stil
    expect([...own.keys()].filter((name) => !light.has(name) && !/^--(paper|torn)-/.test(name))).toEqual([]);
  });
});
