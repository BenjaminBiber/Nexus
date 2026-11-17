import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
// @ts-expect-error – reines ESM-Skript ohne Typen
import { contentHash, writeContentHash } from '../scripts/content-hash.mjs';

// Legt ein kleines Plugin an; Version und Build-Felder der Übersicht wechseln wie bei echten Builds
function plugin(dir: string, version: string, rule = 'regel') {
  mkdirSync(join(dir, '.claude-plugin'), { recursive: true });
  mkdirSync(join(dir, 'customizations'), { recursive: true });
  mkdirSync(join(dir, 'rules'), { recursive: true });
  writeFileSync(join(dir, '.claude-plugin/plugin.json'), JSON.stringify({ name: 'nexus', version }));
  writeFileSync(
    join(dir, 'customizations/overview.json'),
    JSON.stringify({ version: 1, createdAt: new Date().toISOString(), harnessVersion: version, cloneDir: dir, customDir: dir, entries: [] }),
  );
  writeFileSync(join(dir, 'rules/docs.md'), rule);
}

describe('Inhalts-Hash des Plugins', () => {
  let root: string;
  beforeEach(() => { root = mkdtempSync(join(tmpdir(), 'nexus-hash-')); });
  afterEach(() => rmSync(root, { recursive: true, force: true }));

  it('bleibt gleich, wenn sich nur Build-Version und Build-Felder ändern', () => {
    plugin(join(root, 'a'), '0.3.1793607011');
    plugin(join(root, 'b'), '0.3.1793700000');
    expect(contentHash(join(root, 'a'))).toBe(contentHash(join(root, 'b')));
  });

  it('ändert sich, wenn sich eine Datei ändert', () => {
    plugin(join(root, 'a'), '0.3.1', 'regel');
    plugin(join(root, 'b'), '0.3.1', 'andere regel');
    expect(contentHash(join(root, 'a'))).not.toBe(contentHash(join(root, 'b')));
  });

  it('schreibt den Hash ins Plugin und zählt die Datei selbst nicht mit', () => {
    const dir = join(root, 'a');
    plugin(dir, '0.3.1');
    const written = writeContentHash(dir);
    expect(readFileSync(join(dir, 'content-hash'), 'utf8').trim()).toBe(written);
    expect(contentHash(dir)).toBe(written);
  });
});
