import { describe, it, expect, beforeAll } from 'vitest';
import { spawnSync } from 'node:child_process';
import { existsSync, mkdtempSync, readdirSync, readFileSync, rmSync, statSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { buildPlugin, BINARY, PLUGIN_BIN, PLUGIN_OUT, ROOT } from './helpers/pluginBuild.js';

const EVENTS: Record<string, string> = {
  SessionStart: 'session-start',
  UserPromptSubmit: 'user-prompt',
  Notification: 'notification',
  Stop: 'stop',
  SubagentStart: 'subagent-start',
  SubagentStop: 'subagent-stop',
  SessionEnd: 'session-end',
};

describe('Plugin-Build', () => {
  beforeAll(() => {
    const built = buildPlugin();
    expect(built.ok, built.message).toBe(true);
  }, 300000);

  it('erzeugt Manifest mit gültigem Namen und Beschreibung', () => {
    const manifest = JSON.parse(readFileSync(join(PLUGIN_OUT, '.claude-plugin/plugin.json'), 'utf8'));
    expect(manifest.name).toBe('nexus');
    expect(manifest.name).toMatch(/^[a-z0-9-]+$/);
    expect(manifest.name).not.toMatch(/^(claude|anthropic)/);
    expect(manifest.version).toMatch(/^\d+\.\d+\.\d+$/);
    expect(manifest.description.length).toBeGreaterThan(10);
  });

  it('hat für alle sieben Ereignisse einen Befehlshook auf das Binary mit Zeitlimit', () => {
    const file = JSON.parse(readFileSync(join(PLUGIN_OUT, 'hooks/hooks.json'), 'utf8'));
    expect(Object.keys(file.hooks).sort()).toEqual(Object.keys(EVENTS).sort());
    for (const [event, kind] of Object.entries(EVENTS)) {
      const hook = file.hooks[event][0].hooks[0];
      expect(hook.type, event).toBe('command');
      expect(hook.command, event).toBe('${CLAUDE_PLUGIN_ROOT}/bin/nexus');
      expect(hook.args, event).toEqual(['hook', kind]);
      expect(hook.timeout, event).toBeGreaterThan(0);
      expect(hook.timeout, event).toBeLessThanOrEqual(10);
    }
  });

  // Claude Code startet die Hooks unter Windows über die Git Bash; sie löst `bin/nexus` zu
  // `bin/nexus.exe` auf. Der Test prüft, dass die Datei, die auf dieser Plattform wirklich
  // gestartet wird, im Plugin liegt und läuft.
  it('der Hook-Befehl löst sich auf dieser Plattform auf ein ausführbares Binary auf', () => {
    const hooks = JSON.parse(readFileSync(join(PLUGIN_OUT, 'hooks/hooks.json'), 'utf8'));
    const command: string = hooks.hooks.SessionStart[0].hooks[0].command;
    const ohneWurzel = command.replace('${CLAUDE_PLUGIN_ROOT}/', '');
    const aufgeloest = join(PLUGIN_OUT, process.platform === 'win32' ? `${ohneWurzel}.exe` : ohneWurzel);
    expect(aufgeloest).toBe(PLUGIN_BIN);
    expect(existsSync(aufgeloest), aufgeloest).toBe(true);
    if (process.platform !== 'win32') expect(statSync(aufgeloest).mode & 0o111).toBeGreaterThan(0);
    const hilfe = spawnSync(aufgeloest, ['--help'], { encoding: 'utf8', windowsHide: true });
    expect(hilfe.status, hilfe.stderr).toBe(0);
    expect(hilfe.stdout).toContain('Aufruf: nexus');
  });

  it('enthält das Binary, den Alias bin/harness und die Oberfläche', () => {
    for (const file of [`bin/${BINARY}`, 'bin/harness', 'web/index.html', 'skills/harness-grill/SKILL.md', 'skills/harness-implement/SKILL.md']) {
      expect(existsSync(join(PLUGIN_OUT, file)), file).toBe(true);
    }
    expect(existsSync(join(PLUGIN_OUT, 'lib')), 'der Node-Server entfällt mit dem Binary').toBe(false);
    const alias = readFileSync(join(PLUGIN_OUT, 'bin/harness'), 'utf8');
    expect(alias.startsWith('#!/bin/sh')).toBe(true);
    expect(alias).toContain('NEXUS_INVOKED_AS=harness');
    expect(alias).toContain('"$(dirname "$0")/nexus"');
    if (process.platform !== 'win32') expect(statSync(join(PLUGIN_OUT, 'bin/harness')).mode & 0o111).toBeGreaterThan(0);
  });

  it('startet über den Alias bin/harness dasselbe Programm', () => {
    if (process.platform === 'win32') return; // Startdatei für die Bash; cmd nutzt den Alias nicht
    const result = spawnSync(join(PLUGIN_OUT, 'bin/harness'), ['--help'], { encoding: 'utf8' });
    expect(result.status, result.stderr).toBe(0);
    expect(result.stdout).toContain('Aufruf: nexus');
    expect(result.stderr).toContain('`harness` heißt jetzt `nexus`');
  });

  it('kommt ohne Node-Dateien aus: keine .mjs und keine node_modules im Plugin', () => {
    for (const eintrag of readdirSync(PLUGIN_OUT, { recursive: true, encoding: 'utf8' })) {
      expect(eintrag.endsWith('.mjs'), eintrag).toBe(false);
      expect(eintrag.includes('node_modules'), eintrag).toBe(false);
    }
  });

  it('baut nach NEXUS_PLUGIN_OUT und lässt den geteilten Repo-Ordner unberührt', () => {
    const geteilt = join(ROOT, 'packages/plugin/dist/nexus');
    const vorher = existsSync(geteilt) ? statSync(geteilt).mtimeMs : null;
    const eigen = join(mkdtempSync(join(tmpdir(), 'nexus-plugin-ziel-')), 'nexus');
    try {
      const built = buildPlugin(undefined, { NEXUS_PLUGIN_OUT: eigen });
      expect(built.ok, built.message).toBe(true);
      expect(existsSync(join(eigen, 'bin', BINARY))).toBe(true);
      expect(existsSync(geteilt) ? statSync(geteilt).mtimeMs : null).toBe(vorher);
    } finally {
      rmSync(eigen, { recursive: true, force: true });
    }
  }, 300000);
});
