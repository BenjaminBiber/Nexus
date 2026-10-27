import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { HarnessHome } from '../../../src/registry/HarnessHome.js';
import { CliInstaller } from '../../../src/install/CliInstaller.js';
import { ShellProfilePath } from '../../../src/install/ShellProfilePath.js';
import type { UserPath } from '../../../src/install/UserPath.js';

// Merkt sich die Aufrufe statt den PATH wirklich zu ändern
class FakeUserPath implements UserPath {
  readonly added: string[] = [];
  constructor(private readonly result = true) {}
  async add(dir: string): Promise<boolean> {
    this.added.push(dir);
    return this.result;
  }
}

describe('CliInstaller', () => {
  let base: string;
  let plugin: string;
  let home: HarnessHome;

  // Legt ein minimales gebautes Plugin an: lib/cli.mjs, lib/server.mjs, web/index.html, rules/docs.md
  beforeEach(() => {
    base = mkdtempSync(join(tmpdir(), 'harness-install-'));
    plugin = join(base, 'plugin');
    mkdirSync(join(plugin, 'lib'), { recursive: true });
    mkdirSync(join(plugin, 'web'), { recursive: true });
    mkdirSync(join(plugin, 'rules'), { recursive: true });
    writeFileSync(join(plugin, 'lib', 'cli.mjs'), 'console.log("cli");');
    writeFileSync(join(plugin, 'lib', 'server.mjs'), 'console.log("server");');
    writeFileSync(join(plugin, 'web', 'index.html'), '<html></html>');
    writeFileSync(join(plugin, 'rules', 'docs.md'), '# Regeln');
    home = new HarnessHome({ HARNESS_HOME: join(base, 'home') });
  });

  afterEach(() => rmSync(base, { recursive: true, force: true }));

  it('erkennt die Plugin-Wurzel nur bei vollständigem Aufbau', () => {
    const installer = new CliInstaller(home, new FakeUserPath());
    expect(installer.bundleRoot(join(plugin, 'lib', 'cli.mjs'))).toBe(plugin);
    expect(installer.bundleRoot(join(plugin, 'lib', 'andere.mjs'))).toBeNull();
    rmSync(join(plugin, 'web'), { recursive: true });
    expect(installer.bundleRoot(join(plugin, 'lib', 'cli.mjs'))).toBeNull();
  });

  it('kopiert CLI, Server und Oberfläche und legt beide Startdateien an', async () => {
    const result = await new CliInstaller(home, new FakeUserPath()).install(plugin);
    expect(existsSync(join(result.appDir, 'lib', 'cli.mjs'))).toBe(true);
    expect(existsSync(join(result.appDir, 'lib', 'server.mjs'))).toBe(true);
    expect(existsSync(join(result.appDir, 'web', 'index.html'))).toBe(true);
    expect(readFileSync(join(result.binDir, 'harness.cmd'), 'utf8')).toContain('..\\app\\lib\\cli.mjs');
    expect(readFileSync(join(result.binDir, 'harness'), 'utf8')).toContain('/../app/lib/cli.mjs');
    expect(readFileSync(join(result.binDir, 'harness'), 'utf8')).not.toContain('\r');
  });

  it('trägt den Startordner in den PATH ein und meldet, ob er neu war', async () => {
    const path = new FakeUserPath(true);
    const result = await new CliInstaller(home, path).install(plugin);
    expect(path.added).toEqual([result.binDir]);
    expect(result.pathAdded).toBe(true);
    const again = await new CliInstaller(home, new FakeUserPath(false)).install(plugin);
    expect(again.pathAdded).toBe(false);
  });

  it('lässt den PATH mit registerPath=false unberührt', async () => {
    const path = new FakeUserPath();
    const result = await new CliInstaller(home, path).install(plugin, { registerPath: false });
    expect(path.added).toEqual([]);
    expect(result.pathSkipped).toBe(true);
  });

  it('ist wiederholbar und entfernt veraltete Dateien der Kopie', async () => {
    const installer = new CliInstaller(home, new FakeUserPath());
    const first = await installer.install(plugin);
    writeFileSync(join(first.appDir, 'lib', 'alt.mjs'), 'alt');
    writeFileSync(join(plugin, 'lib', 'cli.mjs'), 'console.log("neu");');
    await installer.install(plugin);
    expect(existsSync(join(first.appDir, 'lib', 'alt.mjs'))).toBe(false);
    expect(readFileSync(join(first.appDir, 'lib', 'cli.mjs'), 'utf8')).toContain('neu');
  });

  it('kopiert nicht auf sich selbst, wenn der Aufruf aus der installierten Kopie kommt', async () => {
    const installer = new CliInstaller(home, new FakeUserPath());
    const first = await installer.install(plugin);
    const second = await installer.install(first.appDir);
    expect(existsSync(join(second.appDir, 'lib', 'cli.mjs'))).toBe(true);
  });

  it('legt die zentralen Regeln unter rules/ ab, auch beim Aufruf aus der installierten Kopie', async () => {
    const installer = new CliInstaller(home, new FakeUserPath());
    const first = await installer.install(plugin);
    expect(first.rulesDir).toBe(home.rulesDir);
    expect(readFileSync(join(home.rulesDir, 'docs.md'), 'utf8')).toBe('# Regeln');
    writeFileSync(join(home.rulesDir, 'docs.md'), 'von Hand geändert');
    await installer.install(first.appDir);
    expect(readFileSync(join(home.rulesDir, 'docs.md'), 'utf8')).toBe('# Regeln');
  });

  it('ersetzt die Regeln beim Aktualisieren und entfernt veraltete Dateien', async () => {
    const installer = new CliInstaller(home, new FakeUserPath());
    await installer.install(plugin);
    writeFileSync(join(home.rulesDir, 'alt.md'), 'alt');
    writeFileSync(join(plugin, 'rules', 'docs.md'), '# Neu');
    await installer.install(plugin);
    expect(existsSync(join(home.rulesDir, 'alt.md'))).toBe(false);
    expect(readFileSync(join(home.rulesDir, 'docs.md'), 'utf8')).toBe('# Neu');
  });

  it('kommt ohne rules/ im Plugin aus (ältere Builds)', async () => {
    rmSync(join(plugin, 'rules'), { recursive: true });
    await new CliInstaller(home, new FakeUserPath()).install(plugin);
    expect(existsSync(home.rulesDir)).toBe(false);
  });

  it('legt die angewendeten Anpassungen unter ~/.harness/customizations ab', async () => {
    mkdirSync(join(plugin, 'customizations'), { recursive: true });
    writeFileSync(join(plugin, 'customizations', 'overview.json'), '{"version":1}');
    mkdirSync(home.customizationsDir, { recursive: true });
    writeFileSync(join(home.customizationsDir, 'workflow.json'), '[]');
    await new CliInstaller(home, new FakeUserPath()).install(plugin);
    expect(readFileSync(join(home.customizationsDir, 'overview.json'), 'utf8')).toBe('{"version":1}');
    expect(existsSync(join(home.customizationsDir, 'workflow.json'))).toBe(false);
  });

  it('meldet eine unvollständige Quelle verständlich', async () => {
    rmSync(join(plugin, 'lib', 'cli.mjs'));
    await expect(new CliInstaller(home, new FakeUserPath()).install(plugin)).rejects.toThrow(/lib\/cli\.mjs/);
  });
});

describe('ShellProfilePath', () => {
  let homeDir: string;

  beforeEach(() => {
    homeDir = mkdtempSync(join(tmpdir(), 'harness-profile-'));
  });

  afterEach(() => rmSync(homeDir, { recursive: true, force: true }));

  it('ergänzt vorhandene Profile genau einmal', async () => {
    writeFileSync(join(homeDir, '.bashrc'), 'alias ll="ls -l"');
    const path = new ShellProfilePath(homeDir);
    expect(await path.add('/home/x/.harness/bin')).toBe(true);
    expect(await path.add('/home/x/.harness/bin')).toBe(false);
    const text = readFileSync(join(homeDir, '.bashrc'), 'utf8');
    expect(text).toContain('alias ll="ls -l"\n');
    expect(text.match(/\.harness\/bin/g)).toHaveLength(1);
    expect(existsSync(join(homeDir, '.profile'))).toBe(false);
  });

  it('legt .profile an, wenn es kein Profil gibt', async () => {
    expect(await new ShellProfilePath(homeDir).add('/h/bin')).toBe(true);
    expect(readFileSync(join(homeDir, '.profile'), 'utf8')).toContain('export PATH="/h/bin:$PATH"');
  });
});
