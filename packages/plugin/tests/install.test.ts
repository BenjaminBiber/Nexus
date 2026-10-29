import { describe, it, expect, beforeAll, afterAll } from 'vitest';
import { spawnSync } from 'node:child_process';
import { existsSync, mkdtempSync, realpathSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { buildPlugin, BINARY, PLUGIN_BIN } from './helpers/pluginBuild.js';

// `nexus install` aus dem gebauten Plugin: Kopie unter NEXUS_HOME, lauffähig ohne das Plugin.
// Claude Code bleibt unberührt (--no-plugin), der PATH auch (--no-path).
describe('nexus install', () => {
  let home: string;
  let env: NodeJS.ProcessEnv;

  const nexus = (bin: string, args: string[]) => spawnSync(bin, args, { env, encoding: 'utf8', windowsHide: true });

  beforeAll(() => {
    const built = buildPlugin();
    expect(built.ok, built.message).toBe(true);
    home = realpathSync(mkdtempSync(join(tmpdir(), 'nexus-install-home-')));
    env = { ...process.env, NEXUS_HOME: home };
  }, 300000);

  afterAll(() => rmSync(home, { recursive: true, force: true }));

  it('kopiert das Binary und legt Startdateien an, ohne PATH und Claude Code zu ändern', () => {
    const result = nexus(PLUGIN_BIN, ['install', '--no-path', '--no-plugin']);
    expect(result.status, result.stderr).toBe(0);
    expect(result.stdout).toContain('Plugin in Claude Code nicht angemeldet (--no-plugin).');
    expect(result.stdout).toContain('PATH nicht geändert (--no-path).');
    expect(existsSync(join(home, 'bin', BINARY))).toBe(true);
    expect(existsSync(join(home, 'app', 'lib', 'server.mjs'))).toBe(true);
    expect(existsSync(join(home, 'app', 'web', 'index.html'))).toBe(true);
    expect(existsSync(join(home, 'bin', 'harness.cmd'))).toBe(true);
    expect(existsSync(join(home, 'bin', 'harness'))).toBe(true);
    expect(existsSync(join(home, 'rules', 'docs.md'))).toBe(true);
    expect(existsSync(join(home, 'rules', 'board.md'))).toBe(true);
  });

  it('die installierte Kopie läuft eigenständig und kann sich selbst erneut installieren', () => {
    const installiert = join(home, 'bin', BINARY);
    const help = nexus(installiert, ['--help']);
    expect(help.status, help.stderr).toBe(0);
    expect(help.stdout).toContain('install');
    // Die Kopie liegt in <home>/bin, also nicht in einer Plugin-Wurzel: sie nennt den Grund
    const again = nexus(installiert, ['install', '--no-path', '--no-plugin']);
    expect(again.status).toBe(1);
    expect(again.stderr).toContain('nur aus dem gebauten Plugin');
  });
});
