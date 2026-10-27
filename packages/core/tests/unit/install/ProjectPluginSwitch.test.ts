import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { ProjectPluginSwitch, PLUGIN_ID } from '../../../src/install/ProjectPluginSwitch.js';

describe('ProjectPluginSwitch', () => {
  let dir: string;
  const sw = new ProjectPluginSwitch();
  const read = () => JSON.parse(readFileSync(sw.file(dir), 'utf8')) as Record<string, unknown>;

  beforeEach(() => {
    dir = mkdtempSync(join(tmpdir(), 'harness-switch-'));
  });

  afterEach(() => rmSync(dir, { recursive: true, force: true }));

  it('legt .claude/settings.local.json mit dem Eintrag an', async () => {
    expect(await sw.enable(dir)).toBe(true);
    expect(read()).toEqual({ enabledPlugins: { [PLUGIN_ID]: true } });
  });

  it('ist wiederholbar und meldet false, wenn schon eingeschaltet', async () => {
    await sw.enable(dir);
    expect(await sw.enable(dir)).toBe(false);
  });

  it('lässt vorhandene Einstellungen und andere Plugins unberührt', async () => {
    mkdirSync(join(dir, '.claude'));
    writeFileSync(sw.file(dir), JSON.stringify({ permissions: { allow: ['Bash(ls)'] }, enabledPlugins: { 'x@y': true } }));
    await sw.enable(dir);
    expect(read()).toEqual({
      permissions: { allow: ['Bash(ls)'] },
      enabledPlugins: { 'x@y': true, [PLUGIN_ID]: true },
    });
  });

  it('überschreibt kaputtes JSON nie', async () => {
    mkdirSync(join(dir, '.claude'));
    writeFileSync(sw.file(dir), '{ kaputt');
    await expect(sw.enable(dir)).rejects.toThrow(/kein gültiges JSON/);
    expect(readFileSync(sw.file(dir), 'utf8')).toBe('{ kaputt');
  });

  it('entfernt nur den eigenen Eintrag', async () => {
    mkdirSync(join(dir, '.claude'));
    writeFileSync(sw.file(dir), JSON.stringify({ enabledPlugins: { 'x@y': true, [PLUGIN_ID]: true } }));
    expect(await sw.disable(dir)).toBe(true);
    expect(read()).toEqual({ enabledPlugins: { 'x@y': true } });
    expect(await sw.disable(dir)).toBe(false);
  });

  it('disable ohne Datei tut nichts', async () => {
    expect(await sw.disable(dir)).toBe(false);
    expect(existsSync(sw.file(dir))).toBe(false);
  });
});
