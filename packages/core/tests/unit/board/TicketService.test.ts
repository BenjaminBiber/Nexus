import { describe, expect, it } from 'vitest';
import { BoardFixture } from './helpers/BoardFixture.js';

describe('TicketService', () => {
  describe('anlegen', () => {
    it('vergibt die ID aus dem Projekt und setzt Standardwerte', async () => {
      const f = await BoardFixture.create();
      const t = await f.tickets.create(f.boardId, { title: '  Login bauen ' }, f.actor);
      expect(t).toMatchObject({
        id: 'ARC-0001', title: 'Login bauen', status: 'backlog', priority: 'normal', needsPlanning: true,
        assignee: null, rev: 1, createdByName: 'claude', column: 'backlog', waiting: false, sortOrder: 0,
      });
      const second = await f.tickets.create(f.boardId, { title: 'Zweites', labels: ['ui, api', 'ui'] }, f.actor);
      expect(second).toMatchObject({ id: 'ARC-0002', labels: ['ui', 'api'], sortOrder: 1 });
    });

    it('schreibt „angelegt“ in den Verlauf', async () => {
      const f = await BoardFixture.create();
      await f.tickets.create(f.boardId, { title: 'A', status: 'planned' }, f.actor);
      expect((await f.tickets.get('ARC-0001')).history).toMatchObject([{ field: 'status', from: null, to: 'planned', note: 'angelegt', byName: 'claude' }]);
    });

    it('lehnt leeren Titel und unbekannte Voraussetzungen ab', async () => {
      const f = await BoardFixture.create();
      await expect(f.tickets.create(f.boardId, { title: ' ' }, f.actor)).rejects.toMatchObject({ code: 'title-empty' });
      await expect(f.tickets.create(f.boardId, { title: 'A', dependsOn: ['ARC-0042'] }, f.actor)).rejects.toMatchObject({ code: 'dependency-unknown' });
    });

    it('lehnt ein offenes Ticket mit gleichem Titel auf demselben Board ab; force, fertige Tickets und andere Boards nicht', async () => {
      const f = await BoardFixture.create();
      await f.tickets.create(f.boardId, { title: 'Login bauen' }, f.actor);
      await expect(f.tickets.create(f.boardId, { title: '  login BAUEN ' }, f.actor)).rejects.toMatchObject({
        code: 'ticket-duplicate', message: 'Ein offenes Ticket mit diesem Titel gibt es schon: ARC-0001.',
      });
      // Die Dublettenprüfung hat eine eigene Option; force allein übergeht sie nicht
      await expect(f.tickets.create(f.boardId, { title: 'Login bauen' }, f.actor, { force: true })).rejects.toMatchObject({ code: 'ticket-duplicate' });
      expect((await f.tickets.create(f.boardId, { title: 'Login bauen' }, f.actor, { allowDuplicate: true })).id).toBe('ARC-0002');
      // allowDuplicate übergeht keine Sperre
      await expect(f.tickets.create(f.boardId, { title: 'Login bauen', status: 'not_needed' }, f.actor, { allowDuplicate: true }))
        .rejects.toMatchObject({ code: 'not-needed-user' });
      expect((await f.tickets.create(f.boardId, { title: 'Login bauen (neu)' }, f.actor)).id).toBe('ARC-0003');
      const other = await f.boards.create('ARC', 'Anderes', f.actor);
      expect((await f.tickets.create(other.id, { title: 'Login bauen' }, f.actor)).boardId).toBe(other.id);
      // Erledigt und „Nicht mehr nötig“ zählen nicht
      await f.tickets.create(f.boardId, { title: 'Alt', status: 'done', acceptance: 'ok' }, f.actor);
      await f.tickets.create(f.boardId, { title: 'Weg', status: 'not_needed' }, f.actor, { byUser: true });
      expect((await f.tickets.create(f.boardId, { title: 'alt' }, f.actor)).title).toBe('alt');
      expect((await f.tickets.create(f.boardId, { title: 'Weg' }, f.actor)).title).toBe('Weg');
    });

    it('prüft die Status-Sperren auch beim Anlegen', async () => {
      const f = await BoardFixture.create();
      await expect(f.tickets.create(f.boardId, { title: 'A', status: 'done' }, f.actor)).rejects.toMatchObject({ code: 'acceptance-missing' });
      expect((await f.tickets.create(f.boardId, { title: 'B', status: 'done', acceptance: 'läuft' }, f.actor)).status).toBe('done');
      await f.tickets.create(f.boardId, { title: 'Basis' }, f.actor);
      await expect(f.tickets.create(f.boardId, { title: 'C', status: 'in_progress', dependsOn: ['ARC-0002'] }, f.actor)).rejects.toMatchObject({ code: 'dependency-open' });
      expect((await f.tickets.create(f.boardId, { title: 'D', status: 'in_progress', dependsOn: ['ARC-0002'] }, f.actor, { force: true })).status).toBe('in_progress');
      await expect(f.tickets.create(f.boardId, { title: 'E', status: 'not_needed' }, f.actor)).rejects.toMatchObject({ code: 'not-needed-user' });
      expect((await f.tickets.create(f.boardId, { title: 'F', status: 'not_needed' }, f.actor, { byUser: true })).status).toBe('not_needed');
    });
  });

  describe('lesen', () => {
    it('zeigt offene Voraussetzungen, Abhängige und „wartet“', async () => {
      const f = await BoardFixture.create();
      await f.tickets.create(f.boardId, { title: 'Basis' }, f.actor);
      await f.tickets.create(f.boardId, { title: 'Aufbau', dependsOn: ['arc-1'] }, f.actor);
      const detail = await f.tickets.get('arc-2');
      expect(detail).toMatchObject({ id: 'ARC-0002', dependsOn: ['ARC-0001'], dependsOpen: ['ARC-0001'], waiting: true, column: 'waiting' });
      expect((await f.tickets.get('ARC-0001')).dependents).toEqual(['ARC-0002']);
    });

    it('nennt in unlocks die Tickets, die mit diesem Ticket ihre letzte offene Voraussetzung verlieren', async () => {
      const f = await BoardFixture.create();
      await f.tickets.create(f.boardId, { title: 'Basis' }, f.actor);
      await f.tickets.create(f.boardId, { title: 'Zweite' }, f.actor);
      await f.tickets.create(f.boardId, { title: 'Nur Basis', dependsOn: ['ARC-0001'] }, f.actor);
      await f.tickets.create(f.boardId, { title: 'Beide', dependsOn: ['ARC-0001', 'ARC-0002'] }, f.actor);
      expect((await f.tickets.get('ARC-0001')).unlocks).toEqual(['ARC-0003']);
      expect((await f.tickets.get('ARC-0003')).unlocks).toEqual([]);
    });

    it('accept: nur Blockiert mit Grund „Human Interaction: …“, setzt Erledigt, leert den Grund und schreibt den Verlauf', async () => {
      const f = await BoardFixture.create();
      await f.tickets.create(f.boardId, { title: 'Ansehen', acceptance: 'ok' }, f.actor);
      await f.tickets.create(f.boardId, { title: 'Ohne Abnahme' }, f.actor);
      await expect(f.tickets.accept('ARC-0001', f.actor)).rejects.toMatchObject({ code: 'accept-not-possible' });
      await f.tickets.update('ARC-0001', { status: 'blocked', blockedReason: 'Merge: kaputt' }, f.actor);
      await expect(f.tickets.accept('ARC-0001', f.actor)).rejects.toMatchObject({ code: 'accept-not-possible' });
      await f.tickets.update('ARC-0001', { blockedReason: 'Human Interaction: Oberfläche ansehen' }, f.actor);
      const accepted = await f.tickets.accept('ARC-0001', f.actor);
      expect(accepted).toMatchObject({ status: 'done', blockedReason: null });
      const history = (await f.tickets.get('ARC-0001')).history;
      expect(history.some((h) => h.field === 'status' && h.to === 'done' && h.note === 'abgenommen')).toBe(true);
      await f.tickets.update('ARC-0002', { status: 'blocked', blockedReason: 'Human Interaction' }, f.actor);
      await expect(f.tickets.accept('ARC-0002', f.actor)).rejects.toMatchObject({ code: 'acceptance-missing' });
    });

    it('nennt erledigte Voraussetzungen, die noch einen Branch haben (noch nicht gemergt)', async () => {
      const f = await BoardFixture.create();
      await f.tickets.create(f.boardId, { title: 'Basis', acceptance: 'ok' }, f.actor);
      await f.tickets.create(f.boardId, { title: 'Zweite', acceptance: 'ok' }, f.actor);
      await f.tickets.create(f.boardId, { title: 'Aufbau', dependsOn: ['ARC-0001', 'ARC-0002'] }, f.actor);
      expect((await f.tickets.get('ARC-0003')).dependsUnmerged).toEqual([]);
      await f.tickets.update('ARC-0001', { status: 'done', branch: 'ticket/ARC-0001-basis' }, f.actor);
      await f.tickets.update('ARC-0002', { status: 'done' }, f.actor);
      expect(await f.tickets.get('ARC-0003')).toMatchObject({ dependsOpen: [], dependsUnmerged: ['ARC-0001'], waiting: false });
      await f.tickets.update('ARC-0001', { branch: null }, f.actor);
      expect((await f.tickets.get('ARC-0003')).dependsUnmerged).toEqual([]);
    });

    it('meldet ein unbekanntes Ticket', async () => {
      const f = await BoardFixture.create();
      await expect(f.tickets.get('ARC-0099')).rejects.toMatchObject({ code: 'ticket-not-found' });
    });

    it('filtert nach Spalte, Suchtext, Label, Zuweisung und Planungsbedarf und sortiert', async () => {
      const f = await BoardFixture.create();
      await f.tickets.create(f.boardId, { title: 'Login', labels: ['ui'], priority: 'low' }, f.actor);
      await f.tickets.create(f.boardId, { title: 'Cache', description: 'login-Token', priority: 'urgent', needsPlanning: false }, f.actor);
      await f.tickets.create(f.boardId, { title: 'Wartet', dependsOn: ['ARC-0001'] }, f.actor);
      const all = await f.tickets.list({ boardIds: [f.boardId] });
      expect(all.map((t) => t.id)).toEqual(['ARC-0002', 'ARC-0001', 'ARC-0003']);
      expect((await f.tickets.list({ boardIds: [f.boardId], q: 'LOGIN' })).map((t) => t.id)).toEqual(['ARC-0002', 'ARC-0001']);
      expect((await f.tickets.list({ boardIds: [f.boardId], label: 'ui' })).map((t) => t.id)).toEqual(['ARC-0001']);
      expect((await f.tickets.list({ boardIds: [f.boardId], columns: ['waiting'] })).map((t) => t.id)).toEqual(['ARC-0003']);
      expect((await f.tickets.list({ boardIds: [f.boardId], columns: ['backlog'] })).map((t) => t.id)).toEqual(['ARC-0002', 'ARC-0001']);
      expect((await f.tickets.list({ boardIds: [f.boardId], needsPlanning: true })).map((t) => t.id)).toEqual(['ARC-0001', 'ARC-0003']);
    });
  });

  describe('ändern', () => {
    it('behandelt den Blockiert-Grund beim Ändern des Status über update', async () => {
      const f = await BoardFixture.create();
      for (const n of ['A', 'B', 'C']) await f.tickets.create(f.boardId, { title: n }, f.actor);
      for (const id of ['ARC-0001', 'ARC-0002', 'ARC-0003']) await f.tickets.update(id, { status: 'blocked', blockedReason: 'Zugang fehlt' }, f.actor);
      expect((await f.tickets.update('ARC-0001', { status: 'planned' }, f.actor)).blockedReason).toBeNull();
      expect((await f.tickets.update('ARC-0002', { status: 'planned', blockedReason: 'Rest' }, f.actor)).blockedReason).toBe('Rest');
      expect((await f.tickets.update('ARC-0003', { blockedReason: 'Neu' }, f.actor)).blockedReason).toBe('Neu');
    });

    it('ändert Felder, erhöht die Version und schreibt den Verlauf', async () => {
      const f = await BoardFixture.create();
      await f.tickets.create(f.boardId, { title: 'A' }, f.actor);
      const t = await f.tickets.update('ARC-0001', { title: 'B', plan: 'Plan', priority: 'high', assignee: 'claude' }, f.actor);
      expect(t).toMatchObject({ title: 'B', plan: 'Plan', priority: 'high', assignee: 'claude', rev: 2, status: 'backlog', needsPlanning: true });
      const fields = (await f.tickets.get('ARC-0001')).history.map((h) => h.field);
      expect(fields).toEqual(['status', 'title', 'priority', 'assignee', 'plan']);
    });

    it('lehnt eine leere Änderung und eine veraltete Version ab', async () => {
      const f = await BoardFixture.create();
      await f.tickets.create(f.boardId, { title: 'A' }, f.actor);
      await expect(f.tickets.update('ARC-0001', {}, f.actor)).rejects.toMatchObject({ code: 'nothing-changed' });
      await f.tickets.update('ARC-0001', { title: 'B' }, f.actor, { rev: 1 });
      await expect(f.tickets.update('ARC-0001', { title: 'C' }, f.actor, { rev: 1 })).rejects.toMatchObject({ code: 'conflict' });
    });

    it('ergänzt und entfernt Voraussetzungen und lehnt Kreise ab', async () => {
      const f = await BoardFixture.create();
      for (const title of ['A', 'B', 'C']) await f.tickets.create(f.boardId, { title }, f.actor);
      await f.tickets.update('ARC-0003', { dependsOnAdd: ['ARC-0001', 'ARC-0002'] }, f.actor);
      expect((await f.tickets.update('ARC-0003', { dependsOnRemove: ['ARC-0001'] }, f.actor)).dependsOn).toEqual(['ARC-0002']);
      await expect(f.tickets.update('ARC-0002', { dependsOn: ['ARC-0003'] }, f.actor)).rejects.toMatchObject({ code: 'dependency-cycle' });
    });

    it('verschiebt auf ein anderes Board desselben Projekts, nicht in ein anderes Projekt', async () => {
      const f = await BoardFixture.create();
      const other = await f.boards.create('ARC', 'Kollege', f.actor);
      await f.projects.create('INC', 'Incidents', f.actor);
      const foreign = await f.boards.create('INC', 'Fremd', f.actor);
      await f.tickets.create(f.boardId, { title: 'A' }, f.actor);
      expect((await f.tickets.update('ARC-0001', { boardId: other.id }, f.actor))).toMatchObject({ id: 'ARC-0001', boardId: other.id });
      await expect(f.tickets.update('ARC-0001', { boardId: foreign.id }, f.actor)).rejects.toMatchObject({ code: 'board-other-project' });
    });
  });

  describe('verschieben', () => {
    it('verlangt für Erledigt eine Abnahme, nimmt sie aber im selben Aufruf an', async () => {
      const f = await BoardFixture.create();
      await f.tickets.create(f.boardId, { title: 'A' }, f.actor);
      await expect(f.tickets.move('ARC-0001', 'done', f.actor)).rejects.toMatchObject({ code: 'acceptance-missing' });
      const t = await f.tickets.move('ARC-0001', 'done', f.actor, { acceptance: 'Test grün' });
      expect(t).toMatchObject({ status: 'done', acceptance: 'Test grün' });
      expect((await f.tickets.get('ARC-0001')).history.at(-1)).toMatchObject({ note: 'verschoben' });
    });

    it('sperrt In Arbeit und Erledigt bei offener Voraussetzung, außer mit force', async () => {
      const f = await BoardFixture.create();
      await f.tickets.create(f.boardId, { title: 'A' }, f.actor);
      await f.tickets.create(f.boardId, { title: 'B', dependsOn: ['ARC-0001'] }, f.actor);
      await expect(f.tickets.move('ARC-0002', 'in_progress', f.actor)).rejects.toMatchObject({ code: 'dependency-open', message: expect.stringContaining('ARC-0001') });
      await expect(f.tickets.move('ARC-0002', 'in_progress', f.actor)).rejects.toMatchObject({ message: expect.not.stringContaining('--force') });
      expect((await f.tickets.move('ARC-0002', 'planned', f.actor)).status).toBe('planned');
      expect((await f.tickets.move('ARC-0002', 'in_progress', f.actor, { force: true })).status).toBe('in_progress');
    });

    it('überlässt „nicht mehr nötig“ dem Nutzer', async () => {
      const f = await BoardFixture.create();
      await f.tickets.create(f.boardId, { title: 'A' }, f.actor);
      await expect(f.tickets.move('ARC-0001', 'not_needed', f.actor)).rejects.toMatchObject({ code: 'not-needed-user' });
      expect((await f.tickets.move('ARC-0001', 'not_needed', f.actor, { byUser: true })).status).toBe('not_needed');
    });

    it('löscht den Blockiert-Grund beim Verlassen von Blockiert und hängt hinten an', async () => {
      const f = await BoardFixture.create();
      await f.tickets.create(f.boardId, { title: 'A', status: 'planned' }, f.actor);
      await f.tickets.create(f.boardId, { title: 'B' }, f.actor);
      await f.tickets.update('ARC-0002', { status: 'blocked', blockedReason: 'Zugang fehlt' }, f.actor);
      const t = await f.tickets.move('ARC-0002', 'planned', f.actor);
      expect(t).toMatchObject({ blockedReason: null, sortOrder: 1 });
    });
  });

  describe('löschen', () => {
    it('protokolliert die Löschung und entfernt Verweise', async () => {
      const f = await BoardFixture.create();
      await f.tickets.create(f.boardId, { title: 'A' }, f.actor);
      await f.tickets.create(f.boardId, { title: 'B', dependsOn: ['ARC-0001'] }, f.actor);
      await f.tickets.delete('ARC-0001', f.actor);
      await expect(f.tickets.get('ARC-0001')).rejects.toMatchObject({ code: 'ticket-not-found' });
      expect((await f.tickets.get('ARC-0002')).dependsOn).toEqual([]);
      expect(await f.db.transaction((u) => u.deletions.list())).toMatchObject([{ ticketId: 'ARC-0001', ticketTitle: 'A', fromStatus: 'backlog', byName: 'claude' }]);
    });
  });

  describe('Längen und Labels', () => {
    it('lehnt zu lange Titel ab, beim Anlegen und Ändern', async () => {
      const f = await BoardFixture.create();
      await expect(f.tickets.create(f.boardId, { title: 'x'.repeat(401) }, f.actor)).rejects.toMatchObject({ code: 'value-too-long', message: expect.stringContaining('Titel') });
      await f.tickets.create(f.boardId, { title: 'x'.repeat(400) }, f.actor);
      await expect(f.tickets.update('ARC-0001', { title: 'y'.repeat(401) }, f.actor)).rejects.toMatchObject({ code: 'value-too-long' });
    });

    it('lehnt zu lange Labels, Gründe, Zuständige und Handelnde ab', async () => {
      const f = await BoardFixture.create();
      await expect(f.tickets.create(f.boardId, { title: 'A', labels: ['l'.repeat(101)] }, f.actor)).rejects.toMatchObject({ code: 'value-too-long', message: expect.stringContaining('Label') });
      await f.tickets.create(f.boardId, { title: 'A', labels: ['l'.repeat(100)] }, f.actor);
      await expect(f.tickets.update('ARC-0001', { labels: ['ok', 'l'.repeat(101)] }, f.actor)).rejects.toMatchObject({ code: 'value-too-long' });
      await expect(f.tickets.update('ARC-0001', { blockedReason: 'r'.repeat(501) }, f.actor)).rejects.toMatchObject({ code: 'value-too-long' });
      await expect(f.tickets.update('ARC-0001', { assignee: 'a'.repeat(101) }, f.actor)).rejects.toMatchObject({ code: 'value-too-long' });
      await expect(f.tickets.claim('ARC-0001', 'a'.repeat(101), f.actor)).rejects.toMatchObject({ code: 'value-too-long' });
      await expect(f.tickets.update('ARC-0001', { title: 'B' }, { ...f.actor, name: 'n'.repeat(101) })).rejects.toMatchObject({ code: 'value-too-long', message: expect.stringContaining('Handelnden') });
      await expect(f.comments.add('ARC-0001', 'Hallo', { ...f.actor, name: 'n'.repeat(101) })).rejects.toMatchObject({ code: 'value-too-long' });
      expect((await f.tickets.get('ARC-0001')).title).toBe('A');
    });

    it('entfernt doppelte Labels ohne Rücksicht auf Groß-/Kleinschreibung (erste Schreibweise bleibt)', async () => {
      const f = await BoardFixture.create();
      const t = await f.tickets.create(f.boardId, { title: 'A', labels: ['UI', 'ui, Api', 'api', 'Ui'] }, f.actor);
      expect(t.labels).toEqual(['UI', 'Api']);
      expect((await f.tickets.update(t.id, { labels: ['cache', 'CACHE'] }, f.actor)).labels).toEqual(['cache']);
    });
  });
});
