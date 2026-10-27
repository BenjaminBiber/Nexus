import { describe, expect, it } from 'vitest';
import { InboxRanking } from '../../../src/board/inbox/InboxRanking.js';
import type { InboxItem } from '../../../src/board/inbox/InboxTypes.js';

const ranking = new InboxRanking();
const item = (key: string, kind: InboxItem['kind'], extra: Partial<InboxItem> = {}): InboxItem => ({
  key, kind, title: key, detail: null, boardId: 1, boardName: 'B', projectKey: 'AIH', ticketId: null, questionId: null, noteId: null,
  waitingTickets: 0, priority: null, since: '2026-10-19T10:00:00.000Z', rank: ranking.rank(kind), ...extra,
});

describe('InboxRanking.classify', () => {
  it.each([
    ['Merge: Konflikt in a.ts', 'merge'],
    ['Einrichtung fehlgeschlagen: npm ci', 'setup'],
    ['Human Interaction: Oberfläche ansehen', 'accept'],
    ['  Human Interaction', 'accept'],
    ['Zugang zur DB fehlt', 'blocked'],
    [null, 'blocked'],
  ] as const)('ordnet Blockiert mit Grund „%s“ als %s ein', (reason, kind) => {
    expect(ranking.classify({ status: 'blocked', blockedReason: reason })).toBe(kind);
  });

  it('nimmt Tickets in Planung als planning, andere Status gar nicht', () => {
    expect(ranking.classify({ status: 'in_planning', blockedReason: null })).toBe('planning');
    for (const status of ['backlog', 'planned', 'in_progress', 'done', 'not_needed'] as const) {
      expect(ranking.classify({ status, blockedReason: 'Merge: alt' })).toBeNull();
    }
  });

  it('erkennt Präfixe nur mit genauer Schreibung wie isHumanInteraction', () => {
    expect(ranking.classify({ status: 'blocked', blockedReason: 'merge: klein' })).toBe('blocked');
    expect(ranking.classify({ status: 'blocked', blockedReason: 'human interaction: klein' })).toBe('blocked');
  });
});

describe('InboxRanking.sort', () => {
  it('sortiert nach Art: merge, setup, question, accept, blocked, planning, note', () => {
    const kinds = ['note', 'planning', 'blocked', 'accept', 'question', 'setup', 'merge'] as const;
    const sorted = ranking.sort(kinds.map((k) => item(k, k)));
    expect(sorted.map((i) => i.kind)).toEqual([...kinds].reverse());
  });

  it('nimmt bei gleicher Art den älteren Eintrag zuerst', () => {
    const sorted = ranking.sort([
      item('b', 'blocked', { since: '2026-10-19T12:00:00.000Z' }),
      item('a', 'blocked', { since: '2026-10-18T12:00:00.000Z' }),
    ]);
    expect(sorted.map((i) => i.key)).toEqual(['a', 'b']);
  });

  it('nimmt bei Fragen die mit mehr wartenden Tickets zuerst', () => {
    const sorted = ranking.sort([
      item('q1', 'question', { waitingTickets: 1, since: '2026-10-01T00:00:00.000Z' }),
      item('q3', 'question', { waitingTickets: 3 }),
    ]);
    expect(sorted.map((i) => i.key)).toEqual(['q3', 'q1']);
  });

  it('nimmt bei Tickets in Planung die höhere Priorität zuerst', () => {
    const sorted = ranking.sort([
      item('low', 'planning', { priority: 'low', since: '2026-10-01T00:00:00.000Z' }),
      item('urgent', 'planning', { priority: 'urgent' }),
      item('normal', 'planning', { priority: 'normal' }),
    ]);
    expect(sorted.map((i) => i.key)).toEqual(['urgent', 'normal', 'low']);
  });
});
