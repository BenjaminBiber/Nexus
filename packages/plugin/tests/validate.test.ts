import { describe, it, expect, beforeAll } from 'vitest';
import { spawnSync } from 'node:child_process';
import { buildPlugin, PLUGIN_OUT } from './helpers/pluginBuild.js';

// Ist die Claude-Code-CLI vorhanden?
const claude = spawnSync('claude', ['--version'], { encoding: 'utf8', shell: process.platform === 'win32', windowsHide: true });
const hasClaude = claude.status === 0;

describe.skipIf(!hasClaude)('claude plugin validate', () => {
  beforeAll(() => {
    const built = buildPlugin();
    expect(built.ok, built.message).toBe(true);
  }, 120000);

  it('akzeptiert das gebaute Plugin ohne Fehler', () => {
    const result = spawnSync('claude', ['plugin', 'validate', PLUGIN_OUT], { encoding: 'utf8', shell: process.platform === 'win32', windowsHide: true });
    const output = `${result.stdout}${result.stderr}`;
    expect(result.status, output).toBe(0);
    expect(output).not.toMatch(/Validation failed/i);
  }, 60000);
});
