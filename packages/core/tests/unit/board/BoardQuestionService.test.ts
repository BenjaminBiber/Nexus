import { describe, expect, it } from 'vitest';
import { BoardQuestionService } from '../../../src/board/BoardQuestionService.js';
import { BoardServices } from '../../../src/board/BoardServices.js';
import { BoardFixture } from './helpers/BoardFixture.js';

// Fixture mit zwei Tickets (ARC-0001 „Login“, ARC-0002 „Export“) auf dem Board
async function setup(): Promise<BoardFixture> {
  const f = await BoardFixture.create();
  await f.tickets.create(f.boardId, { title: 'Login' }, f.actor);
  await f.tickets.create(f.boardId, { title: 'Export' }, f.actor);
  return f;
}

describe('BoardQuestionService – anlegen und lesen', () => {
  it('legt eine Frage mit Nummer je Projekt, Herkunft, Tags, Tickets und Verlauf an', async () => {
    const f = await setup();
    const q = await f.questions.create('arc', {
      title: ' Welcher Speicher? ', description: '# Kontext', tags: ['speicher', 'Speicher', 'ui'], occasion: 'Grill', ticketIds: ['arc-2', 'ARC-0001', 'ARC-0002'],
    }, f.agent);
    expect(q).toMatchObject({
      id: 'ARC-F0001', number: 1, projectKey: 'ARC', title: 'Welcher Speicher?', description: '# Kontext', state: 'open',
      tags: ['speicher', 'ui'], occasion: 'Grill', originKind: 'session', originName: 'claude', originSession: 's-1',
      ticketIds: ['ARC-0001', 'ARC-0002'], dependsOn: [], decision: null, rev: 1,
      ready: true, researched: false, optionCount: 0, waitingOn: [], research: [], comments: [],
      createdAt: '2026-10-13T10:00:00.000Z',
      tickets: [
        { id: 'ARC-0001', title: 'Login', status: 'backlog', boardId: f.boardId },
        { id: 'ARC-0002', title: 'Export', status: 'backlog', boardId: f.boardId },
      ],
    });
    expect(q.history).toEqual([expect.objectContaining({ type: 'created', text: 'Frage angelegt', byName: 'claude', byKind: 'session' })]);
    expect((await f.questions.create('ARC', { title: 'Zweite' }, f.human)).id).toBe('ARC-F0002');
    expect((await f.questions.get('arc-f1')).title).toBe('Welcher Speicher?');
  });

  it('prüft Titel, Projekt, Tickets und Nummer, ohne Nummern zu verbrauchen', async () => {
    const f = await setup();
    await f.projects.create('OTH', 'Other', f.actor);
    const other = await f.boards.create('OTH', 'Fremd', f.actor);
    await f.tickets.create(other.id, { title: 'Fremd' }, f.actor);
    await expect(f.questions.create('ARC', { title: ' ' }, f.agent)).rejects.toMatchObject({ code: 'title-empty' });
    await expect(f.questions.create('XXX', { title: 'A' }, f.agent)).rejects.toMatchObject({ code: 'project-not-found' });
    await expect(f.questions.create('ARC', { title: 'A', ticketIds: ['ARC-0099'] }, f.agent)).rejects.toMatchObject({ code: 'question-ticket-unknown' });
    await expect(f.questions.create('ARC', { title: 'A', ticketIds: ['OTH-0001'] }, f.agent)).rejects.toMatchObject({ code: 'question-ticket-other-project' });
    await expect(f.questions.create('ARC', { title: 'x'.repeat(401) }, f.agent)).rejects.toMatchObject({ code: 'value-too-long' });
    await expect(f.questions.get('ARC-F0009')).rejects.toMatchObject({ code: 'question-not-found' });
    await expect(f.questions.get('ARC-0001')).rejects.toMatchObject({ code: 'question-id-invalid' });
    expect((await f.questions.create('ARC', { title: 'Nach Fehlern' }, f.agent)).id).toBe('ARC-F0001');
  });

  it('listet Fragen eines Projekts mit Zählern und Filtern, bereite zuerst', async () => {
    const f = await setup();
    await f.questions.create('ARC', { title: 'Alt, wartet', dependsOn: ['ARC-0001'] }, f.agent);
    f.advance(1);
    await f.questions.create('ARC', { title: 'Neuer, bereit', tags: ['ui'], ticketIds: ['ARC-0002'] }, f.agent);
    const all = await f.questions.list('ARC');
    expect(all.counts).toEqual({ open: 2, answered: 0, discarded: 0, mine: 0 });
    expect(all.questions.map((q) => [q.id, q.ready, q.waitingOn])).toEqual([
      ['ARC-F0002', true, []],
      ['ARC-F0001', false, [{ id: 'ARC-0001', state: 'backlog' }]],
    ]);
    expect((await f.questions.list('ARC', { ticket: 'arc-2' })).questions.map((q) => q.id)).toEqual(['ARC-F0002']);
    expect((await f.questions.list('ARC', { tag: 'ui' })).questions.map((q) => q.id)).toEqual(['ARC-F0002']);
    expect((await f.questions.list('ARC', { text: 'WARTET' })).questions.map((q) => q.id)).toEqual(['ARC-F0001']);
    expect((await f.questions.list('ARC', { state: 'answered' })).questions).toEqual([]);
    expect((await f.questions.list('ARC', { state: 'answered' })).counts.open).toBe(2);
  });
});

describe('BoardQuestionService – ändern und verknüpfen', () => {
  it('ändert Titel, Beschreibung, Tags und Anlass mit Verlauf und Version', async () => {
    const f = await setup();
    await f.questions.create('ARC', { title: 'Alt', description: 'Erst' }, f.agent);
    const changed = await f.questions.update('ARC-F0001', { title: 'Neu', description: 'Zweit', tags: ['a'], occasion: 'spontan' }, f.human, { rev: 1 });
    expect(changed).toMatchObject({ title: 'Neu', description: 'Zweit', tags: ['a'], occasion: 'spontan', rev: 2 });
    expect(changed.history.slice(1).map((h) => [h.type, h.text, h.byKind])).toEqual([
      ['title', 'Alt → Neu', 'human'], ['description', 'Erst', 'human'], ['tags', '(keine) → a', 'human'], ['occasion', '(keiner) → spontan', 'human'],
    ]);
    await expect(f.questions.update('ARC-F0001', { title: 'Zu spät' }, f.human, { rev: 1 })).rejects.toMatchObject({ code: 'conflict' });
    await expect(f.questions.update('ARC-F0001', {}, f.human)).rejects.toMatchObject({ code: 'nothing-changed' });
    expect((await f.questions.update('ARC-F0001', { title: 'Neu' }, f.human)).rev).toBe(2);
  });

  it('setzt betroffene Tickets und Abhängigkeiten, lehnt Unbekanntes, Selbstbezug und Kreise ab', async () => {
    const f = await setup();
    await f.questions.create('ARC', { title: 'A' }, f.agent);
    await f.questions.create('ARC', { title: 'B', dependsOn: ['ARC-F0001'] }, f.agent);
    const linked = await f.questions.setTickets('ARC-F0001', ['ARC-0002', 'arc-1'], f.agent, { rev: 1 });
    expect(linked).toMatchObject({ ticketIds: ['ARC-0001', 'ARC-0002'], rev: 2 });
    expect(linked.history.at(-1)).toMatchObject({ type: 'tickets', text: '(keine) → ARC-0001, ARC-0002' });
    const deps = await f.questions.setDepends('ARC-F0001', ['arc-2'], f.agent);
    expect(deps).toMatchObject({ dependsOn: ['ARC-0002'], waitingOn: [{ id: 'ARC-0002', state: 'backlog' }], ready: false, rev: 3 });
    expect(deps.history.at(-1)).toMatchObject({ type: 'depends', text: '(keine) → ARC-0002' });
    expect((await f.questions.get('ARC-F0002')).waitingOn).toEqual([{ id: 'ARC-F0001', state: 'open' }]);
    await expect(f.questions.setDepends('ARC-F0001', ['ARC-F0002'], f.agent)).rejects.toMatchObject({ code: 'dependency-cycle' });
    await expect(f.questions.setDepends('ARC-F0001', ['ARC-F0001'], f.agent)).rejects.toMatchObject({ code: 'dependency-self' });
    await expect(f.questions.setDepends('ARC-F0001', ['ARC-F0042'], f.agent)).rejects.toMatchObject({ code: 'dependency-unknown' });
    await expect(f.questions.setDepends('ARC-F0001', ['quatsch'], f.agent)).rejects.toMatchObject({ code: 'reference-invalid' });
    await expect(f.questions.create('ARC', { title: 'C', dependsOn: ['ARC-0042'] }, f.agent)).rejects.toMatchObject({ code: 'dependency-unknown' });
    expect((await f.questions.setTickets('ARC-F0001', [], f.agent)).ticketIds).toEqual([]);
  });
});

describe('BoardQuestionService – Kommentare und Recherchen', () => {
  it('hängt Kommentare an und erhöht die Version', async () => {
    const f = await setup();
    await f.questions.create('ARC', { title: 'A' }, f.agent);
    const q = await f.questions.comment('ARC-F0001', ' Wie wäre es mit SQL? ', f.human);
    expect(q.rev).toBe(2);
    expect(q.comments).toEqual([expect.objectContaining({ body: 'Wie wäre es mit SQL?', authorName: 'Florian', authorKind: 'human' })]);
    await expect(f.questions.comment('ARC-F0001', '  ', f.human)).rejects.toMatchObject({ code: 'comment-empty' });
  });

  it('ergänzt Recherchen mit fortlaufenden Optionen und beschreibt sie im Verlauf', async () => {
    const f = await setup();
    await f.questions.create('ARC', { title: 'A' }, f.agent);
    await f.questions.addResearch('ARC-F0001', {
      context: 'Heute: Datei',
      options: [{ title: 'Datei', text: 'bleibt' }, { title: 'DB', text: 'SQL', pros: ['geteilt'], recommended: true }],
    }, f.agent);
    const q = await f.questions.addResearch('ARC-F0001', { options: [{ title: 'Cache', text: 'im Speicher', cons: ['flüchtig'] }] }, { ...f.agent, kind: 'subagent' });
    expect(q.research.map((r) => [r.kind, r.byName, r.options.map((o) => o.id)])).toEqual([['session', 'claude', ['O1', 'O2']], ['subagent', 'claude', ['O3']]]);
    expect(q.research[0]).toMatchObject({ context: 'Heute: Datei', options: [expect.objectContaining({ pros: [] }), expect.objectContaining({ pros: ['geteilt'], recommended: true })] });
    expect(q).toMatchObject({ researched: true, optionCount: 1, rev: 3 });
    expect(q.history.filter((h) => h.type === 'research').map((h) => h.text)).toEqual(['2 Optionen (O1 bis O2), O2 empfohlen', '1 Option (O3)']);
    await expect(f.questions.addResearch('ARC-F0001', {}, f.agent)).rejects.toMatchObject({ code: 'research-invalid' });
    await expect(f.questions.addResearch('ARC-F0001', {
      options: [{ title: 'a', text: 'b', recommended: true }, { title: 'c', text: 'd', recommended: true }],
    }, f.agent)).rejects.toMatchObject({ code: 'research-recommended-multiple' });
    await expect(f.questions.addResearch('ARC-F0001', {
      options: Array.from({ length: 6 }, (_, i) => ({ title: `T${i}`, text: 'x' })),
    }, f.agent)).rejects.toMatchObject({ code: 'research-invalid' });
  });

  it('ist Teil des Dienst-Bündels', () => {
    expect(new BoardServices(new BoardFixture().db).questions).toBeInstanceOf(BoardQuestionService);
  });
});
