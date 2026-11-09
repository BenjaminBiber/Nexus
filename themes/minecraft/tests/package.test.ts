// Paket: Vite-Build nach dist/, `nexus theme pack` und Inhalt des .nexus – keine Mojang-Dateien im Paket
import { execFileSync } from 'node:child_process';
import { existsSync, mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterAll, beforeAll, describe, expect, it } from 'vitest';
import { build } from 'vite';

const repo = join(import.meta.dirname, '..', '..', '..');
const theme = join(repo, 'themes', 'minecraft');
let tmp: string;
let dist: string;

// Dateinamen aus dem zentralen Verzeichnis eines ZIP-Archivs
function zipEntries(bytes: Buffer): string[] {
  let eocd = bytes.length - 22;
  while (eocd >= 0 && bytes.readUInt32LE(eocd) !== 0x06054b50) eocd--;
  if (eocd < 0) throw new Error('kein ZIP');
  const count = bytes.readUInt16LE(eocd + 10);
  let at = bytes.readUInt32LE(eocd + 16);
  const names: string[] = [];
  for (let i = 0; i < count; i++) {
    const nameLength = bytes.readUInt16LE(at + 28);
    const extraLength = bytes.readUInt16LE(at + 30);
    const commentLength = bytes.readUInt16LE(at + 32);
    names.push(bytes.toString('utf8', at + 46, at + 46 + nameLength));
    at += 46 + nameLength + extraLength + commentLength;
  }
  return names;
}

beforeAll(async () => {
  tmp = mkdtempSync(join(tmpdir(), 'theme-minecraft-'));
  dist = join(tmp, 'dist');
  await build({ configFile: join(theme, 'vite.config.ts'), logLevel: 'silent', build: { outDir: dist } });
});

afterAll(() => rmSync(tmp, { recursive: true, force: true }));

describe('Minecraft-Theme-Paket', () => {
  it('baut main.js als ESM, React und SDK bleiben extern', () => {
    const main = readFileSync(join(dist, 'main.js'), 'utf8');
    expect(main).toMatch(/from\s*["']@nexus\/theme-sdk["']/);
    expect(main).toMatch(/from\s*["']react\/jsx-runtime["']/);
    expect(main).not.toContain('react.transitional.element');
  });

  it('nexus theme pack dist erzeugt ein Paket ohne Mojang-Dateien', () => {
    const out = join(tmp, 'minecraft.nexus');
    execFileSync('cargo', ['run', '-q', '-p', 'nexus-cli', '--', 'theme', 'pack', dist, '--out', out], {
      cwd: repo,
      env: { ...process.env, NEXUS_HOME: join(tmp, 'home') },
      stdio: 'pipe',
    });
    expect(existsSync(out)).toBe(true);
    const entries = zipEntries(readFileSync(out)).filter((name) => !name.endsWith('/'));
    expect(entries.sort()).toEqual(['main.js', 'manifest.json', 'theme.css']);
    for (const name of entries) {
      expect(name).not.toMatch(/minecraft\/textures|assets\/minecraft|\.(png|ogg|jar|ttf|otf)$/);
    }
  }, 1200000);
});
