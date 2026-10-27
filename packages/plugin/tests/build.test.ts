import { describe, it, expect, beforeAll } from 'vitest';
import { existsSync, mkdtempSync, readFileSync, rmSync, statSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { buildPlugin, PLUGIN_OUT, ROOT } from './helpers/pluginBuild.js';

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
  }, 120000);

  it('erzeugt Manifest mit gültigem Namen und Beschreibung', () => {
    const manifest = JSON.parse(readFileSync(join(PLUGIN_OUT, '.claude-plugin/plugin.json'), 'utf8'));
    expect(manifest.name).toBe('ai-harness');
    expect(manifest.name).toMatch(/^[a-z0-9-]+$/);
    expect(manifest.name).not.toMatch(/^(claude|anthropic)/);
    expect(manifest.version).toMatch(/^\d+\.\d+\.\d+$/);
    expect(manifest.description.length).toBeGreaterThan(10);
  });

  it('hat für alle sieben Ereignisse einen Befehlshook in Argumentform mit Zeitlimit', () => {
    const file = JSON.parse(readFileSync(join(PLUGIN_OUT, 'hooks/hooks.json'), 'utf8'));
    expect(Object.keys(file.hooks).sort()).toEqual(Object.keys(EVENTS).sort());
    for (const [event, kind] of Object.entries(EVENTS)) {
      const hook = file.hooks[event][0].hooks[0];
      expect(hook.type, event).toBe('command');
      expect(hook.command, event).toBe('node');
      expect(hook.args, event).toEqual(['${CLAUDE_PLUGIN_ROOT}/lib/cli.mjs', 'hook', kind]);
      expect(hook.timeout, event).toBeGreaterThan(0);
      expect(hook.timeout, event).toBeLessThanOrEqual(10);
    }
  });

  it('enthält bin/harness, CLI, Server und Oberfläche', () => {
    for (const file of ['bin/harness', 'lib/cli.mjs', 'lib/server.mjs', 'web/index.html', 'skills/harness-grill/SKILL.md', 'skills/harness-implement/SKILL.md']) {
      expect(existsSync(join(PLUGIN_OUT, file)), file).toBe(true);
    }
    expect(readFileSync(join(PLUGIN_OUT, 'bin/harness'), 'utf8').startsWith('#!/usr/bin/env node')).toBe(true);
    if (process.platform !== 'win32') expect(statSync(join(PLUGIN_OUT, 'bin/harness')).mode & 0o111).toBeGreaterThan(0);
  });

  it('bündelt die Abhängigkeiten, ohne auf node_modules zu verweisen', () => {
    const cli = readFileSync(join(PLUGIN_OUT, 'lib/cli.mjs'), 'utf8');
    expect(cli).not.toMatch(/from ['"]commander['"]/);
    expect(cli).not.toMatch(/from ['"]@harness\/core['"]/);
    const server = readFileSync(join(PLUGIN_OUT, 'lib/server.mjs'), 'utf8');
    expect(server).not.toMatch(/from ['"]fastify['"]/);
  });

  it('baut nach HARNESS_PLUGIN_OUT und lässt den geteilten Repo-Ordner unberührt', () => {
    const geteilt = join(ROOT, 'packages/plugin/dist/ai-harness');
    const vorher = existsSync(geteilt) ? statSync(geteilt).mtimeMs : null;
    const eigen = join(mkdtempSync(join(tmpdir(), 'harness-plugin-ziel-')), 'ai-harness');
    try {
      const built = buildPlugin(undefined, { HARNESS_PLUGIN_OUT: eigen });
      expect(built.ok, built.message).toBe(true);
      expect(existsSync(join(eigen, 'bin/harness'))).toBe(true);
      expect(existsSync(geteilt) ? statSync(geteilt).mtimeMs : null).toBe(vorher);
    } finally {
      rmSync(eigen, { recursive: true, force: true });
    }
  }, 120000);
});
