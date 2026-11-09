// Tests für scripts/build-themes.mjs: packt Theme-Ordner mit dem echten `nexus`-CLI.

import { spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, mkdtempSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { afterEach, beforeAll, describe, expect, it } from 'vitest';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const script = join(root, 'scripts', 'build-themes.mjs');
const nexusBin = join(root, 'target', 'debug', process.platform === 'win32' ? 'nexus.exe' : 'nexus');

let temp;

beforeAll(() => {
  const build = spawnSync('cargo', ['build', '-q', '-p', 'nexus-cli', '--bin', 'nexus'], {
    cwd: root,
    encoding: 'utf8',
  });
  expect(build.status, build.stderr).toBe(0);
}, 1_200_000);

afterEach(() => {
  if (temp) rmSync(temp, { recursive: true, force: true });
  temp = undefined;
});

function theme(src, folder, id = folder, css = ':root { --bg: #000; }') {
  const dir = join(src, folder);
  mkdirSync(dir, { recursive: true });
  writeFileSync(
    join(dir, 'manifest.json'),
    JSON.stringify({ id, name: id, version: '1.0.0', author: 'Ada', sdkVersion: '^1.0', colorScheme: 'dark' }),
  );
  writeFileSync(join(dir, 'theme.css'), css);
}

function run(src, out) {
  return spawnSync(process.execPath, [script, '--src', src, '--out', out], {
    cwd: root,
    encoding: 'utf8',
    env: { ...process.env, NEXUS_BIN: nexusBin },
  });
}

describe('build-themes', () => {
  it('packt jeden Theme-Ordner und überspringt Punkt-Ordner', () => {
    temp = mkdtempSync(join(tmpdir(), 'nexus-build-themes-'));
    const src = join(temp, 'themes');
    const out = join(temp, 'out');
    theme(src, 'alpha');
    theme(src, 'beta');
    theme(src, '.versteckt');
    const result = run(src, out);
    expect(result.status, result.stderr).toBe(0);
    expect(readdirSync(out).sort()).toEqual(['alpha.nexus', 'beta.nexus']);
  });

  it('bricht ab, wenn Ordnername und Manifest-ID abweichen', () => {
    temp = mkdtempSync(join(tmpdir(), 'nexus-build-themes-'));
    const src = join(temp, 'themes');
    theme(src, 'alpha', 'anders');
    const result = run(src, join(temp, 'out'));
    expect(result.status).toBe(1);
    expect(result.stderr).toContain('Manifest-ID');
  });

  it('bricht bei ungültigem Theme mit Exit 1 ab und legt kein Paket dafür an', () => {
    temp = mkdtempSync(join(tmpdir(), 'nexus-build-themes-'));
    const src = join(temp, 'themes');
    const out = join(temp, 'out');
    theme(src, 'kaputt', 'kaputt', '@import url(https://evil.example/x.css);');
    const result = run(src, out);
    expect(result.status).toBe(1);
    expect(existsSync(join(out, 'kaputt.nexus'))).toBe(false);
  });

  it('legt ohne Themes einen leeren Ausgabeordner an', () => {
    temp = mkdtempSync(join(tmpdir(), 'nexus-build-themes-'));
    const out = join(temp, 'out');
    const result = run(join(temp, 'fehlt'), out);
    expect(result.status).toBe(0);
    expect(readdirSync(out)).toEqual([]);
  });
});
