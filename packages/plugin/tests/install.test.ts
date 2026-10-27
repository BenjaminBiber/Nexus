import { describe, it, expect, beforeAll, afterAll } from 'vitest';
import { spawnSync } from 'node:child_process';
import { existsSync, mkdtempSync, realpathSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { buildPlugin, PLUGIN_OUT } from './helpers/pluginBuild.js';

// `harness install` aus dem gebauten Plugin: Kopie unter HARNESS_HOME, lauffähig ohne das Plugin.
describe('harness install', () => {
  let home: string;
  let env: NodeJS.ProcessEnv;

  const node = (args: string[]) => spawnSync(process.execPath, args, { env, encoding: 'utf8', windowsHide: true });

  beforeAll(() => {
    const built = buildPlugin();
    expect(built.ok, built.message).toBe(true);
    home = realpathSync(mkdtempSync(join(tmpdir(), 'harness-install-home-')));
    env = { ...process.env, HARNESS_HOME: home };
  }, 120000);

  afterAll(() => rmSync(home, { recursive: true, force: true }));

  it('kopiert das CLI und legt Startdateien an, ohne den PATH zu ändern (--no-path)', () => {
    const result = node([join(PLUGIN_OUT, 'lib/cli.mjs'), 'install', '--no-path']);
    expect(result.status, result.stderr).toBe(0);
    expect(existsSync(join(home, 'app', 'lib', 'cli.mjs'))).toBe(true);
    expect(existsSync(join(home, 'app', 'web', 'index.html'))).toBe(true);
    expect(existsSync(join(home, 'bin', 'harness.cmd'))).toBe(true);
    expect(existsSync(join(home, 'bin', 'harness'))).toBe(true);
    expect(existsSync(join(home, 'rules', 'docs.md'))).toBe(true);
    expect(existsSync(join(home, 'rules', 'board.md'))).toBe(true);
  });

  it('die installierte Kopie läuft eigenständig und kann sich selbst erneut installieren', () => {
    const help = node([join(home, 'app', 'lib', 'cli.mjs'), '--help']);
    expect(help.status, help.stderr).toBe(0);
    expect(help.stdout).toContain('install');
    const again = node([join(home, 'app', 'lib', 'cli.mjs'), 'install', '--no-path']);
    expect(again.status, again.stderr).toBe(0);
  });
});
