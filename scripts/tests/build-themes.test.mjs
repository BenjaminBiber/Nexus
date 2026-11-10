// Tests für scripts/build-themes.mjs: packt Theme-Ordner mit dem echten `nexus`-CLI.

import { spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
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

// Code-Theme mit eigenem Build: `npm run build` schreibt dist/ mit main.js, manifest.json und theme.css
function codeTheme(src, folder, buildScript) {
  theme(src, folder);
  const dir = join(src, folder);
  mkdirSync(join(dir, 'src'), { recursive: true });
  writeFileSync(join(dir, 'src', 'main.js'), 'export default { id: "quelle", slots: {} };');
  writeFileSync(join(dir, 'package.json'), JSON.stringify({ name: folder, private: true, scripts: { build: 'node build.mjs' } }));
  writeFileSync(
    join(dir, 'build.mjs'),
    buildScript ??
      `import { cpSync, mkdirSync, writeFileSync } from 'node:fs';
mkdirSync('dist', { recursive: true });
cpSync('manifest.json', 'dist/manifest.json');
cpSync('theme.css', 'dist/theme.css');
writeFileSync('dist/main.js', 'export default { id: "${folder}", slots: {} };');`,
  );
}

// Dateinamen aus dem zentralen Verzeichnis eines ZIP-Archivs
function zipEntries(bytes) {
  let eocd = bytes.length - 22;
  while (eocd >= 0 && bytes.readUInt32LE(eocd) !== 0x06054b50) eocd--;
  const count = bytes.readUInt16LE(eocd + 10);
  let at = bytes.readUInt32LE(eocd + 16);
  const names = [];
  for (let i = 0; i < count; i++) {
    const nameLength = bytes.readUInt16LE(at + 28);
    names.push(bytes.toString('utf8', at + 46, at + 46 + nameLength));
    at += 46 + nameLength + bytes.readUInt16LE(at + 30) + bytes.readUInt16LE(at + 32);
  }
  return names.filter((name) => !name.endsWith('/')).sort();
}

function run(src, out) {
  return spawnSync(process.execPath, [script, '--src', src, '--out', out], {
    cwd: root,
    encoding: 'utf8',
    env: { ...process.env, NEXUS_BIN: nexusBin },
  });
}

describe('build-themes', () => {
  it('packt jeden Theme-Ordner und überspringt Punkt- und Unterstrich-Ordner', () => {
    temp = mkdtempSync(join(tmpdir(), 'nexus-build-themes-'));
    const src = join(temp, 'themes');
    const out = join(temp, 'out');
    theme(src, 'alpha');
    theme(src, 'beta');
    theme(src, '.versteckt');
    theme(src, '_template');
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

  it('baut ein Code-Theme mit Skript build zuerst und packt dann dist/ statt des Quellordners', () => {
    temp = mkdtempSync(join(tmpdir(), 'nexus-build-themes-'));
    const src = join(temp, 'themes');
    const out = join(temp, 'out');
    codeTheme(src, 'code');
    theme(src, 'nurcss');
    const result = run(src, out);
    expect(result.status, result.stderr).toBe(0);
    expect(readdirSync(out).sort()).toEqual(['code.nexus', 'nurcss.nexus']);
    expect(zipEntries(readFileSync(join(out, 'code.nexus')))).toEqual(['main.js', 'manifest.json', 'theme.css']);
    expect(zipEntries(readFileSync(join(out, 'nurcss.nexus')))).toEqual(['manifest.json', 'theme.css']);
  });

  it('bricht ab, wenn der Build eines Code-Themes fehlschlägt', () => {
    temp = mkdtempSync(join(tmpdir(), 'nexus-build-themes-'));
    const src = join(temp, 'themes');
    const out = join(temp, 'out');
    codeTheme(src, 'kaputt', 'process.exit(3);');
    const result = run(src, out);
    expect(result.status).toBe(1);
    expect(result.stderr).toContain('Theme kaputt konnte nicht gebaut werden');
    expect(existsSync(join(out, 'kaputt.nexus'))).toBe(false);
  });

  it('packt die echten Theme-Ordner einschließlich des Code-Themes win95', () => {
    temp = mkdtempSync(join(tmpdir(), 'nexus-build-themes-'));
    const out = join(temp, 'out');
    const result = spawnSync(process.execPath, [script, '--out', out], {
      cwd: root,
      encoding: 'utf8',
      env: { ...process.env, NEXUS_BIN: nexusBin },
    });
    expect(result.status, result.stderr).toBe(0);
    expect(readdirSync(out)).toContain('win95.nexus');
    expect(zipEntries(readFileSync(join(out, 'win95.nexus')))).toEqual(['assets/preview.png', 'main.js', 'manifest.json', 'theme.css']);
  });

  it('legt ohne Themes einen leeren Ausgabeordner an', () => {
    temp = mkdtempSync(join(tmpdir(), 'nexus-build-themes-'));
    const out = join(temp, 'out');
    const result = run(join(temp, 'fehlt'), out);
    expect(result.status).toBe(0);
    expect(readdirSync(out)).toEqual([]);
  });
});
