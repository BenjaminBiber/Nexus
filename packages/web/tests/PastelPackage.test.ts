/// <reference types="node" />
import { existsSync, readdirSync, readFileSync, statSync } from 'node:fs';
import { join, relative } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

// Pfade relativ zum Repo
const repo = (path: string) => fileURLToPath(new URL(`../../../${path}`, import.meta.url));
const read = (path: string) => readFileSync(repo(path), 'utf8');
const PASTEL = /pastel/i;
const SCOPE = /^(:root)?\[data-theme='pastel'\]/;

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

// Oberste Blöcke einer CSS-Datei als Kopf (Selektor oder At-Regel) und Inhalt; Kommentare entfernt
function blocks(css: string): { head: string; body: string }[] {
  const text = css.replace(/\/\*[\s\S]*?\*\//g, '');
  const result: { head: string; body: string }[] = [];
  let depth = 0;
  let head = '';
  let start = 0;
  for (let i = 0; i < text.length; i++) {
    if (text[i] === '{') {
      if (depth === 0) {
        head = text.slice(start, i).trim();
        start = i + 1;
      }
      depth++;
    } else if (text[i] === '}') {
      depth--;
      if (depth === 0) {
        result.push({ head, body: text.slice(start, i) });
        start = i + 1;
      }
    }
  }
  expect(depth).toBe(0);
  return result;
}

// Einzelne Selektoren einer Selektorliste; Kommas in :is(), :not() usw. trennen nicht
function selectors(list: string): string[] {
  let flat = list;
  while (/\([^()]*\)/.test(flat)) flat = flat.replace(/\([^()]*\)/g, '');
  return flat.split(',').map((s) => s.trim()).filter(Boolean);
}

const THEME = 'themes/pastel';
const css = () => read(`${THEME}/theme.css`);

describe('Theme „Paper Notes“ (Schlüssel „pastel“) als Paket', () => {
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

  it('ersetzt Pastell unter derselben ID als reines CSS-Paket mit neuer Hauptversion', () => {
    const manifest = JSON.parse(read(`${THEME}/manifest.json`));
    // Gespeicherte Auswahl „pastel“ bleibt gültig
    expect(manifest.id).toBe('pastel');
    expect(manifest.name).toBe('Paper Notes');
    expect(manifest.version).toBe('2.0.0');
    expect(manifest.colorScheme).toBe('light');
    expect(manifest.startColor).toBe('#faf8f0');
    expect(manifest.slots ?? []).toEqual([]);
    expect(manifest.permissions?.network ?? []).toEqual([]);
    expect(existsSync(repo(`${THEME}/main.js`))).toBe(false);
  });

  it('enthält nur Dateien, die `nexus theme pack` annimmt', () => {
    const allowed = /\.(css|js|mjs|json|png|jpe?g|gif|webp|svg|avif|ico|woff2?|ttf|otf|mp3|ogg|wav)$/i;
    const all = files(repo(THEME)).map((path) => relative(repo(THEME), path).replaceAll('\\', '/'));
    expect(all.filter((path) => !allowed.test(path) || path.split('/').some((part) => part.startsWith('.')))).toEqual([]);
    expect(all.filter((path) => !['manifest.json', 'theme.css'].includes(path) && !path.startsWith('assets/'))).toEqual([]);
  });

  it('gilt in theme.css nur für data-theme=pastel und ändert nur Variablen, die das helle Schema kennt', () => {
    const text = css();
    expect(text).not.toMatch(/@import/i);
    const top = blocks(text);
    const rules = top.flatMap(({ head, body }) => {
      if (head.startsWith('@media')) return blocks(body).map((inner) => inner.head);
      if (head.startsWith('@')) return [];
      return [head];
    });
    expect(rules.length).toBeGreaterThan(5);
    expect(rules.flatMap(selectors).filter((selector) => !SCOPE.test(selector))).toEqual([]);
    // Außer @font-face und @media nur eigene Keyframes (Präfix paper-)
    const atRules = top.map(({ head }) => head).filter((head) => head.startsWith('@') && !/^@(font-face|media)\b/.test(head));
    expect(atRules.filter((head) => !/^@keyframes paper-[\w-]+$/.test(head))).toEqual([]);

    const light = variables(read('packages/web/src/styles/variables.css'), ':root {');
    const own = variables(text, ":root[data-theme='pastel'] {");
    expect(own.size).toBeGreaterThan(20);
    expect(own.get('--bg')).toBe('#faf8f0');
    expect(own.get('--font-sans')).toMatch(/^'Patrick Hand'/);
    // --paper-* gibt es nur im Paper-Notes-Stil
    expect([...own.keys()].filter((name) => !light.has(name) && !/^--paper-/.test(name))).toEqual([]);
  });

  it('bringt Patrick Hand und Caveat als woff2 mit relativer url() mit', () => {
    const faces = blocks(css()).filter(({ head }) => head === '@font-face');
    const families = faces.map(({ body }) => /font-family:\s*'([^']+)'/.exec(body)?.[1]);
    expect(new Set(families)).toEqual(new Set(['Patrick Hand', 'Caveat']));
    const sources = faces.map(({ body }) => /url\('([^']+)'\)/.exec(body)?.[1] ?? '');
    expect(sources.length).toBe(3);
    for (const source of sources) {
      expect(source).toMatch(/^assets\/fonts\/[\w-]+-latin-\d{3}-normal\.woff2$/);
      const bytes = readFileSync(repo(`${THEME}/${source}`));
      expect(bytes.subarray(0, 4).toString('latin1')).toBe('wOF2');
    }
    // Jede url() ist relativ (vorhandene Datei) oder data:, nie extern
    const urls = [...css().replace(/\/\*[\s\S]*?\*\//g, '').matchAll(/url\(\s*(["']?)(.*?)\1\s*\)/g)].map((m) => m[2]);
    expect(urls.filter((url) => !url.startsWith('data:') && !existsSync(repo(`${THEME}/${url}`)))).toEqual([]);
  });

  it('nutzt den Tintenfilter als data:-SVG statt einer Kern-Komponente und definiert jede eigene Animation', () => {
    const text = css();
    expect(text).toMatch(/--paper-ink-filter:\s*url\("data:image\/svg\+xml,[^"]*feDisplacementMap[^"]*#paper-ink"\)/);
    const defined = new Set([...text.matchAll(/@keyframes ([\w-]+)/g)].map((m) => m[1]));
    const used = new Set([...text.matchAll(/animation:\s*(paper-[\w-]+)/g)].map((m) => m[1]));
    expect([...used].filter((name) => !defined.has(name))).toEqual([]);
  });
});
