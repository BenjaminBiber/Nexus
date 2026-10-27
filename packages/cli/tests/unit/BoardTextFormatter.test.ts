import { describe, expect, it } from 'vitest';
import type { BoardTicketDetail } from '@harness/core';
import { BoardTextFormatter } from '../../src/board/BoardTextFormatter.js';

// Ein Ticket mit einem Mensch-Kommentar zu Beginn, fünf Agenten-Kommentaren und zehn Verlaufseinträgen
function detail(overrides: Partial<BoardTicketDetail> = {}): BoardTicketDetail {
  const comments = [
    { createdAt: '2026-01-01', authorName: 'Florian', body: 'Bitte kompakt halten' },
    ...Array.from({ length: 5 }, (_, i) => ({ createdAt: `2026-01-0${i + 2}`, authorName: 'junior-AIH-0070', body: `Bericht ${i + 1}` })),
  ];
  const history = Array.from({ length: 10 }, (_, i) => ({ at: `2026-01-0${i}`, byName: 'claude', field: 'status', from: 'planned', to: 'in_progress', note: null }));
  return {
    id: 'AIH-0070', title: 'Kompakte Ausgabe', status: 'in_progress', priority: 'high', rev: 7, assignee: 'junior-AIH-0070',
    needsPlanning: false, labels: [], waiting: false, blockedReason: null, acceptance: '', dependsOn: [], dependsOpen: [],
    dependents: [], waitingOnQuestions: [], spec: null, specTask: null, branch: null, worktree: null, touches: [], consumes: [],
    produces: [], sessions: [], description: '', plan: '', handbook: '', diagram: '', steps: [], stepsDone: 0, stepsTotal: 0,
    comments, history, ...overrides,
  } as unknown as BoardTicketDetail;
}
import { BoardTextFormatter } from '../../src/board/BoardTextFormatter.js';
import { BoardFixture } from '../../../core/tests/unit/board/helpers/BoardFixture.js';

describe('BoardTextFormatter.showText', () => {
  const format = new BoardTextFormatter();

  it('lässt ohne Optionen den Verlauf und ältere Agenten-Kommentare weg', () => {
    const lines = format.showText(detail());
    expect(lines).not.toContain('## Verlauf');
    expect(lines).toContain('Verlauf: 10 Einträge – --history zeigt sie');
    expect(lines).toContain('  2 ältere Agenten-Kommentare ausgeblendet – --all-comments zeigt sie');
    expect(lines).toContain('  2026-01-01 Florian: Bitte kompakt halten');
    expect(lines.join('\n')).not.toContain('Bericht 1');
    expect(lines.join('\n')).not.toContain('Bericht 2');
    for (const n of [3, 4, 5]) expect(lines).toContain(`  2026-01-0${n + 1} junior-AIH-0070: Bericht ${n}`);
  });

  it('gibt mit history und allComments alles aus', () => {
    const lines = format.showText(detail(), { history: true, allComments: true });
    expect(lines).toContain('## Verlauf');
    expect(lines.filter((l) => l.includes('status: planned → in_progress'))).toHaveLength(10);
    expect(lines.join('\n')).toContain('Bericht 1');
    expect(lines.join('\n')).not.toContain('ausgeblendet');
    expect(lines.join('\n')).not.toContain('--history zeigt sie');
  });

  it('zeigt ohne Kommentare und Verlauf keine Hinweiszeilen', () => {
    const lines = format.showText(detail({ comments: [], history: [] } as Partial<BoardTicketDetail>));
    expect(lines).toContain('(keine)');
    expect(lines.join('\n')).not.toContain('ausgeblendet');
    expect(lines.join('\n')).not.toContain('Verlauf:');
  });

  it('blendet nichts aus, wenn es höchstens drei Agenten-Kommentare gibt', () => {
    const comments = [
      { createdAt: '2026-01-01', authorName: 'Florian', body: 'Hallo' },
      { createdAt: '2026-01-02', authorName: 'claude', body: 'A' },
      { createdAt: '2026-01-03', authorName: 'dispatcher', body: 'B' },
    ];
    const lines = format.showText(detail({ comments } as Partial<BoardTicketDetail>));
    expect(lines.join('\n')).not.toContain('ausgeblendet');
    expect(lines.join('\n')).toContain('A');
  });

  it('zeigt den Zielbranch vor dem Branch des Tickets', async () => {
    const f = await BoardFixture.create();
    await f.tickets.create(f.boardId, { title: 'A', targetBranch: 'release/1' }, f.actor);
    await f.tickets.update('ARC-0001', { branch: 'ticket/ARC-0001-a', worktree: 'C:/w/ARC-0001' }, f.actor);
    const lines = format.showText(await f.tickets.get('ARC-0001'));
    const target = lines.indexOf('Zielbranch: release/1');
    expect(target).toBeGreaterThan(-1);
    expect(lines[target + 1]).toBe('Branch: ticket/ARC-0001-a (Worktree C:/w/ARC-0001)');
  });

  it('zeigt den Zielbranch auch ohne Branch und lässt die Zeile ohne Zielbranch weg', async () => {
    const f = await BoardFixture.create();
    await f.tickets.create(f.boardId, { title: 'A', targetBranch: 'release/1' }, f.actor);
    await f.tickets.create(f.boardId, { title: 'B' }, f.actor);
    expect(format.showText(await f.tickets.get('ARC-0001'))).toContain('Zielbranch: release/1');
    expect(format.showText(await f.tickets.get('ARC-0002')).some((l) => l.startsWith('Zielbranch'))).toBe(false);
  });
});
