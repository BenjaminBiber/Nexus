import { spawnSync } from 'node:child_process';
import { existsSync, mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterAll, beforeAll, describe, expect, it } from 'vitest';
import { ROOT } from './helpers/pluginBuild.js';

describe('npm run customize', () => {
  let custom: string;
  // Startet scripts/customize.mjs mit dem Testordner als Ordner der Anpassungen
  const run = (...args: string[]) => {
    const r = spawnSync(process.execPath, [join(ROOT, 'scripts/customize.mjs'), ...args], { cwd: ROOT, encoding: 'utf8', windowsHide: true, env: { ...process.env, HARNESS_CUSTOM_DIR: custom } });
    return { code: r.status, out: `${r.stdout}${r.stderr}` };
  };

  beforeAll(() => { custom = mkdtempSync(join(tmpdir(), 'harness-customize-')); });
  afterAll(() => rmSync(custom, { recursive: true, force: true }));

  it('prüft ohne Anpassungen', () => {
    expect(run('check')).toMatchObject({ code: 0, out: expect.stringContaining('Keine Anpassungen, überall gilt der Standard.') });
  });

  it('kopiert, prüft und setzt zurück', () => {
    expect(run('copy', 'skill', 'harness-grill').code).toBe(0);
    expect(existsSync(join(custom, 'skills', 'harness-grill', 'SKILL.md'))).toBe(true);
    expect(run('check').out).toContain('harness-grill (ersetzt)');
    expect(run('reset', 'skill', 'harness-grill').code).toBe(0);
    expect(existsSync(join(custom, 'skills', 'harness-grill'))).toBe(false);
  });

  it('kopiert den Workflow und erkennt ihn als gültig', () => {
    expect(run('copy', 'workflow').code).toBe(0);
    expect(existsSync(join(custom, 'workflow.ts'))).toBe(true);
    const check = run('check');
    expect(check.code).toBe(0);
    expect(check.out).toContain('Workflow        angepasst');
    expect(check.out).not.toContain('Rückfälle auf den Standard');
    expect(run('reset', 'workflow').code).toBe(0);
  });

  it('nennt bei falschem Aufruf die Verwendung', () => {
    const result = run('löschen');
    expect(result.code).toBe(1);
    expect(result.out).toContain('Aufruf: npm run customize --');
  });

  it('kopiert bei einer unbekannten Option nichts', () => {
    const result = run('copy', 'skill', 'harness-grill', '--rebas');
    expect(result.code).toBe(1);
    expect(result.out).toContain('Aufruf: npm run customize --');
    expect(existsSync(join(custom, 'skills', 'harness-grill'))).toBe(false);
  });
});
