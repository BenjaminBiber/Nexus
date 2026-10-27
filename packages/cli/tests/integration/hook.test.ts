import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { createServer as createNetServer } from 'node:net';
import { existsSync, mkdtempSync, readFileSync, realpathSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { BoardConfigStore, HarnessHome } from '@harness/core';
import { TestDb } from '../../../db/tests/helpers/TestDb.js';
import { TempProject } from './helpers/TempProject.js';

const SERVER_ENTRY = resolve(__dirname, '../../../server/src/main.ts');

// Sucht einen freien Port
function freePort(): Promise<number> {
  return new Promise((done) => {
    const s = createNetServer();
    s.listen(0, '127.0.0.1', () => {
      const port = (s.address() as { port: number }).port;
      s.close(() => done(port));
    });
  });
}

// Beendet den Server, den ein Hook gestartet hat
async function stopServer(home: string): Promise<void> {
  const info = join(home, 'server.json');
  if (existsSync(info)) {
    try {
      process.kill(JSON.parse(readFileSync(info, 'utf8')).pid);
    } catch {
      /* läuft nicht mehr */
    }
  }
  await new Promise((r) => setTimeout(r, 300));
}

describe('harness hook ohne Board-DB', () => {
  let p: TempProject;
  let home: string;
  let port: number;
  let env: Record<string, string>;
  const hook = (kind: string, payload: object | string, extraEnv: Record<string, string> = {}, cwd = p.path) =>
    p.run(['hook', kind], cwd, { ...env, ...extraEnv }, typeof payload === 'string' ? payload : JSON.stringify(payload));
  const sessions = async () => {
    const projects = await (await fetch(`http://127.0.0.1:${port}/api/projects`)).json();
    return (await (await fetch(`http://127.0.0.1:${port}/api/projects/${projects[0].id}/sessions`)).json()) as any[];
  };

  beforeEach(async () => {
    p = new TempProject();
    home = realpathSync(mkdtempSync(join(tmpdir(), 'harness-hook-')));
    port = await freePort();
    env = { HARNESS_HOME: home, HARNESS_SERVER_ENTRY: SERVER_ENTRY, HARNESS_PORT: String(port), HARNESS_HOOK_DEADLINE_MS: '60000', HARNESS_HOOK_SERVER_WAIT_MS: '45000' };
    p.run(['init'], p.path, env);
  });
  afterEach(async () => {
    await stopServer(home);
    p.dispose();
    rmSync(home, { recursive: true, force: true });
  });

  it('meldet Start, Benachrichtigung, Subagenten, Stop und Ende ohne Kontext und setzt nur HARNESS_SESSION', async () => {
    const envFile = join(home, 'claude-env.sh');
    const start = hook('session-start', { session_id: 's1', cwd: p.path }, { CLAUDE_ENV_FILE: envFile });
    expect(start.code).toBe(0);
    expect(start.stdout).toBe('');
    expect(readFileSync(envFile, 'utf8')).toBe("export HARNESS_SESSION='s1'\n");
    hook('notification', { session_id: 's1', cwd: p.path, type: 'permission_prompt' });
    hook('subagent-start', { session_id: 's1', cwd: p.path, agent_id: 'a1', agent_type: 'Explore' });
    let s = (await sessions())[0];
    expect(s).toMatchObject({ id: 's1', tickets: [] });
    expect(s.subagents[0]).toMatchObject({ id: 'a1', type: 'Explore', state: 'running' });
    hook('subagent-stop', { session_id: 's1', cwd: p.path, agent_id: 'a1', agent_type: 'Explore' });
    hook('stop', { session_id: 's1', cwd: p.path });
    expect((await sessions())[0].state).toBe('idle');
    hook('session-end', { session_id: 's1', cwd: p.path, reason: 'other' });
    s = (await sessions())[0];
    expect(s).toMatchObject({ state: 'ended', displayState: 'ended' });
  });

  it('bleibt in Projekten ohne harness init wirkungslos: keine Ausgabe, keine Dateien, kein Server', async () => {
    const bare = new TempProject();
    const bareHome = realpathSync(mkdtempSync(join(tmpdir(), 'harness-bare-')));
    try {
      const freshPort = await freePort();
      const result = bare.run(['hook', 'session-start'], bare.path, { ...env, HARNESS_HOME: bareHome, HARNESS_PORT: String(freshPort), CLAUDE_ENV_FILE: join(bareHome, 'env.sh') }, JSON.stringify({ session_id: 'x', cwd: bare.path }));
      expect(result.code).toBe(0);
      expect(result.stdout).toBe('');
      expect(result.stderr).toBe('');
      expect(existsSync(join(bare.path, '.git', 'harness'))).toBe(false);
      expect(existsSync(join(bareHome, 'env.sh'))).toBe(false);
      expect(existsSync(join(bareHome, 'server.json'))).toBe(false);
      await expect(fetch(`http://127.0.0.1:${freshPort}/api/health`)).rejects.toThrow();
    } finally {
      bare.dispose();
      rmSync(bareHome, { recursive: true, force: true });
    }
  });

  it('übersteht ungültige, leere und fremde Eingaben still', () => {
    for (const input of ['{kaputt', '', '[]', 'null', JSON.stringify({ cwd: p.path }), JSON.stringify({ session_id: 5 })]) {
      const r = hook('session-start', input);
      expect(r.code, input).toBe(0);
      expect(r.stdout, input).toBe('');
      expect(r.stderr, input).toBe('');
    }
    expect(hook('quatsch', { session_id: 's', cwd: p.path }).code).toBe(0);
  });
});

TestDb.describe('harness hook mit Test-DB', () => {
  let p: TempProject;
  let home: string;
  let port: number;
  let env: Record<string, string>;
  let worktree: string;
  const hook = (kind: string, payload: object, extraEnv: Record<string, string> = {}) =>
    p.run(['hook', kind], p.path, { ...env, ...extraEnv }, JSON.stringify(payload));
  const json = (...args: string[]) => {
    const r = p.run(['--json', ...args], p.path, env);
    expect(r.code, r.stderr).toBe(0);
    return JSON.parse(r.stdout);
  };

  beforeEach(async () => {
    await TestDb.reset();
    p = new TempProject();
    home = realpathSync(mkdtempSync(join(tmpdir(), 'harness-hook-')));
    port = await freePort();
    env = { HARNESS_HOME: home, HARNESS_SERVER_ENTRY: SERVER_ENTRY, HARNESS_PORT: String(port), HARNESS_HOOK_DEADLINE_MS: '60000', HARNESS_HOOK_SERVER_WAIT_MS: '45000' };
    await new BoardConfigStore(new HarnessHome({ HARNESS_HOME: home })).write(TestDb.config()!);
    p.run(['init'], p.path, env);
    json('board', 'project', 'create', 'ARC', 'Arches');
    const board = json('board', 'boards', 'create', 'ARC', 'Sprint', '--use');
    json('board', 'boards', 'repo', String(board.id), p.path);
    json('board', 'create', 'Login bauen');
    worktree = json('board', 'start', 'ARC-0001').path;
  });
  afterEach(async () => {
    await stopServer(home);
    p.dispose();
    rmSync(home, { recursive: true, force: true });
  });

  it('ordnet im Worktree zu, liefert Kontext, speist einen Kommentar genau einmal ein und meldet das Ticket an den Server', async () => {
    const start = p.run(['hook', 'session-start'], worktree, env, JSON.stringify({ session_id: 's1', cwd: worktree }));
    expect(start.code).toBe(0);
    expect(JSON.parse(start.stdout).hookSpecificOutput.additionalContext).toContain('ARC-0001 „Login bauen“');
    const projects = await (await fetch(`http://127.0.0.1:${port}/api/projects`)).json();
    const sessions = await (await fetch(`http://127.0.0.1:${port}/api/projects/${projects[0].id}/sessions`)).json();
    expect(sessions[0]).toMatchObject({ id: 's1', tickets: ['ARC-0001'] });

    json('board', 'comment', 'ARC-0001', 'Bitte Tastatur beachten', '--author', 'Jeremias');
    const first = hook('user-prompt', { session_id: 's1', cwd: worktree, prompt: 'weiter' });
    expect(JSON.parse(first.stdout).hookSpecificOutput.additionalContext).toContain('Bitte Tastatur beachten');
    const second = hook('user-prompt', { session_id: 's1', cwd: worktree });
    expect(second.stdout).toBe('');
  });

  it('claim verknüpft die laufende Session; deren Kommentare gelten als gelesen', () => {
    json('board', 'create', 'Zweites');
    const claimed = p.run(['--json', 'board', 'claim', 'ARC-0002', '--agent', 'claude'], p.path, { ...env, HARNESS_SESSION: 's7' });
    expect(claimed.code, claimed.stderr).toBe(0);
    expect(json('board', 'show', 'ARC-0002').sessions).toEqual(['s7']);
    p.run(['board', 'comment', 'ARC-0002', 'Eigener Stand', '--author', 'claude'], p.path, { ...env, HARNESS_SESSION: 's7' });
    const ctx = hook('session-start', { session_id: 's7', cwd: p.path });
    const text = JSON.parse(ctx.stdout).hookSpecificOutput.additionalContext as string;
    expect(text).toContain('ARC-0002 „Zweites“');
    expect(text).not.toContain('Eigener Stand');
  });
});
