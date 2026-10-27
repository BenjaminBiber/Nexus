import { describe, expect, it } from 'vitest';
import { BoardFixture } from './helpers/BoardFixture.js';
import type { BoardDatabase } from '../../../src/board/BoardDatabase.js';
import { NoteService } from '../../../src/board/NoteService.js';

describe('NoteService', () => {
  it('legt Notizen an und listet standardmäßig nur offene', async () => {
    const f = await BoardFixture.create();
    await f.tickets.create(f.boardId, { title: 'A' }, f.actor);
    const n = await f.notes.add(f.boardId, { title: 'Cache läuft nie ab' }, f.actor, { source: 'arc-1', labels: ['cache'] });
    expect(n).toMatchObject({ state: 'open', source: 'ARC-0001', labels: ['cache'], authorName: 'claude' });
    await f.notes.add(f.boardId, { title: 'Zweite' }, f.actor);
    await f.notes.drop((await f.notes.list(f.boardId))[1].id, 'erledigt sich');
    expect((await f.notes.list(f.boardId)).map((x) => x.title)).toEqual(['Cache läuft nie ab']);
    expect(await f.notes.list(f.boardId, { states: ['open', 'taken', 'dropped'] })).toHaveLength(2);
  });

  it('lehnt eine unbekannte Herkunft, einen leeren, mehrzeiligen oder zu langen Titel ab', async () => {
    const f = await BoardFixture.create();
    await expect(f.notes.add(f.boardId, { title: 'x' }, f.actor, { source: 'ARC-0009' })).rejects.toMatchObject({ code: 'ticket-not-found' });
    await expect(f.notes.add(f.boardId, { title: ' ', body: 'nur Details' }, f.actor)).rejects.toMatchObject({ code: 'note-empty' });
    await expect(f.notes.add(f.boardId, { title: 'Zeile eins\nZeile zwei' }, f.actor)).rejects.toMatchObject({ code: 'note-title-multiline' });
    await expect(f.notes.add(f.boardId, { title: 't'.repeat(201) }, f.actor)).rejects.toMatchObject({ code: 'value-too-long', message: expect.stringContaining('--details') });
  });

  it('speichert Titel und Details getrennt, getrimmt', async () => {
    const f = await BoardFixture.create();
    const n = await f.notes.add(f.boardId, { title: '  Cache läuft nie ab ', body: '\nBoardViewService ohne TTL\n' }, f.actor);
    expect(n).toMatchObject({ title: 'Cache läuft nie ab', body: 'BoardViewService ohne TTL', seenCount: 1, seenIn: [] });
    expect(await f.notes.get(n.id)).toMatchObject({ title: 'Cache läuft nie ab', body: 'BoardViewService ohne TTL' });
  });

  it('lehnt denselben Befund ab und nennt offene und übernommene Kandidaten; force legt trotzdem an', async () => {
    const f = await BoardFixture.create();
    await f.tickets.create(f.boardId, { title: 'A' }, f.actor);
    const first = await f.notes.add(f.boardId, { title: 'Core-Unit-Tests scheitern am Windows-Kurzpfad im TEMP', body: 'GitPaths, LongPaths: BENJAM~1.BIB statt Langname' }, f.actor);
    const again = { title: 'npm run test:unit: GitPaths und LongPaths rot', body: 'Windows-Kurzpfad BENJAM~1.BIB im TEMP, unabhängig von ARC-0001' };
    await expect(f.notes.add(f.boardId, again, f.actor, { source: 'ARC-0001' })).rejects.toMatchObject({
      code: 'note-similar',
      message: expect.stringContaining(`harness board note seen ${first.id} --from ARC-0001`),
      details: { notes: [{ id: first.id, title: first.title, state: 'open', ticketId: null }] },
    });
    const { ticket } = await f.notes.take(first.id, f.actor);
    await expect(f.notes.add(f.boardId, again, f.actor)).rejects.toMatchObject({ code: 'note-similar', message: expect.stringContaining(`an ${ticket.id} kommentieren`) });
    expect(await f.notes.add(f.boardId, again, f.actor, { force: true })).toMatchObject({ state: 'open', seenCount: 1 });
    // Etwas anderes geht ohne force durch
    expect(await f.notes.add(f.boardId, { title: 'Cache der Board-Ansicht läuft nie ab' }, f.actor)).toMatchObject({ state: 'open' });
  });

  it('zählt einen erneut gesehenen Befund hoch und merkt sich jedes Ticket einmal', async () => {
    const f = await BoardFixture.create();
    for (const title of ['A', 'B', 'C']) await f.tickets.create(f.boardId, { title }, f.actor);
    const n = await f.notes.add(f.boardId, { title: 'Befund' }, f.actor, { source: 'ARC-0001' });
    await f.notes.seen(n.id, { source: 'ARC-0002' });
    await f.notes.seen(n.id, { source: 'arc-2' });
    await f.notes.seen(n.id, { source: 'ARC-0001' });
    const seen = await f.notes.seen(n.id);
    expect(seen).toMatchObject({ seenCount: 5, seenIn: ['ARC-0002'] });
    expect(await f.notes.get(n.id)).toMatchObject({ seenCount: 5, seenIn: ['ARC-0002'] });
    const { ticket } = await f.notes.take(n.id, f.actor);
    expect(ticket.description).toBe('(Notiz vom 2026-10-13T10:00:00.000Z, aufgefallen bei ARC-0001; insgesamt 5× aufgefallen, auch bei ARC-0002)');
    await expect(f.notes.seen(n.id)).rejects.toMatchObject({ code: 'note-taken', message: expect.stringContaining(ticket.id) });
    await expect(f.notes.seen(999)).rejects.toMatchObject({ code: 'note-not-found' });
  });

  it('sucht ähnliche offene und übernommene Notizen, ähnlichste zuerst', async () => {
    const f = await BoardFixture.create();
    const a = await f.notes.add(f.boardId, { title: 'Dispatcher startet Worktree doppelt', body: 'WorktreeManager legt Branch zweimal an' }, f.actor);
    await f.notes.add(f.boardId, { title: 'Cache der Board-Ansicht läuft nie ab' }, f.actor);
    const dropped = await f.notes.add(f.boardId, { title: 'Worktree Branch doppelt angelegt vom Dispatcher' }, f.actor, { force: true });
    await f.notes.drop(dropped.id, 'doppelt');
    expect((await f.notes.similar(f.boardId, 'WorktreeManager: Branch doppelt, Dispatcher')).map((n) => n.id)).toEqual([a.id]);
  });

  it('hält eine kurze Notiz nicht für Dublette einer langen, nur weil drei Begriffe des Bereichs vorkommen', async () => {
    const f = await BoardFixture.create();
    await f.notes.add(f.boardId, {
      title: 'Dispatcher räumt Worktree nach Abbruch nicht auf',
      body: 'Nach einem abgebrochenen Lauf bleibt der Branch samt Ordner liegen; WorktreeManager prüft beim nächsten Start nur '
        + 'den Pfad, nicht den Branch. Folge: zweiter Lauf scheitert mit „already exists“, Aufräumen per Hand nötig.',
    }, f.actor);
    expect(await f.notes.similar(f.boardId, 'Dispatcher zeigt Branch des Worktree falsch an')).toEqual([]);
    expect(await f.notes.add(f.boardId, { title: 'Dispatcher zeigt Branch des Worktree falsch an' }, f.actor)).toMatchObject({ state: 'open' });
  });

  it('lehnt eine Herkunft aus einem anderen Projekt ab', async () => {
    const f = await BoardFixture.create();
    await f.projects.create('AIH', 'Harness', f.actor);
    const aih = await f.boards.create('AIH', 'Harness', f.actor);
    await f.tickets.create(aih.id, { title: 'A' }, f.actor);
    await expect(f.notes.add(f.boardId, { title: 'x' }, f.actor, { source: 'AIH-0001' })).rejects.toMatchObject({ code: 'note-source-other-project' });
    expect(await f.notes.add(aih.id, { title: 'x' }, f.actor, { source: 'AIH-0001' })).toMatchObject({ boardId: aih.id, source: 'AIH-0001' });
  });

  it('macht aus einer Notiz ein Ticket mit Herkunftsvermerk', async () => {
    const f = await BoardFixture.create();
    await f.tickets.create(f.boardId, { title: 'A' }, f.actor);
    const n = await f.notes.add(f.boardId, { title: 'Cache prüfen', body: 'Details' }, f.actor, { source: 'ARC-0001', labels: ['cache'] });
    const { note, ticket } = await f.notes.take(n.id, f.actor, { priority: 'high' });
    expect(ticket).toMatchObject({ id: 'ARC-0002', title: 'Cache prüfen', priority: 'high', labels: ['cache'], status: 'backlog' });
    expect(ticket.description).toBe('Details\n\n(Notiz vom 2026-10-13T10:00:00.000Z, aufgefallen bei ARC-0001)');
    expect(note).toMatchObject({ state: 'taken', ticketId: 'ARC-0002' });
    await expect(f.notes.take(n.id, f.actor)).rejects.toMatchObject({ code: 'note-not-open', message: expect.stringContaining('ARC-0002') });
  });

  it('übernimmt eine Notiz auch, wenn ein offenes Ticket denselben Titel hat', async () => {
    const f = await BoardFixture.create();
    await f.tickets.create(f.boardId, { title: 'Cache prüfen' }, f.actor);
    const n = await f.notes.add(f.boardId, { title: 'Cache prüfen' }, f.actor);
    expect((await f.notes.take(n.id, f.actor)).ticket).toMatchObject({ id: 'ARC-0002', title: 'Cache prüfen' });
  });

  it('verwirft nur mit Grund und nicht nach der Übernahme', async () => {
    const f = await BoardFixture.create();
    const n = await f.notes.add(f.boardId, { title: 'x' }, f.actor);
    await expect(f.notes.drop(n.id, ' ')).rejects.toMatchObject({ code: 'reason-missing' });
    await f.notes.take(n.id, f.actor);
    await expect(f.notes.drop(n.id, 'egal')).rejects.toMatchObject({ code: 'note-taken' });
    await expect(f.notes.get(999)).rejects.toMatchObject({ code: 'note-not-found' });
  });

  it('sperrt die Notiz beim Übernehmen und Verwerfen (forUpdate)', async () => {
    const f = await BoardFixture.create();
    const a = await f.notes.add(f.boardId, { title: 'A' }, f.actor);
    const b = await f.notes.add(f.boardId, { title: 'B' }, f.actor);
    const locked: number[] = [];
    const spying: BoardDatabase = {
      close: () => f.db.close(),
      changeMarker: (boardId) => f.db.changeMarker(boardId),
      questionMarker: (projectId) => f.db.questionMarker(projectId),
      transaction: (work) => f.db.transaction((u) => work({
        ...u,
        notes: { ...u.notes, byId: (id, forUpdate) => { if (forUpdate) locked.push(id); return u.notes.byId(id, forUpdate); } },
      })),
    };
    const notes = new NoteService(spying, f.clock);
    await notes.get(a.id);
    expect(locked).toEqual([]);
    await notes.take(a.id, f.actor);
    await notes.drop(b.id, 'egal');
    expect(locked).toEqual([a.id, b.id]);
  });

  it('entfernt doppelte Labels ohne Rücksicht auf Groß-/Kleinschreibung und prüft Längen', async () => {
    const f = await BoardFixture.create();
    const n = await f.notes.add(f.boardId, { title: 'A' }, f.actor, { labels: ['Cache', 'cache, UI', 'ui'] });
    expect(n.labels).toEqual(['Cache', 'UI']);
    await expect(f.notes.add(f.boardId, { title: 'B' }, f.actor, { labels: ['l'.repeat(101)] })).rejects.toMatchObject({ code: 'value-too-long' });
    const many = Array.from({ length: 12 }, (_, i) => `${i}`.padEnd(90, 'x'));
    await expect(f.notes.add(f.boardId, { title: 'C' }, f.actor, { labels: many })).rejects.toMatchObject({ code: 'value-too-long', message: expect.stringContaining('Label-Liste') });
    await expect(f.notes.add(f.boardId, { title: 'D' }, { ...f.actor, name: 'n'.repeat(101) })).rejects.toMatchObject({ code: 'value-too-long' });
    await expect(f.notes.drop(n.id, 'g'.repeat(1001))).rejects.toMatchObject({ code: 'value-too-long' });
  });
});
