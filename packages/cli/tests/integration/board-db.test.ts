import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { BoardConfigStore, HarnessHome } from '@harness/core';
import { TestDb } from '../../../db/tests/helpers/TestDb.js';
import { TempProject } from './helpers/TempProject.js';

describe('harness board ohne Konfig', () => {
  it('meldet die fehlende board.yaml mit Exit 1', () => {
    const p = new TempProject();
    const home = mkdtempSync(join(tmpdir(), 'harness-home-'));
    try {
      const result = p.run(['board', 'project', 'list'], p.path, { HARNESS_HOME: home });
      expect(result.code).toBe(1);
      expect(result.stderr).toContain('board.yaml fehlt');
    } finally {
      p.dispose();
      rmSync(home, { recursive: true, force: true });
    }
  });

  it('nennt den Fehlercode: im Klartext in eckigen Klammern, mit --json als JSON-Objekt auf stdout', () => {
    const p = new TempProject();
    const home = mkdtempSync(join(tmpdir(), 'harness-home-'));
    try {
      const text = p.run(['board', 'project', 'list'], p.path, { HARNESS_HOME: home });
      expect(text.code).toBe(1);
      expect(text.stderr.trim()).toMatch(/fehlt – .* \[board-config-missing\]$/);
      const json = p.run(['--json', 'board', 'project', 'list'], p.path, { HARNESS_HOME: home });
      expect(json.code).toBe(1);
      const parsed = JSON.parse(json.stdout) as { error: { code: string; message: string } };
      expect(parsed.error.code).toBe('board-config-missing');
      expect(parsed.error.message).toContain('board.yaml fehlt');
      expect(parsed.error.message).not.toContain('[board-config-missing]');
    } finally {
      p.dispose();
      rmSync(home, { recursive: true, force: true });
    }
  });

  it('weist ein unbekanntes Wort ohne Datenbankzugriff ab', () => {
    const p = new TempProject();
    const home = mkdtempSync(join(tmpdir(), 'harness-home-'));
    try {
      const result = p.run(['board', 'lst'], p.path, { HARNESS_HOME: home });
      expect(result.code).toBe(1);
      expect(result.stderr).toContain('lst');
      expect(result.stderr).not.toContain('board.yaml');
    } finally {
      p.dispose();
      rmSync(home, { recursive: true, force: true });
    }
  });
});

TestDb.describe('harness board mit Test-DB', () => {
  let p: TempProject;
  let home: string;
  const run = (...args: string[]) => p.run(args, p.path, { HARNESS_HOME: home });
  const json = (...args: string[]) => {
    const result = run('--json', ...args);
    expect(result.code, result.stderr).toBe(0);
    return JSON.parse(result.stdout);
  };

  beforeEach(async () => {
    await TestDb.reset();
    p = new TempProject();
    home = mkdtempSync(join(tmpdir(), 'harness-home-'));
    await new BoardConfigStore(new HarnessHome({ HARNESS_HOME: home })).write(TestDb.config()!);
  });
  afterEach(() => {
    p.dispose();
    rmSync(home, { recursive: true, force: true });
  });

  // Projekt ARC mit aktivem Board „Sprint“
  function seed(): number {
    run('board', 'project', 'create', 'ARC', 'Arches');
    return json('board', 'boards', 'create', 'ARC', 'Sprint', '--use').id;
  }

  it('bildet den Ticketablauf des alten Boards ab', () => {
    seed();
    expect(run('board', 'create', 'Login bauen', '--label', 'ui', '--by', 'claude').stdout).toContain('ARC-0001');
    run('board', 'create', 'Token prüfen', '--depends-on', 'ARC-0001', '--priority', 'high');
    const dup = run('board', 'create', 'login BAUEN');
    expect(dup.code).toBe(1);
    expect(dup.stderr.trim()).toBe('Ein offenes Ticket mit diesem Titel gibt es schon: ARC-0001. Mit --force trotzdem anlegen. [ticket-duplicate]');
    expect(run('board', 'list', '--board', '99').stderr).toContain('Board 99 gibt es nicht. [board-not-found]');
    expect(run('board', 'list').stdout).toContain('Wartet (1)');
    expect(run('board', 'list', '--status', 'wartet', '--md').stdout).toContain('Wartet auf: ARC-0001');

    const noAcceptance = run('board', 'move', 'ARC-0001', 'erledigt');
    expect(noAcceptance.code).toBe(1);
    expect(noAcceptance.stderr).toContain('Abnahme');
    expect(run('board', 'move', 'ARC-0001', 'erledigt', '--acceptance', 'Test grün', '--by', 'claude').code).toBe(0);

    expect(json('board', 'show', 'ARC-0002')).toMatchObject({ dependsOpen: [], waiting: false });
    expect(run('board', 'claim', 'ARC-0002', '--agent', 'claude').code).toBe(0);
    expect(run('board', 'claim', 'ARC-0002', '--agent', 'andere').code).toBe(1);
    expect(run('board', 'comment', 'ARC-0002', 'Zwischenstand', '--author', 'claude').code).toBe(0);
    expect(run('board', 'show', 'ARC-0002').stdout).toContain('claude: Zwischenstand');
    expect(run('board', 'show', 'ARC-0001', '--field', 'acceptance').stdout.trim()).toBe('Test grün');

    const rev = json('board', 'show', 'ARC-0002').rev;
    expect(run('board', 'set', 'ARC-0002', '--title', 'Neu', '--rev', String(rev)).code).toBe(0);
    expect(run('board', 'set', 'ARC-0002', '--title', 'Zu spät', '--rev', String(rev)).code).toBe(1);
    expect(run('board', 'release', 'ARC-0002', '--status', 'planned').code).toBe(0);
    expect(run('board', 'move', 'ARC-0002', 'nicht mehr nötig').code).toBe(1);
  });

  it('plant über next-planning und plan-json', () => {
    seed();
    run('board', 'create', 'Planen');
    const next = json('board', 'next-planning', '--claim', 'claude');
    expect(next.ticket).toMatchObject({ id: 'ARC-0001', planningClaimBy: 'claude' });
    const planned = p.run(['--json', 'board', 'plan-json', 'ARC-0001', '--file', '-', '--by', 'claude'], p.path, { HARNESS_HOME: home },
      JSON.stringify({ plan: '## Ziel', handbook: '# HB', diagram: 'flowchart TD\n A-->B' }));
    expect(planned.code, planned.stderr).toBe(0);
    expect(JSON.parse(planned.stdout)).toMatchObject({ status: 'planned', needsPlanning: false, plannedBy: 'claude' });
    expect(json('board', 'next-planning')).toEqual({ ticket: null });
  });

  it('legt Projekt und Board an, schaltet um und listet', () => {
    expect(run('board', 'project', 'create', 'arc', 'Arches').code).toBe(0);
    expect(run('board', 'project', 'list').stdout).toContain('ARC  Arches');
    const board = json('board', 'boards', 'create', 'ARC', 'Sprint', '--use');
    expect(board).toMatchObject({ name: 'Sprint', projectId: expect.any(Number) });
    const listed = run('board', 'boards', 'list');
    expect(listed.stdout).toContain(`* ${board.id}  ARC  Sprint  @Test`);
    expect(run('board', 'boards', 'repo', String(board.id), 'C:\\Projects\\Arches').code).toBe(0);
    expect(run('board', 'boards', 'list').stdout).toContain('C:\\Projects\\Arches');
    expect(run('board', 'boards', 'rename', String(board.id), 'Sprint 2').code).toBe(0);
    expect(run('board', 'boards', 'list', '--mine').stdout).toContain('Sprint 2');
  });

  it('verlangt ein aktives Board für Ticketbefehle', () => {
    run('board', 'project', 'create', 'ARC', 'Arches');
    const result = run('board', 'list');
    expect(result.code).toBe(1);
    expect(result.stderr).toContain('Kein aktives Board');
  });

  it('löscht ein Board nur ohne offene Tickets oder mit --delete-open', () => {
    run('board', 'project', 'create', 'ARC', 'Arches');
    const board = json('board', 'boards', 'create', 'ARC', 'Sprint', '--use');
    run('board', 'create', 'Offen');
    const refused = run('board', 'boards', 'delete', String(board.id));
    expect(refused.code).toBe(1);
    expect(refused.stderr).toContain('ARC-0001');
    expect(run('board', 'boards', 'delete', String(board.id), '--delete-open').code).toBe(0);
    expect(run('board', 'boards', 'list').stdout).toContain('Keine Boards');
  });

  it('hält Notizen fest, macht daraus ein Ticket und verwirft nur mit Grund', () => {
    seed();
    run('board', 'create', 'Laufend');
    const note = json('board', 'note', 'add', 'Cache läuft nie ab', '--from', 'ARC-0001', '--label', 'cache', '--author', 'claude');
    expect(run('board', 'note', 'list', '--md').stdout).toContain('Cache läuft nie ab');
    expect(run('board', 'note', 'list', '--from', 'arc-1').stdout).toContain('Cache läuft nie ab');
    const taken = json('board', 'note', 'take', String(note.id), '--priority', 'high');
    expect(taken.ticket).toMatchObject({ id: 'ARC-0002', priority: 'high', labels: ['cache'] });
    expect(run('board', 'note', 'list').stdout).toContain('(keine Notizen)');
    expect(run('board', 'note', 'list', '--state', 'alle').stdout).toContain('übernommen');
    const other = json('board', 'note', 'add', 'Egal');
    expect(run('board', 'note', 'drop', String(other.id)).code).not.toBe(0);
    expect(run('board', 'note', 'drop', String(other.id), '--reason', 'erledigt sich').code).toBe(0);
    expect(run('board', 'note', 'show', String(other.id)).stdout).toContain('Grund: erledigt sich');
  });

  it('nimmt im eingerichteten Repo das Board des Repos, außerhalb das aktive Board; --board hat Vorrang', () => {
    const active = seed();
    const repoBoard = json('board', 'boards', 'create', 'ARC', 'Repo').id;
    // Noch nicht eingerichtet: aktives Board, auch wenn ein Board dem Ordner zugeordnet ist
    json('board', 'boards', 'repo', String(repoBoard), p.path);
    const before = json('board', 'create', 'Vor init');
    expect(before.boardId).toBe(active);
    expect(run('init').code).toBe(0);
    expect(json('board', 'create', 'Im Repo').boardId).toBe(repoBoard);
    expect(json('board', 'create', 'Ausdrücklich', '--board', String(active)).boardId).toBe(active);
    expect(run('board', 'list', '--md').stdout).toContain('Im Repo');
    expect(run('board', 'list', '--md').stdout).not.toContain('Vor init');
    expect(json('board', 'next-planning').ticket).toMatchObject({ title: 'Im Repo' });
    expect(json('board', 'note', 'add', 'Repo-Notiz').boardId).toBe(repoBoard);
    // Mit --from: Board des Herkunftstickets, nicht das des Repos
    expect(json('board', 'note', 'add', 'Herkunfts-Notiz', '--from', before.id).boardId).toBe(active);

    // Außerhalb eines Repos (HARNESS_HOME ist kein Git-Ordner): weiterhin das aktive Board
    const outside = p.run(['--json', 'board', 'create', 'Draußen'], home, { HARNESS_HOME: home });
    expect(outside.code, outside.stderr).toBe(0);
    expect(JSON.parse(outside.stdout).boardId).toBe(active);
    expect(p.run(['board', 'list', '--md'], home, { HARNESS_HOME: home }).stdout).toContain('Vor init');

    // Zuordnung aufgehoben (--clear): wieder das aktive Board
    json('board', 'boards', 'repo', String(repoBoard), '--clear');
    expect(json('board', 'create', 'Ohne Zuordnung').boardId).toBe(active);
  });
});
