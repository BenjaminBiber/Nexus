/// <reference types="node" />
import { existsSync, readdirSync, readFileSync, statSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import { BUILTIN_THEME_IDS } from '../src/api/types.js';

// Pfade relativ zum Repo
const repo = (path: string) => fileURLToPath(new URL(`../../../${path}`, import.meta.url));
const read = (path: string) => readFileSync(repo(path), 'utf8');
const WIN95 = /win95|w95|windows 95/i;

function files(dir: string): string[] {
  return readdirSync(dir).flatMap((name) => {
    const path = join(dir, name);
    return statSync(path).isDirectory() ? files(path) : [path];
  });
}

describe('Kern kennt nur Hell und Dunkel', () => {
  it('enthält kein Windows 95 mehr', () => {
    const sources = [...files(repo('packages/web/src')), repo('packages/web/index.html')];
    const hits = sources.flatMap((path) =>
      readFileSync(path, 'utf8')
        .split('\n')
        .filter((line) => WIN95.test(line))
        .map((line) => `${path}: ${line.trim()}`),
    );
    expect(hits).toEqual([]);
    expect(existsSync(repo('packages/web/src/components/win95'))).toBe(false);
    expect(existsSync(repo('packages/web/src/themes/win95.tsx'))).toBe(false);
    expect(existsSync(repo('packages/web/src/styles/win95.css'))).toBe(false);
  });

  it('hat als eingebaute Themes nur system, light und dark', () => {
    expect([...BUILTIN_THEME_IDS]).toEqual(['system', 'light', 'dark']);
    expect(read('crates/nexus-core/src/settings/user_settings.rs')).toMatch(
      /BUILTIN_THEME_IDS: \[&str; 3\] = \["system", "light", "dark"\];/,
    );
  });

  it('liefert Windows 95 als Code-Paket unter themes/win95', () => {
    const manifest = JSON.parse(read('themes/win95/manifest.json'));
    expect(manifest.id).toBe('win95');
    expect(manifest.name).toBe('Windows 95');
    expect(manifest.colorScheme).toBe('light');
    expect(manifest.startColor).toBe('#c0c0c0');
    expect(manifest.slots).toEqual(['Shell', 'NavIcon', 'Progress']);
    expect(existsSync(repo('themes/win95/theme.css'))).toBe(true);
    expect(existsSync(repo('themes/win95/src/main.tsx'))).toBe(true);
  });
});
