import { afterEach, describe, expect, it, vi } from 'vitest';
import { ApiClient } from '../src/api/ApiClient.js';
import { filterDbQuestions, historyEntries, optionTitle, researchDtos } from '../src/dbquestion/questionDtos.js';
import { NO_FILTER } from '../src/question/filterQuestions.js';
import { dbQuestion } from './helpers/fakeApi.js';

afterEach(() => vi.unstubAllGlobals());

// Ersetzt fetch und merkt sich Methode, Adresse und Body jedes Aufrufs
function stubFetch() {
  const calls: Array<{ url: string; method: string; body: unknown; headers: Record<string, string> }> = [];
  vi.stubGlobal('fetch', vi.fn(async (url: string, init: RequestInit) => {
    calls.push({ url, method: init.method ?? 'GET', body: init.body ? JSON.parse(init.body as string) : undefined, headers: (init.headers ?? {}) as Record<string, string> });
    return new Response('{}', { status: 200 });
  }));
  return calls;
}

describe('ApiClient für Fragen der Board-Datenbank', () => {
  it('spricht die Routen unter /api/db an', async () => {
    const calls = stubFetch();
    const api = new ApiClient();
    await api.getDbQuestions('ARC');
    await api.getDbQuestion('ARC-F0001');
    await api.createDbQuestion('ARC', { title: 'A', ticketIds: ['ARC-0001'] });
    await api.patchDbQuestion('ARC-F0001', { rev: 2, title: 'B' });
    await api.putDbQuestionTickets('ARC-F0001', 3, ['ARC-0001']);
    await api.putDbQuestionDepends('ARC-F0001', 4, ['ARC-F0002']);
    await api.addDbQuestionComment('ARC-F0001', 'Hallo');
    await api.decideDbQuestion('ARC-F0001', { rev: 5, option: 'O1', reason: 'passt' });
    await api.discardDbQuestion('ARC-F0001', 6, 'doppelt');
    await api.reopenDbQuestion('ARC-F0001', 7);
    await api.reopenDbQuestion('ARC-F0001', 8, 'neu');
    expect(calls.map((c) => `${c.method} ${c.url}`)).toEqual([
      'GET /api/db/projects/ARC/questions', 'GET /api/db/questions/ARC-F0001', 'POST /api/db/projects/ARC/questions',
      'PATCH /api/db/questions/ARC-F0001', 'PUT /api/db/questions/ARC-F0001/tickets', 'PUT /api/db/questions/ARC-F0001/depends',
      'POST /api/db/questions/ARC-F0001/comments', 'POST /api/db/questions/ARC-F0001/decision', 'POST /api/db/questions/ARC-F0001/discard',
      'POST /api/db/questions/ARC-F0001/reopen', 'POST /api/db/questions/ARC-F0001/reopen',
    ]);
    expect(calls[2].body).toEqual({ title: 'A', ticketIds: ['ARC-0001'] });
    expect(calls[2].headers['x-harness']).toBe('1');
    expect(calls[3].body).toEqual({ rev: 2, title: 'B' });
    expect(calls[4].body).toEqual({ rev: 3, ticketIds: ['ARC-0001'] });
    expect(calls[5].body).toEqual({ rev: 4, dependsOn: ['ARC-F0002'] });
    expect(calls[6].body).toEqual({ body: 'Hallo' });
    expect(calls[7].body).toEqual({ rev: 5, option: 'O1', reason: 'passt' });
    expect(calls[8].body).toEqual({ rev: 6, reason: 'doppelt' });
    expect(calls[9].body).toEqual({ rev: 7 });
    expect(calls[10].body).toEqual({ rev: 8, reason: 'neu' });
  });

  it('hängt Board und Projekt an den Ereignisstrom', () => {
    const urls: string[] = [];
    vi.stubGlobal('EventSource', class {
      constructor(url: string) {
        urls.push(url);
      }
    });
    const api = new ApiClient();
    api.openEvents('p1', null, 3);
    api.openEvents('p1', 4, 3);
    api.openEvents('p1', 4);
    expect(urls).toEqual(['/api/projects/p1/events?dbProject=3', '/api/projects/p1/events?dbBoard=4&dbProject=3', '/api/projects/p1/events?dbBoard=4']);
  });
});

describe('Abbildung der DB-Fragen auf die Fragen-Bausteine', () => {
  const research = [
    { id: 1, questionId: 'ARC-F0001', at: '2026-10-14T10:00:00.000Z', byId: 'u1', byName: 'claude', kind: 'session' as const, context: 'K', options: [{ id: 'O1', title: 'Datei', text: 't', pros: [], cons: [], recommended: false }] },
    { id: 2, questionId: 'ARC-F0001', at: '2026-10-14T11:00:00.000Z', byId: 'u1', byName: 'claude', kind: 'subagent' as const, context: '', options: [{ id: 'O2', title: 'DB', text: 't', pros: [], cons: [], recommended: true }] },
  ];

  it('übersetzt Recherchen und findet Optionstitel, neueste zuerst', () => {
    expect(researchDtos(research)).toEqual([
      { at: '2026-10-14T10:00:00.000Z', author: 'claude', kind: 'session', context: 'K', options: research[0].options },
      { at: '2026-10-14T11:00:00.000Z', author: 'claude', kind: 'subagent', context: '', options: research[1].options },
    ]);
    expect(optionTitle({ research }, 'O1')).toBe('Datei');
    expect(optionTitle({ research }, 'O9')).toBeNull();
  });

  it('mischt Verlauf und Kommentare zeitlich; Systemeinträge gelten als Mensch', () => {
    const entries = historyEntries({
      history: [
        { questionId: 'ARC-F0001', at: '2026-10-14T10:00:00.000Z', byId: 'u1', byName: 'claude', byKind: 'session', type: 'created', text: 'Frage angelegt' },
        { questionId: 'ARC-F0001', at: '2026-10-14T12:00:00.000Z', byId: 'u1', byName: 'Florian', byKind: null, type: 'ticket-deleted', text: 'weg' },
      ],
      comments: [{ id: 1, questionId: 'ARC-F0001', authorId: 'u1', authorName: 'Florian', authorKind: 'human', body: 'Hallo', createdAt: '2026-10-14T11:00:00.000Z' }],
    });
    expect(entries.map((e) => [e.type, e.author, e.kind, e.text])).toEqual([
      ['created', 'claude', 'session', 'Frage angelegt'], ['comment', 'Florian', 'human', 'Hallo'], ['ticket-deleted', 'Florian', 'human', 'weg'],
    ]);
    expect(entries.every((e) => e.read === undefined)).toBe(true);
  });

  it('filtert nach Zustand und Suchtext', () => {
    const list = [dbQuestion('ARC-F0001', 'Speicher', { tags: ['ui'] }), dbQuestion('ARC-F0002', 'Export', { state: 'answered', ticketIds: ['ARC-0007'] })];
    expect(filterDbQuestions(list, 'open', { ...NO_FILTER, text: '' }).map((q) => q.id)).toEqual(['ARC-F0001']);
    expect(filterDbQuestions(list, 'all', { ...NO_FILTER, text: 'arc-0007' }).map((q) => q.id)).toEqual(['ARC-F0002']);
    expect(filterDbQuestions(list, 'all', { ...NO_FILTER, text: ' UI ' }).map((q) => q.id)).toEqual(['ARC-F0001']);
  });
});
