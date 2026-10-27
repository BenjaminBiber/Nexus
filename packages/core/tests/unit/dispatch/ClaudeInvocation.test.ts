import { describe, expect, it } from 'vitest';
import { ClaudeInvocation } from '../../../src/dispatch/ClaudeInvocation.js';
import { PLANNING_DENIED_TOOLS, PLANNING_TOOLS, RESEARCH_DENIED_TOOLS, RESEARCH_TOOLS } from '../../../src/dispatch/DispatchConfig.js';
import { DispatchConfigNormalizer } from '../../../src/dispatch/DispatchConfigNormalizer.js';

const cfg = new DispatchConfigNormalizer().normalize({ claude: { allowedTools: ['Read', 'Bash(harness:*)'], extraArgs: ['--verbose'] } });
const run = { profile: 'standard', why: '', model: 'opus', effort: 'high', maxTurns: 80, timeoutMinutes: 90 };

describe('ClaudeInvocation', () => {
  it('Recherche: fester Rechte-Modus default, keine extraArgs, nur Recherche-Tools, Verbote vor den Erlaubnissen', () => {
    const risky = new DispatchConfigNormalizer().normalize({
      claude: { permissionMode: 'bypassPermissions', extraArgs: ['--dangerously-skip-permissions'], allowedTools: ['Bash(*)'] },
    });
    const args = new ClaudeInvocation().args(risky, run, [], 'research');
    expect(args).not.toContain('--dangerously-skip-permissions');
    expect(args).not.toContain('Bash(*)');
    expect(args[args.indexOf('--permission-mode') + 1]).toBe('default');
    expect(args.filter((a) => a === '--permission-mode')).toHaveLength(1);
    const deny = args.indexOf('--disallowedTools');
    const allow = args.indexOf('--allowedTools');
    expect(deny).toBeGreaterThan(args.indexOf('--permission-mode'));
    expect(allow).toBeGreaterThan(deny);
    // Keine Liste darf eine Option schlucken: zwischen den Listen und danach stehen nur Werte ohne führendes „-“
    expect(args.slice(deny + 1, allow).every((a) => !a.startsWith('-'))).toBe(true);
    expect(args.slice(allow + 1).every((a) => !a.startsWith('-'))).toBe(true);
    expect(args.slice(deny + 1, allow)).toEqual(RESEARCH_DENIED_TOOLS);
    expect(args.slice(deny + 1, allow)).toEqual(expect.arrayContaining([
      'MultiEdit', 'NotebookEdit', 'Bash(git commit:*)', 'Bash(git push:*)', 'Bash(git checkout:*)', 'Bash(git reset:*)',
      'Bash(git switch:*)', 'Bash(git merge:*)', 'Bash(git rebase:*)', 'Bash(npm:*)', 'Bash(node:*)', 'Bash(harness board dispatch:*)',
      'Bash(harness board question research-start:*)', 'Bash(harness board question answer:*)', 'Bash(harness board question discard:*)',
      'Bash(git diff --output:*)', 'Bash(git log --output:*)',
    ]));
    expect(args.slice(allow + 1)).toEqual(RESEARCH_TOOLS);
    expect(RESEARCH_TOOLS).not.toContain('Write');
    expect(RESEARCH_TOOLS).not.toContain('Edit');
    expect(RESEARCH_TOOLS).toEqual(expect.arrayContaining(['Write(./research.json)', 'Edit(./research.json)']));
    expect(RESEARCH_DENIED_TOOLS).not.toContain('Write');
    expect(RESEARCH_TOOLS.filter((t) => t.startsWith('Bash(harness'))).toEqual([
      'Bash(harness board question show:*)', 'Bash(harness board question research:*)', 'Bash(harness board show:*)', 'Bash(harness board list:*)',
    ]);
  });

  it('setzt HARNESS_TICKET nur mit Ticket', () => {
    const env = new ClaudeInvocation().env({ HARNESS_TICKET: 'ARC-0001' }, cfg, null);
    expect(env.HARNESS_TICKET).toBeUndefined();
    expect(new ClaudeInvocation().env({}, cfg, 'ARC-0002').HARNESS_TICKET).toBe('ARC-0002');
  });

  it('baut die Argumente mit stream-json (samt --verbose, nicht doppelt aus extraArgs), --allowedTools zuletzt', () => {
    expect(new ClaudeInvocation().args(cfg, run)).toEqual([
      '-p', '--output-format', 'stream-json', '--verbose', '--max-turns', '80', '--model', 'opus', '--effort', 'high',
      '--permission-mode', 'acceptEdits', '--allowedTools', 'Read', 'Bash(harness:*)',
    ]);
    const extra = new DispatchConfigNormalizer().normalize({ claude: { extraArgs: ['--add-dir', 'x'] } });
    expect(new ClaudeInvocation().args(extra, run).slice(0, 4)).toEqual(['-p', '--output-format', 'stream-json', '--verbose']);
    expect(new ClaudeInvocation().args(extra, run)).toEqual(expect.arrayContaining(['--add-dir', 'x']));
    for (const kind of ['merge', 'research'] as const) {
      expect(new ClaudeInvocation().args(cfg, run, [], kind).slice(0, 4)).toEqual(['-p', '--output-format', 'stream-json', '--verbose']);
    }
  });

  it('lässt fehlendes Modell und Effort weg und ergänzt Werkzeuge', () => {
    const args = new ClaudeInvocation().args(cfg, { ...run, model: null, effort: null }, ['Bash(git merge:*)']);
    expect(args).not.toContain('--model');
    expect(args).not.toContain('--effort');
    expect(args.slice(-4)).toEqual(['--allowedTools', 'Read', 'Bash(harness:*)', 'Bash(git merge:*)']);
  });

  it('entfernt API-Keys und Session-Variablen und setzt HARNESS_TICKET', () => {
    const env = new ClaudeInvocation().env(
      { PATH: 'p', ANTHROPIC_API_KEY: 'k', ANTHROPIC_AUTH_TOKEN: 't', HARNESS_SESSION: 's', CLAUDECODE: '1', CLAUDE_CODE_ENTRYPOINT: 'cli' }, cfg, 'ARC-0001');
    expect(env).toEqual({ PATH: 'p', HARNESS_TICKET: 'ARC-0001', BASH_DEFAULT_TIMEOUT_MS: '900000', BASH_MAX_TIMEOUT_MS: '1800000' });
    const keep = new DispatchConfigNormalizer().normalize({ claude: { useSubscription: false } });
    expect(new ClaudeInvocation().env({ ANTHROPIC_API_KEY: 'k' }, keep, 'ARC-0001').ANTHROPIC_API_KEY).toBe('k');
  });

  it('setzt mit gefundener Git Bash PowerShell-Tool aus und Bash-Pfad; ohne bleibt die Umgebung unverändert', () => {
    const bash = 'C:/G/bin/bash.exe';
    const env = new ClaudeInvocation().env({ PATH: 'p' }, cfg, 'ARC-0001', bash);
    const timeouts = { BASH_DEFAULT_TIMEOUT_MS: '900000', BASH_MAX_TIMEOUT_MS: '1800000' };
    expect(env).toEqual({ PATH: 'p', HARNESS_TICKET: 'ARC-0001', ...timeouts, CLAUDE_CODE_USE_POWERSHELL_TOOL: '0', CLAUDE_CODE_GIT_BASH_PATH: bash });
    expect(new ClaudeInvocation().env({ PATH: 'p' }, cfg, 'ARC-0001', null)).toEqual({ PATH: 'p', HARNESS_TICKET: 'ARC-0001', ...timeouts });
  });

  it('setzt längere Bash-Zeitlimits, eigene Werte aus der Umgebung bleiben', () => {
    const env = new ClaudeInvocation().env({ BASH_MAX_TIMEOUT_MS: '60000' }, cfg, 'ARC-0001');
    expect(env.BASH_DEFAULT_TIMEOUT_MS).toBe('900000');
    expect(env.BASH_MAX_TIMEOUT_MS).toBe('60000');
  });

  it('übernimmt gitBashPath aus der Einstellung (leer = null)', () => {
    expect(cfg.claude.gitBashPath).toBeNull();
    expect(new DispatchConfigNormalizer().normalize({ claude: { gitBashPath: ' C:/x/bash.exe ' } }).claude.gitBashPath).toBe('C:/x/bash.exe');
    expect(new DispatchConfigNormalizer().normalize({ claude: { gitBashPath: '' } }).claude.gitBashPath).toBeNull();
  });

  it('Planung: fester Rechte-Modus default, keine extraArgs, nur Planungs-Tools, Verbote der Recherche und der Statusänderung', () => {
    const risky = new DispatchConfigNormalizer().normalize({
      claude: { permissionMode: 'bypassPermissions', extraArgs: ['--dangerously-skip-permissions'], allowedTools: ['Bash(*)'] },
    });
    const args = new ClaudeInvocation().args(risky, run, [], 'plan');
    expect(args).not.toContain('--dangerously-skip-permissions');
    expect(args).not.toContain('Bash(*)');
    expect(args[args.indexOf('--permission-mode') + 1]).toBe('default');
    const deny = args.indexOf('--disallowedTools');
    const allow = args.indexOf('--allowedTools');
    expect(args.slice(deny + 1, allow)).toEqual(PLANNING_DENIED_TOOLS);
    expect(args.slice(allow + 1)).toEqual(PLANNING_TOOLS);
    expect(PLANNING_DENIED_TOOLS).toEqual(expect.arrayContaining([...RESEARCH_DENIED_TOOLS, 'Bash(harness board move:*)', 'Bash(harness board set:*)', 'Bash(harness board claim:*)']));
    expect(PLANNING_TOOLS).toEqual(expect.arrayContaining(['Write(./plan.json)', 'Bash(harness board plan-json:*)', 'Bash(harness board question add:*)']));
    expect(PLANNING_TOOLS).not.toContain('Write');
    expect(PLANNING_TOOLS).not.toContain('Bash(harness:*)');
  });
});
