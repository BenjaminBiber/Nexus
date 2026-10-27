import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { BoardConfigStore, HarnessHome } from '@harness/core';
import { TestDb } from '../../../db/tests/helpers/TestDb.js';
import { TempProject } from './helpers/TempProject.js';

describe('harness board question ohne Datenbank', () => {
  let p: TempProject;
  let home: string;

  beforeEach(() => {
    p = new TempProject();
    home = mkdtempSync(join(tmpdir(), 'harness-home-'));
  });
  afterEach(() => {
    p.dispose();
    rmSync(home, { recursive: true, force: true });
  });

  it('zeigt die Unterbefehle in der Hilfe', () => {
    const result = p.run(['board', 'question', '--help'], p.path, { HARNESS_HOME: home });
    expect(result.code).toBe(0);
    for (const word of ['add', 'list', 'show', 'research', 'answer', 'discard', 'reopen', 'link', 'depends', 'comment']) {
      expect(result.stdout, word).toContain(word);
    }
  });

  it('verweigert das Entscheiden innerhalb einer Session ohne Datenbankzugriff', () => {
    for (const args of [
      ['board', 'question', 'answer', 'ARC-F0001', '--text', 'x', '--reason', 'y'],
      ['board', 'question', 'discard', 'ARC-F0001', '--reason', 'y'],
      ['board', 'question', 'reopen', 'ARC-F0001'],
    ]) {
      const result = p.run(args, p.path, { HARNESS_HOME: home, HARNESS_SESSION: 's-1' });
      expect(result.code, args.join(' ')).toBe(1);
      expect(result.stderr).toContain('trifft ein Mensch');
      expect(result.stderr).not.toContain('board.yaml');
    }
  });
});

TestDb.describe('harness board question mit Test-DB', () => {
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

  // Projekt ARC mit aktivem Board „Sprint“ und Ticket ARC-0001 (mit Abnahme)
  function seed(): void {
    run('board', 'project', 'create', 'ARC', 'Arches');
    json('board', 'boards', 'create', 'ARC', 'Sprint', '--use');
    run('board', 'create', 'Login bauen', '--acceptance', 'geht');
  }

  it('fragt, recherchiert, sperrt das Ticket und gibt es nach der Entscheidung frei', () => {
    seed();
    const created = json('board', 'question', 'add', 'Welcher Speicher?', '--ticket', 'ARC-0001', '--tag', 'speicher', '--kind', 'session', '--author', 'claude', '--session', 's-1');
    expect(created).toMatchObject({ id: 'ARC-F0001', ticketIds: ['ARC-0001'], originKind: 'session', originName: 'claude', originSession: 's-1' });
    expect(run('board', 'list').stdout).toContain('[Frage ARC-F0001]');
    const blocked = run('board', 'claim', 'ARC-0001', '--agent', 'claude');
    expect(blocked.code).toBe(1);
    expect(blocked.stderr).toContain('wartet auf Frage ARC-F0001');
    // Den --force-Hinweis ergänzt die CLI (die Kernmeldung ist neutral)
    expect(blocked.stderr).toContain('Mit --force übergehen.');
    const researched = p.run(
      ['board', 'question', 'research', 'ARC-F0001', '--from', '-', '--kind', 'session', '--author', 'claude'], p.path, { HARNESS_HOME: home },
      JSON.stringify({ context: 'Heute Datei', options: [{ title: 'Datei', text: 'JSON' }, { title: 'DB', text: 'SQL', recommended: true }] }),
    );
    expect(researched.code, researched.stderr).toBe(0);
    expect(researched.stdout).toContain('2 Optionen (O1 bis O2), O2 empfohlen');
    expect(run('board', 'question', 'list').stdout).toContain('ARC-F0001');
    expect(run('board', 'question', 'list', '--md').stdout).toContain('Betrifft: ARC-0001');
    const shown = run('board', 'question', 'show', 'ARC-F0001').stdout;
    expect(shown).toContain('O2 (empfohlen): DB');
    expect(shown).toContain('Herkunft: claude (Session), Session s-1');
    const inSession = p.run(['board', 'question', 'answer', 'ARC-F0001', '--option', 'O2', '--reason', 'geteilt'], p.path, { HARNESS_HOME: home, HARNESS_SESSION: 's-1' });
    expect(inSession.code).toBe(1);
    expect(run('board', 'question', 'answer', 'ARC-F0001', '--option', 'O2', '--reason', 'geteilt').code).toBe(0);
    expect(json('board', 'show', 'ARC-0001').waitingOnQuestions).toEqual([]);
    expect(run('board', 'claim', 'ARC-0001', '--agent', 'claude').code).toBe(0);
    expect(json('board', 'question', 'list', '--state', 'beantwortet').questions.map((q: { id: string }) => q.id)).toEqual(['ARC-F0001']);
  });

  it('verknüpft Tickets und Abhängigkeiten, kommentiert, verwirft und öffnet wieder', () => {
    seed();
    run('board', 'create', 'Export');
    run('board', 'question', 'add', 'Erste');
    run('board', 'question', 'add', 'Zweite', '--depends', 'ARC-F0001');
    expect(run('board', 'question', 'link', 'ARC-F0002', '--add', 'ARC-0001', '--add', 'ARC-0002').stdout).toContain('ARC-0001, ARC-0002');
    expect(run('board', 'question', 'link', 'ARC-F0002', '--remove', 'ARC-0001').stdout).toContain('Betroffene Tickets von ARC-F0002: ARC-0002');
    const cycle = run('board', 'question', 'depends', 'ARC-F0001', '--add', 'ARC-F0002');
    expect(cycle.code).toBe(1);
    expect(cycle.stderr).toContain('Kreis');
    expect(run('board', 'question', 'comment', 'ARC-F0002', 'Bitte bis Freitag', '--author', 'claude').code).toBe(0);
    expect(run('board', 'question', 'show', 'ARC-F0002').stdout).toContain('Bitte bis Freitag');
    expect(run('board', 'question', 'discard', 'ARC-F0002', '--reason', 'doppelt').code).toBe(0);
    expect(run('board', 'question', 'reopen', 'ARC-F0002', '--reason', 'doch nötig').code).toBe(0);
    expect(json('board', 'question', 'show', 'ARC-F0002')).toMatchObject({
      state: 'open', ticketIds: ['ARC-0002'], comments: [expect.objectContaining({ body: 'Bitte bis Freitag' })],
    });
  });
});
