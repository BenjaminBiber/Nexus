import { existsSync, readFileSync, readdirSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';

const root = join(import.meta.dirname, '..');
const readmePath = join(root, 'README.md');
const readme = () => (existsSync(readmePath) ? readFileSync(readmePath, 'utf8') : '');
const src = (name: string) => readFileSync(join(root, 'src', name), 'utf8');

// Namen, die im README nicht als Code vorkommen (`Name`, auch als Aufruf `name()` oder generisch `Name<S>`)
const fehlend = (names: string[]) => {
  const text = readme();
  return names.filter((n) => !['`', '(', '<'].some((end) => text.includes('`' + n + end)));
};

/** Slot-Namen aus `ThemeSlots` samt Seiten-Slots `page:<name>` aus `PageName` */
function slotNames(): string[] {
  const slots = src('slots.ts');
  const block = /export interface ThemeSlots[^{]*\{([\s\S]*?)\n\}/.exec(slots)?.[1] ?? '';
  const names = [...block.matchAll(/^\s*(\w+)\?:/gm)].map((m) => m[1]);
  const pages = /export type PageName\s*=\s*([^;]+);/.exec(slots)?.[1] ?? '';
  return [...names, ...[...pages.matchAll(/'([^']+)'/g)].map((m) => `page:${m[1]}`)];
}

/** Alle Namen, die `src/index.ts` exportiert (Wert-Exporte und Typen der re-exportierten Dateien) */
function exportNames(): string[] {
  const index = src('index.ts');
  const values = [...index.matchAll(/export \{([^}]+)\}/g)].flatMap((m) => m[1].split(',').map((n) => n.trim()));
  const typeFiles = [...index.matchAll(/export type \* from '\.\/(\w+)\.js'/g)].map((m) => m[1]);
  const files = readdirSync(join(root, 'src'));
  const types = typeFiles.flatMap((base) => {
    const file = files.find((f) => f === `${base}.ts` || f === `${base}.tsx`)!;
    return [...src(file).matchAll(/^export (?:interface|type) (\w+)/gm)].map((m) => m[1]);
  });
  return [...values, ...types].filter(Boolean);
}

/** CSS-Variablen aus dem `:root { … }`-Block von variables.css */
function cssVariables(): string[] {
  const css = readFileSync(join(root, '..', 'web', 'src', 'styles', 'variables.css'), 'utf8');
  const block = /^:root \{([\s\S]*?)^\}/m.exec(css)?.[1] ?? '';
  return [...block.matchAll(/^\s*(--[\w-]+):/gm)].map((m) => m[1]);
}

describe('README des Theme-SDK', () => {
  it('existiert', () => {
    expect(existsSync(readmePath)).toBe(true);
  });

  it('nennt jeden Slot', () => {
    const names = slotNames();
    expect(names).toContain('Shell');
    expect(names).toContain('page:dashboard');
    expect(fehlend(names)).toEqual([]);
  });

  it('nennt jeden Export, insbesondere jeden Hook', () => {
    const names = exportNames();
    expect(names.filter((n) => n.startsWith('use')).length).toBeGreaterThan(5);
    expect(names).toContain('ThemeDefinition');
    expect(fehlend(names)).toEqual([]);
  });

  it('nennt jede CSS-Variable aus :root', () => {
    const names = cssVariables();
    expect(names.length).toBeGreaterThan(50);
    expect(fehlend(names)).toEqual([]);
  });

  it('beschreibt Manifest-Felder und Versionierung', () => {
    const fields = ['id', 'name', 'version', 'author', 'sdkVersion', 'colorScheme', 'startColor', 'preview', 'slots', 'permissions.network', 'settingsSchema', 'SDK_VERSION'];
    expect(fehlend(fields)).toEqual([]);
    expect(readme()).toMatch(/semver/i);
  });
});
