// Vorlage themes/_template: Vite-Build nach dist/ (ESM, React und SDK extern) und Paket mit `nexus theme pack`
import { execFileSync } from 'node:child_process';
import { existsSync, mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterAll, beforeAll, describe, expect, it } from 'vitest';
import { build } from 'vite';

const repo = join(import.meta.dirname, '..', '..', '..');
const template = join(repo, 'themes', '_template');
let tmp;
let dist;

beforeAll(async () => {
  tmp = mkdtempSync(join(tmpdir(), 'theme-template-'));
  dist = join(tmp, 'dist');
  // Build mit der Konfiguration der Vorlage, Ausgabe in einen Temp-Ordner statt in die Vorlage
  await build({ configFile: join(template, 'vite.config.ts'), logLevel: 'silent', build: { outDir: dist } });
});

afterAll(() => rmSync(tmp, { recursive: true, force: true }));

describe('Theme-Vorlage', () => {
  it('baut main.js als ESM mit export default, React und SDK bleiben extern', () => {
    const main = readFileSync(join(dist, 'main.js'), 'utf8');
    expect(main).toMatch(/export\s*\{[^}]*\bas default\b[^}]*\}|export default/);
    expect(main).toMatch(/from\s*["']react\/jsx-runtime["']/);
    expect(main).toMatch(/from\s*["']@nexus\/theme-sdk["']/);
    expect(main).toContain('template-shell');
    // Nicht gebündelt: kein React-Quelltext im Ergebnis
    expect(main).not.toContain('react.transitional.element');
    expect(main).not.toContain('__SECRET_INTERNALS');
    expect(main.length).toBeLessThan(5000);
  });

  it('legt Manifest und CSS neben main.js', () => {
    expect(JSON.parse(readFileSync(join(dist, 'manifest.json'), 'utf8'))).toEqual(
      expect.objectContaining({ id: 'my-theme', slots: ['Shell'] }),
    );
    expect(existsSync(join(dist, 'theme.css'))).toBe(true);
  });

  it('dist/ lässt sich mit nexus theme pack packen', () => {
    const out = join(tmp, 'template.nexus');
    execFileSync('cargo', ['run', '-q', '-p', 'nexus-cli', '--', 'theme', 'pack', dist, '--out', out], {
      cwd: repo,
      env: { ...process.env, NEXUS_HOME: join(tmp, 'home') },
      stdio: 'pipe',
    });
    expect(existsSync(out)).toBe(true);
  }, 1200000);
});
