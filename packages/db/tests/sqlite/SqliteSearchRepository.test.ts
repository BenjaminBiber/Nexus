import type { SearchKind, SearchRow } from '@harness/core';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { TempSqlite } from './helpers/TempSqlite.js';

const USER = '11111111-1111-1111-1111-111111111111';
const NOW = '2026-10-13T10:00:00.000Z';
const LATER = '2026-10-13T11:00:00.000Z';
const ALL: SearchKind[] = ['ticket', 'question', 'note'];
// Langes Feld für den Ausschnitt: Fundstelle bei Index 9001
const LONG = `${'x'.repeat(9000)} Fundstelle am Ende`;

interface TicketSeed {
  id: string; number: number; title: string; description?: string; acceptance?: string; plan?: string; handbook?: string;
  boardId?: number; status?: string; projectId?: number; createdAt?: string;
}

describe('SQLite: Suche', () => {
  let temp: TempSqlite;

  beforeEach(async () => { temp = await TempSqlite.create(); });
  afterEach(async () => { await temp.dispose(); });

  // Begriffe suchen und nur die interessanten Felder je Zeile liefern
  const find = (terms: string[], kinds: SearchKind[] = ALL, perSource = 30) =>
    temp.db.transaction((u) => u.search.find({ terms, kinds, perSource }));
  const shape = (rows: SearchRow[]) => rows.map((r) => [r.kind, r.id, r.field]);

  async function ticket(t: TicketSeed): Promise<void> {
    await temp.sql(
      `INSERT INTO Tickets (Id, ProjectId, BoardId, Number, Title, Description, Status, Priority, SortOrder, NeedsPlanning,
         Acceptance, "Plan", Handbook, CreatedAt, CreatedById, CreatedByName, UpdatedAt, Rev)
       VALUES (@id, @projectId, @boardId, @number, @title, @description, @status, 'normal', @number, 0,
         @acceptance, @plan, @handbook, @createdAt, @userId, 'Tester', @createdAt, 1)`,
      {
        id: t.id, projectId: t.projectId ?? 1, boardId: t.boardId ?? 1, number: t.number, title: t.title,
        description: t.description ?? '', status: t.status ?? 'backlog', acceptance: t.acceptance ?? '',
        plan: t.plan ?? '', handbook: t.handbook ?? '', createdAt: t.createdAt ?? NOW, userId: USER,
      },
    );
  }

  // Zwei Projekte mit je einem Board und die Tickets, Kommentare, Fragen und Notizen der Fälle
  async function seed(): Promise<void> {
    await temp.sql('INSERT INTO Users (Id, Name, CreatedAt) VALUES (@id, @name, @now)', { id: USER, name: 'Tester', now: NOW });
    for (const [key, name] of [['ARC', 'Arches'], ['SV1', 'Service']]) {
      await temp.sql('INSERT INTO Projects ("Key", Name, CreatedAt, CreatedBy) VALUES (@key, @name, @now, @user)',
        { key, name, now: NOW, user: USER });
    }
    for (const [projectId, name] of [[1, 'Sprint'], [2, 'Incidents']] as Array<[number, string]>) {
      await temp.sql('INSERT INTO Boards (ProjectId, OwnerId, Name, CreatedAt) VALUES (@projectId, @user, @name, @now)',
        { projectId, user: USER, name, now: NOW });
    }

    await ticket({
      id: 'ARC-0001', number: 1, title: 'Großer Ärger im Login', description: 'Formular mit Passwort',
      acceptance: 'Test ist grün', plan: 'Plan: OAuth einbauen', handbook: 'Vorher npm test laufen lassen',
    });
    await ticket({ id: 'ARC-0002', number: 2, title: 'Ein arger Tippfehler' });
    await ticket({ id: 'ARC-0003', number: 3, title: 'Rabatt 100% auf _x_ [a]', description: LONG });
    await ticket({ id: 'SV1-0001', number: 1, title: 'Incident Speicher voll', projectId: 2, boardId: 2, status: 'in_progress', createdAt: LATER });

    await temp.sql(
      'INSERT INTO Comments (TicketId, AuthorId, AuthorName, Body, CreatedAt) VALUES (@ticketId, @user, @name, @body, @now)',
      { ticketId: 'ARC-0002', user: USER, name: 'Tester', body: 'Zwischenstand zur Speicherwahl', now: NOW },
    );
    await temp.sql(
      `INSERT INTO Questions (Id, ProjectId, Number, Title, Description, State, OriginKind, OriginName, DecisionOutcome,
         DecisionText, DecisionReason, DecidedById, DecidedByName, DecidedAt, CreatedAt, CreatedById, UpdatedAt, Rev)
       VALUES ('ARC-F0001', 1, 1, 'Welcher Speicher?', 'Varianten', 'answered', 'session', 'claude', 'answered',
         'SQL Server', 'Lizenz vorhanden', @user, 'Florian', @now, @now, @user, @now, 2)`,
      { user: USER, now: NOW },
    );
    await temp.sql(
      `INSERT INTO QuestionComments (QuestionId, AuthorId, AuthorName, AuthorKind, Body, CreatedAt)
       VALUES ('ARC-F0001', @user, 'Florian', 'human', 'Kommentar zur Lizenz', @now)`,
      { user: USER, now: NOW },
    );
    await temp.sql(
      `INSERT INTO Notes (BoardId, AuthorId, AuthorName, Title, Body, Labels, State, CreatedAt)
       VALUES (1, @user, 'Tester', 'Befund: Cache leert sich', 'zweite Zeile', '', 'open', @now)`,
      { user: USER, now: NOW },
    );
  }

  it('findet „Ärger“ über „ärger“, aber nicht über „arger“', async () => {
    await seed();

    expect(shape((await find(['ärger'])).rows)).toEqual([['ticket', 'ARC-0001', 'title']]);
    expect(shape((await find(['ÄRGER'])).rows)).toEqual([['ticket', 'ARC-0001', 'title']]);
    expect(shape((await find(['arger'])).rows)).toEqual([['ticket', 'ARC-0002', 'title']]);
  });

  it('verlangt alle Begriffe im selben Feld', async () => {
    await seed();

    expect(shape((await find(['npm', 'laufen'])).rows)).toEqual([['ticket', 'ARC-0001', 'handbook']]);
    expect((await find(['login', 'passwort'])).rows).toEqual([]);
  });

  it('durchsucht alle Quellen in der Reihenfolge des MSSQL-Repositories', async () => {
    await seed();

    const speicher = (await find(['speicher'])).rows;
    expect(shape(speicher)).toEqual([
      ['ticket', 'SV1-0001', 'title'], ['ticket', 'ARC-0002', 'comment'], ['question', 'ARC-F0001', 'title'],
    ]);
    expect(speicher[1]).toMatchObject({ title: 'Ein arger Tippfehler', at: NOW, boardId: 1, projectId: 1 });
    expect(speicher[2]).toMatchObject({ ticketId: null, boardId: null, status: 'answered', at: null });

    const lizenz = (await find(['lizenz'])).rows;
    expect(shape(lizenz)).toEqual([['question', 'ARC-F0001', 'decision'], ['question', 'ARC-F0001', 'question-comment']]);
    expect(lizenz[0]).toMatchObject({ text: 'SQL Server\nLizenz vorhanden', at: NOW });

    expect((await find(['cache'])).rows).toEqual([{
      kind: 'note', id: '1', ticketId: null, field: 'note', title: 'Befund: Cache leert sich',
      projectId: 1, boardId: 1, status: 'open', text: 'Befund: Cache leert sich\nzweite Zeile',
      textStart: 0, textLength: 37, at: NOW,
    }]);
  });

  it('beachtet den Art-Filter', async () => {
    await seed();

    expect(shape((await find(['speicher'], ['question'])).rows)).toEqual([['question', 'ARC-F0001', 'title']]);
  });

  it('nimmt Sonderzeichen wörtlich', async () => {
    await seed();

    for (const term of ['100%', '_x_', '[a]']) {
      expect(shape((await find([term])).rows)).toEqual([['ticket', 'ARC-0003', 'title']]);
    }
    expect((await find(['ra%tt'])).rows).toEqual([]);
  });

  it('liefert denselben Ausschnitt wie MSSQL', async () => {
    await seed();

    const [long] = (await find(['fundstelle'])).rows;
    expect(long).toMatchObject({ id: 'ARC-0003', field: 'description', textStart: 8851, textLength: 9019 });
    expect(long.text.length).toBe(168);
    expect(long.text.endsWith('Fundstelle am Ende')).toBe(true);

    const [plan] = (await find(['oauth'])).rows;
    expect(plan).toMatchObject({ field: 'plan', text: 'Plan: OAuth einbauen', textStart: 0, textLength: 20 });
  });

  it('begrenzt je Quelle und sortiert neueste zuerst', async () => {
    await seed();
    for (let n = 10; n <= 14; n += 1) await ticket({ id: `ARC-00${n}`, number: n, title: 'Massen i' });

    const limited = await find(['massen'], ['ticket'], 3);
    expect(limited.rows.map((r) => r.id)).toEqual(['ARC-0014', 'ARC-0013', 'ARC-0012']);
    expect(limited.truncated).toBe(true);

    const full = await find(['massen'], ['ticket'], 5);
    expect(full.rows).toHaveLength(5);
    expect(full.truncated).toBe(false);
  });

  it('findet einen alten Treffer hinter vielen neueren Zeilen ohne Treffer', async () => {
    await seed();
    await ticket({ id: 'ARC-0999', number: 999, title: 'Nadel im Heu', createdAt: '2026-01-01T00:00:00.000Z' });
    for (let n = 0; n < 450; n += 1) await ticket({ id: `ARC-${1000 + n}`, number: 1000 + n, title: 'Füller' });

    expect(shape((await find(['nadel'], ['ticket'])).rows)).toEqual([['ticket', 'ARC-0999', 'title']]);
  });

  it('liefert ohne Begriffe oder ohne Arten nichts', async () => {
    await seed();

    expect(await find([], ALL)).toEqual({ rows: [], truncated: false });
    expect(await find(['x'], [])).toEqual({ rows: [], truncated: false });
  });
});
