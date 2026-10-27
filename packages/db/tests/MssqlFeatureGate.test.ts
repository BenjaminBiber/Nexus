import { describe, expect, it } from 'vitest';
import type { BoardTicket } from '@harness/core';
import { MssqlDispatchRunRepository } from '../src/MssqlDispatchRunRepository.js';
import { MssqlNoteRepository } from '../src/MssqlNoteRepository.js';
import { MssqlSearchRepository } from '../src/MssqlSearchRepository.js';
import { MssqlTicketRepository } from '../src/MssqlTicketRepository.js';
import type { SqlRunner } from '../src/SqlRunner.js';

// SqlRunner-Ersatz: merkt sich die abgesetzten Anweisungen und liefert vorgegebene Zeilen
const runner = (rows: unknown[] = []) => {
  const texts: string[] = [];
  const sql = {
    query: async (text: string) => {
      texts.push(text);
      return { rows, affected: rows.length };
    },
  } as unknown as SqlRunner;
  return { sql, texts };
};

const filter = { boardId: 1, source: null };

const ticket = (targetBranch: string | null): BoardTicket => ({
  id: 'ARC-0001', projectId: 1, boardId: 1, number: 1, title: 'T', description: '', status: 'backlog', priority: 'medium',
  labels: [], dependsOn: [], sortOrder: 0, assignee: null, needsPlanning: false, acceptance: '', plan: '', handbook: '', diagram: '',
  spec: null, specTask: null, branch: null, worktree: null, targetBranch, touches: [], consumes: [], produces: [], blockedReason: null,
  plannedAt: null, plannedBy: null, planningClaimBy: null, planningClaimAt: null, createdAt: '2026-01-01T00:00:00.000Z',
  createdById: 'a', createdByName: 'A', updatedAt: '2026-01-01T00:00:00.000Z', rev: 1,
}) as BoardTicket;

describe('Zugriffe ohne eingespielte Erweiterung', () => {
  it('verweigert Notizen mit Hinweis auf das Skript', async () => {
    const notes = new MssqlNoteRepository(runner().sql, ['note-structure'], 'AiHarness');
    for (const run of [
      () => notes.list(filter),
      () => notes.byId(1),
      () => notes.insert({ boardId: 1, authorId: 'a', authorName: 'A', title: 'T', body: 'B', source: null, labels: [], seenCount: 1, seenIn: [], state: 'open', ticketId: null, reason: null, createdAt: '2026-01-01T00:00:00.000Z' }),
      () => notes.update({ id: 1, boardId: 1, authorId: 'a', authorName: 'A', title: 'T', body: 'B', source: null, labels: [], seenCount: 1, seenIn: [], state: 'open', ticketId: null, reason: null, createdAt: '2026-01-01T00:00:00.000Z' }),
    ]) {
      await expect(run()).rejects.toMatchObject({
        code: 'board-schema-outdated',
        message: expect.stringContaining('packages/db/sql/005_note_structure.sql'),
      });
    }
  });

  it('nennt die Datenbank und verspricht, dass die übrigen Befehle weiterlaufen', async () => {
    const notes = new MssqlNoteRepository(runner().sql, ['note-structure'], 'AiHarness');
    await expect(notes.list(filter)).rejects.toMatchObject({ message: expect.stringContaining('AiHarness') });
    await expect(notes.list(filter)).rejects.toMatchObject({ message: expect.stringContaining('laufen weiter') });
  });

  it('verweigert die Läufe des Dispatchers mit Hinweis auf das Skript', async () => {
    const runs = new MssqlDispatchRunRepository(runner().sql, ['dispatch-runs'], 'AiHarness');
    for (const run of [
      () => runs.list({ projectId: 1, from: '2026-01-01T00:00:00.000Z', to: '2026-02-01T00:00:00.000Z' }),
      () => runs.firstStarted(1, 'dispatcher'),
      () => runs.existing(['x']),
    ]) {
      await expect(run()).rejects.toMatchObject({
        code: 'board-schema-outdated',
        message: expect.stringContaining('packages/db/sql/004_dispatch_runs.sql'),
      });
    }
  });

  it('verweigert die Läufe auch, wenn nur die Lauf-Details fehlen', async () => {
    // Die Abfragen lesen die Spalten aus 006 mit; ohne die Erweiterung schlägt der Zugriff sonst als SQL-Fehler fehl
    const runs = new MssqlDispatchRunRepository(runner().sql, ['dispatch-run-details'], 'AiHarness');
    await expect(runs.existing(['x'])).rejects.toMatchObject({
      code: 'board-schema-outdated',
      message: expect.stringContaining('packages/db/sql/006_dispatch_run_details.sql'),
    });
  });

  it('lässt Notizen aus der Suche weg, statt die ganze Suche scheitern zu lassen', async () => {
    const gated = runner();
    const result = await new MssqlSearchRepository(gated.sql, ['note-structure']).find({ kinds: ['ticket', 'note'], terms: ['x'], perSource: 5 });
    expect(gated.texts[0]).not.toContain('board.Notes');
    expect(gated.texts[0]).toContain('board.Tickets');
    expect(result).toEqual({ rows: [], truncated: false });
  });

  it('liest und schreibt Tickets ohne Spalte TargetBranch; nur ein gesetzter Zielbranch verlangt das Skript', async () => {
    const gated = runner();
    const tickets = new MssqlTicketRepository(gated.sql, ['target-branch'], 'AiHarness');
    expect(await tickets.list()).toEqual([]);
    expect(gated.texts[0]).toContain('CAST(NULL AS nvarchar(400)) AS TargetBranch');
    expect(gated.texts[0]).not.toContain('t.TargetBranch');
    await tickets.insert(ticket(null));
    expect(gated.texts.at(-1)).not.toContain('TargetBranch');
    await expect(tickets.insert(ticket('release/1'))).rejects.toMatchObject({
      code: 'board-schema-outdated',
      message: expect.stringContaining('packages/db/sql/007_target_branch.sql'),
    });
    await expect(tickets.update({ ...ticket('release/1'), rev: 2 })).rejects.toMatchObject({ code: 'board-schema-outdated' });
  });

  it('sucht gar nicht, wenn nur Notizen gefragt sind und die Erweiterung fehlt', async () => {
    const gated = runner();
    expect(await new MssqlSearchRepository(gated.sql, ['note-structure']).find({ kinds: ['note'], terms: ['x'], perSource: 5 })).toEqual({ rows: [], truncated: false });
    expect(gated.texts).toEqual([]);
  });
});

describe('Zugriffe mit eingespielter Erweiterung', () => {
  it('fragt Notizen und Läufe ganz normal ab', async () => {
    const notes = runner();
    expect(await new MssqlNoteRepository(notes.sql, [], 'AiHarness').list(filter)).toEqual([]);
    expect(notes.texts[0]).toContain('board.Notes');

    const runs = runner();
    expect(await new MssqlDispatchRunRepository(runs.sql, [], 'AiHarness').existing(['x'])).toEqual([]);
    expect(runs.texts[0]).toContain('board.DispatchRuns');

    const tickets = runner();
    await new MssqlTicketRepository(tickets.sql, [], 'AiHarness').list();
    expect(tickets.texts[0]).toContain('t.TargetBranch');
  });

  it('durchsucht auch die Notizen', async () => {
    const all = runner();
    await new MssqlSearchRepository(all.sql).find({ kinds: ['ticket', 'note'], terms: ['x'], perSource: 5 });
    expect(all.texts[0]).toContain('board.Notes');
  });
});
