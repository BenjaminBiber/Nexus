import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import type { BoardDatabase } from '../../../../src/board/BoardDatabase.js';
import { BoardSearchService } from '../../../../src/board/BoardSearchService.js';
import { BoardServices } from '../../../../src/board/BoardServices.js';
import type { BoardQuestionActor } from '../../../../src/board/BoardQuestionTypes.js';
import { USER } from './BoardDatabaseContract.js';

const actor = { userId: USER.id, name: 'claude' };
const human: BoardQuestionActor = { userId: USER.id, name: 'Florian', kind: 'human', session: null };
const agent: BoardQuestionActor = { userId: USER.id, name: 'claude', kind: 'session', session: 's-1' };

// Gemeinsame Fälle der Suche für jede Umsetzung von BoardDatabase (In-Memory und SQL Server)
export function searchContract(name: string, create: () => Promise<BoardDatabase>, reset: () => Promise<void> = async () => {}) {
  describe(`Suche über alle Boards: ${name}`, () => {
    let db: BoardDatabase;
    let search: BoardSearchService;
    let boards: { sprint: number; zweites: number; incidents: number };

    beforeEach(async () => {
      await reset();
      db = await create();
      await db.transaction((u) => u.users.upsert(USER));
      const s = new BoardServices(db, () => new Date('2026-10-13T10:00:00.000Z'));
      await s.projects.create('ARC', 'Arches', actor);
      await s.projects.create('SV1', 'Service', actor);
      boards = {
        sprint: (await s.boards.create('ARC', 'Sprint', actor)).id,
        zweites: (await s.boards.create('ARC', 'Zweites', actor)).id,
        incidents: (await s.boards.create('SV1', 'Incidents', actor)).id,
      };
      await s.tickets.create(boards.sprint, { title: 'Login bauen', description: 'Formular mit Passwort', acceptance: 'Test ist grün' }, actor);
      await s.tickets.update('ARC-0001', { plan: 'Plan: OAuth einbauen', handbook: 'Vorher npm test laufen lassen' }, actor);
      await s.tickets.create(boards.zweites, { title: 'Export' }, actor);
      await s.comments.add('ARC-0002', 'Zwischenstand zur Speicherwahl', actor);
      await s.tickets.create(boards.incidents, { title: 'Incident Speicher voll', status: 'in_progress' }, actor);
      await s.questions.create('ARC', { title: 'Welcher Speicher?', description: 'Varianten', ticketIds: ['ARC-0002'] }, agent);
      await s.questions.answer('ARC-F0001', { text: 'SQL Server', reason: 'Lizenz vorhanden' }, human);
      await s.questions.comment('ARC-F0001', 'Kommentar zur Lizenz', human);
      await s.notes.add(boards.sprint, { title: 'Befund: Cache leert sich', body: 'zweite Zeile' }, actor);
      await s.tickets.create(boards.sprint, { title: 'Rabatt 100% auf _x_ [a]', description: `${'x'.repeat(9000)} Fundstelle am Ende` }, actor);
      search = new BoardSearchService(db);
    });
    afterEach(() => db.close());

    it('findet Titel, Kommentare und Fragen über Projekte und Boards, Titeltreffer zuerst', async () => {
      const result = await search.search({ q: 'speicher' });
      expect(result).toMatchObject({ query: 'speicher', terms: ['speicher'], truncated: false });
      expect(result.hits.map((h) => [h.kind, h.id, h.field])).toEqual([
        ['ticket', 'SV1-0001', 'title'],
        ['question', 'ARC-F0001', 'title'],
        ['ticket', 'ARC-0002', 'comment'],
      ]);
      expect(result.hits[0]).toMatchObject({
        ticketId: 'SV1-0001', status: 'in_progress', project: { key: 'SV1', name: 'Service' }, board: { id: boards.incidents, name: 'Incidents' },
      });
      expect(result.hits[1]).toMatchObject({ ticketId: null, board: null, status: 'answered', project: { key: 'ARC' } });
      expect(result.hits[2]).toMatchObject({ title: 'Export', board: { name: 'Zweites' }, at: expect.any(String) });
    });

    it('hebt die Fundstelle hervor', async () => {
      const [hit] = (await search.search({ q: 'OAUTH' })).hits;
      expect(hit).toMatchObject({ id: 'ARC-0001', field: 'plan', snippet: 'Plan: OAuth einbauen' });
      expect(hit.matches.map(([a, b]) => hit.snippet.slice(a, b))).toEqual(['OAuth']);
    });

    it('verlangt alle Wörter im selben Feld', async () => {
      expect((await search.search({ q: 'npm laufen' })).hits.map((h) => [h.id, h.field])).toEqual([['ARC-0001', 'handbook']]);
      expect((await search.search({ q: 'Login Passwort' })).hits).toEqual([]);
    });

    it('findet Abnahme, Entscheidung und Notiz', async () => {
      expect((await search.search({ q: 'grün' })).hits.map((h) => [h.id, h.field])).toEqual([['ARC-0001', 'acceptance']]);
      expect((await search.search({ q: 'lizenz' })).hits.map((h) => [h.id, h.field])).toEqual([['ARC-F0001', 'decision']]);
      const [note] = (await search.search({ q: 'cache' })).hits;
      expect(note).toMatchObject({ kind: 'note', title: 'Befund: Cache leert sich', board: { name: 'Sprint' }, project: { key: 'ARC' }, status: 'open', ticketId: null });
    });

    it('filtert nach Art', async () => {
      expect((await search.search({ q: 'speicher', kinds: ['question'] })).hits.map((h) => h.id)).toEqual(['ARC-F0001']);
    });

    it('findet Sonderzeichen wörtlich', async () => {
      expect((await search.search({ q: '100%' })).hits.map((h) => h.id)).toEqual(['ARC-0003']);
      expect((await search.search({ q: '_x_' })).hits.map((h) => h.id)).toEqual(['ARC-0003']);
      expect((await search.search({ q: '[a]' })).hits.map((h) => h.id)).toEqual(['ARC-0003']);
      expect((await search.search({ q: 'Ra%tt' })).hits).toEqual([]);
    });

    it('liefert die Fundstelle auch hinter 8000 Zeichen', async () => {
      const [hit] = (await search.search({ q: 'Fundstelle' })).hits;
      expect(hit).toMatchObject({ id: 'ARC-0003', field: 'description' });
      expect(hit.snippet.startsWith('…')).toBe(true);
      expect(hit.snippet).toContain('Fundstelle am Ende');
      expect(hit.matches).toHaveLength(1);
    });

    it('prüft die Eingabe', async () => {
      await expect(search.search({ q: ' a ' })).rejects.toMatchObject({ code: 'query-short' });
      await expect(search.search({ q: 'x'.repeat(101) })).rejects.toMatchObject({ code: 'query-long' });
      await expect(search.search({ q: 'a b c d e f' })).rejects.toMatchObject({ code: 'query-long' });
    });

    it('begrenzt die Treffer und meldet das', async () => {
      const s = new BoardServices(db, () => new Date('2026-10-13T10:00:00.000Z'));
      for (let i = 0; i < 31; i++) await s.tickets.create(boards.zweites, { title: `Massen ${i}` }, actor);
      const all = await search.search({ q: 'massen' });
      expect(all.hits).toHaveLength(30);
      expect(all.truncated).toBe(true);
      const few = await search.search({ q: 'massen', limit: 5 });
      expect(few.hits).toHaveLength(5);
      expect(few.truncated).toBe(true);
    });
  });
}
