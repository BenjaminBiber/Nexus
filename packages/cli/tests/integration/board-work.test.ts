import { existsSync, mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { BoardConfigStore, HarnessHome } from '@harness/core';
import { TestDb } from '../../../db/tests/helpers/TestDb.js';
import { TempProject } from './helpers/TempProject.js';

describe('harness board start ohne eingerichtetes Projekt', () => {
  it('meldet ohne board.yaml die fehlende Konfig', () => {
    const p = new TempProject();
    const home = mkdtempSync(join(tmpdir(), 'harness-home-'));
    try {
      const result = p.run(['board', 'start', 'ARC-0001'], p.path, { HARNESS_HOME: home });
      expect(result.code).toBe(1);
      expect(result.stderr).toContain('board.yaml');
    } finally {
      p.dispose();
      rmSync(home, { recursive: true, force: true });
    }
  });
});

TestDb.describe('harness board start|done|step mit Test-DB', () => {
  let p: TempProject;
  let home: string;
  let boardId: number;
  const run = (args: string[], env: Record<string, string> = {}) => p.run(args, p.path, { HARNESS_HOME: home, ...env });
  const json = (args: string[], env: Record<string, string> = {}) => {
    const result = run(['--json', ...args], env);
    expect(result.code, result.stderr).toBe(0);
    return JSON.parse(result.stdout);
  };

  beforeEach(async () => {
    await TestDb.reset();
    p = new TempProject();
    home = mkdtempSync(join(tmpdir(), 'harness-home-'));
    await new BoardConfigStore(new HarnessHome({ HARNESS_HOME: home })).write(TestDb.config()!);
    expect(run(['init']).code).toBe(0);
    run(['board', 'project', 'create', 'ARC', 'Arches']);
    boardId = json(['board', 'boards', 'create', 'ARC', 'Sprint', '--use']).id;
    json(['board', 'boards', 'repo', String(boardId), p.path]);
  });
  afterEach(() => {
    p.dispose();
    rmSync(home, { recursive: true, force: true });
  });

  it('startet im Worktree, verknüpft die Session, hakt Schritte ab und schließt mit Abnahme ab', () => {
    run(['board', 'create', 'Login bauen']);
    const started = json(['board', 'start', 'ARC-0001', '--by', 'claude'], { HARNESS_SESSION: 's-42' });
    expect(started).toMatchObject({ id: 'ARC-0001', branch: 'ticket/ARC-0001-login-bauen', joined: false });
    expect(existsSync(started.path)).toBe(true);
    expect(json(['board', 'show', 'ARC-0001'])).toMatchObject({ status: 'in_progress', branch: started.branch, sessions: ['s-42'] });
    expect(run(['board', 'show', 'ARC-0001']).stdout).toContain('Branch: ticket/ARC-0001-login-bauen');

    const noStep = run(['board', 'step', 'ARC-0001', '1']);
    expect(noStep.code).toBe(1);
    expect(noStep.stderr).toContain('keine Schritte');
    expect(run(['board', 'step', 'ARC-0001', 'x']).stderr).toContain('Schrittnummer');

    const missing = run(['board', 'done', 'ARC-0001']);
    expect(missing.code).toBe(1);
    expect(missing.stderr).toContain('Abnahme');
    const done = json(['board', 'done', 'ARC-0001', '--acceptance', 'Test grün']);
    expect(done).toMatchObject({ id: 'ARC-0001', readyToMerge: true });
    expect(json(['board', 'show', 'ARC-0001']).status).toBe('done');
  });

  it('legt Schritte mit create --step und step add an, bearbeitet, verschiebt, löscht und hakt sie ab', () => {
    run(['board', 'create', 'Export', '--step', 'Test schreiben', '--step', 'Umsetzen']);
    json(['board', 'step', 'add', 'ARC-0001', 'Committen', '--detail', 'git commit', '--by', 'claude']);
    json(['board', 'step', 'edit', 'ARC-0001', '1', '--text', 'Test zuerst']);
    json(['board', 'step', 'move', 'ARC-0001', '3', '1']);
    json(['board', 'step', 'rm', 'ARC-0001', '3']);
    json(['board', 'step', 'ARC-0001', '1']);
    const detail = json(['board', 'step', 'done', 'ARC-0001', '2']);
    expect(detail.steps).toEqual([{ text: 'Committen', done: true, detail: 'git commit' }, { text: 'Test zuerst', done: true }]);
    expect(run(['board', 'show', 'ARC-0001']).stdout).toContain('Schritte (2/2)');
  });

  it('verweigert den Start bei offener Voraussetzung ohne --force', () => {
    run(['board', 'create', 'Basis']);
    run(['board', 'create', 'Folge', '--depends-on', 'ARC-0001']);
    const blocked = run(['board', 'start', 'ARC-0002']);
    expect(blocked.code).toBe(1);
    expect(blocked.stderr).toContain('ARC-0001');
    expect(json(['board', 'start', 'ARC-0002', '--force']).joined).toBe(false);
  });

  it('lehnt start und done für Tickets eines anderen Boards ab und nennt beide Boards', () => {
    const other = json(['board', 'boards', 'create', 'ARC', 'Fremd']).id;
    run(['board', 'create', 'Fremdes Ticket', '--board', String(other), '--acceptance', 'ok']);
    const start = run(['board', 'start', 'ARC-0001']);
    expect(start.code).toBe(1);
    expect(start.stderr).toContain(`Board ${other}`);
    expect(start.stderr).toContain(`Board ${boardId}`);
    expect(start.stderr).toContain('harness board boards repo');
    expect(p.git('branch', '--list', 'ticket/*')).toBe('');
    const done = run(['board', 'done', 'ARC-0001']);
    expect(done.code).toBe(1);
    expect(done.stderr).toContain('gehört aber zu Board');
  });

  it('verlangt für start ein dem Repo zugeordnetes Board', () => {
    run(['board', 'create', 'Login bauen']);
    // Nur --clear hebt die Zuordnung auf; ohne Pfad zeigt der Befehl sie nur an
    json(['board', 'boards', 'repo', String(boardId), '--clear']);
    const start = run(['board', 'start', 'ARC-0001']);
    expect(start.code).toBe(1);
    expect(start.stderr).toContain('harness board boards repo');
  });
});
