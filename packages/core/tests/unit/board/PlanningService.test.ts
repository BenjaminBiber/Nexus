import { describe, expect, it } from 'vitest';
import { BoardFixture } from './helpers/BoardFixture.js';
import type { BoardDatabase } from '../../../src/board/BoardDatabase.js';
import { PLANNING_CLAIM_TTL_MS, type Ticket } from '../../../src/board/BoardTypes.js';
import { PlanningCandidate } from '../../../src/board/PlanningCandidate.js';
import { PlanningService } from '../../../src/board/PlanningService.js';

describe('PlanningService', () => {
  it('schreibt den Plan, setzt Geplant, löscht Planungsbedarf und Claim', async () => {
    const f = await BoardFixture.create();
    await f.tickets.create(f.boardId, { title: 'A' }, f.actor);
    await f.planning.next([f.boardId], 'claude', f.actor);
    const t = await f.planning.plan('ARC-0001', { plan: '## Ziel', diagram: '```mermaid\nflowchart TD\n  A-->B\n```' }, f.actor);
    expect(t).toMatchObject({
      status: 'planned', needsPlanning: false, planningClaimBy: null, planningClaimAt: null,
      plannedBy: 'claude', plannedAt: '2026-10-13T10:00:00.000Z', plan: '## Ziel', diagram: 'flowchart TD\n  A-->B',
    });
    expect((await f.tickets.get('ARC-0001')).history.at(-1)?.note).toBe('geplant');
  });

  it('protokolliert Planungs-Claim und dessen Freigabe im Verlauf', async () => {
    const f = await BoardFixture.create();
    await f.tickets.create(f.boardId, { title: 'A' }, f.actor);
    await f.planning.next([f.boardId], 'junior-x', f.actor);
    await f.planning.plan('ARC-0001', { plan: 'P' }, f.actor);
    const claims = (await f.tickets.get('ARC-0001')).history.filter((h) => h.field === 'planningClaimBy');
    expect(claims.map((h) => [h.from, h.to])).toEqual([[null, 'junior-x'], ['junior-x', null]]);
  });

  it('vermerkt „Plan aktualisiert“, wenn der Status gleich bleibt', async () => {
    const f = await BoardFixture.create();
    await f.tickets.create(f.boardId, { title: 'A', status: 'planned' }, f.actor);
    await f.planning.plan('ARC-0001', { handbook: 'HB' }, f.actor);
    expect((await f.tickets.get('ARC-0001')).history.at(-1)?.note).toBe('Plan aktualisiert');
  });

  it('schreibt die Abnahme mit; ohne oder mit leerer Abnahme bleibt die vorhandene', async () => {
    const f = await BoardFixture.create();
    await f.tickets.create(f.boardId, { title: 'A', acceptance: 'alt' }, f.actor);
    expect((await f.planning.plan('ARC-0001', { plan: 'P' }, f.actor)).acceptance).toBe('alt');
    expect((await f.planning.plan('ARC-0001', { plan: 'P', acceptance: '  ' }, f.actor)).acceptance).toBe('alt');
    expect((await f.planning.plan('ARC-0001', { plan: 'P', acceptance: '- Test deckt den Fall ab' }, f.actor)).acceptance).toBe('- Test deckt den Fall ab');
  });

  it('verlangt mindestens einen Text', async () => {
    const f = await BoardFixture.create();
    await f.tickets.create(f.boardId, { title: 'A' }, f.actor);
    await expect(f.planning.plan('ARC-0001', {}, f.actor)).rejects.toMatchObject({ code: 'plan-empty' });
  });

  it('liefert das nächste Ticket mit Planungsbedarf und überspringt In Planung, Nicht mehr nötig und Tickets mit offener Frage', async () => {
    const f = await BoardFixture.create();
    await f.tickets.create(f.boardId, { title: 'Nutzer plant', status: 'in_planning' }, f.actor);
    await f.tickets.create(f.boardId, { title: 'Mit Frage', priority: 'urgent' }, f.actor);
    await f.questions.create('ARC', { title: 'Offen', ticketIds: ['ARC-0002'] }, f.agent);
    await f.tickets.create(f.boardId, { title: 'Ohne Planung', needsPlanning: false }, f.actor);
    await f.tickets.create(f.boardId, { title: 'Normal' }, f.actor);
    await f.tickets.create(f.boardId, { title: 'Dringend', priority: 'urgent' }, f.actor);
    expect((await f.planning.next([f.boardId], null, f.actor))?.id).toBe('ARC-0005');
  });

  it('beansprucht mit claim für 15 Minuten', async () => {
    const f = await BoardFixture.create();
    await f.tickets.create(f.boardId, { title: 'A' }, f.actor);
    const claimed = await f.planning.next([f.boardId], 'claude-1', f.actor);
    expect(claimed).toMatchObject({ id: 'ARC-0001', planningClaimBy: 'claude-1', rev: 2 });
    expect(await f.planning.next([f.boardId], 'claude-2', f.actor)).toBeNull();
    f.advance(16);
    expect((await f.planning.next([f.boardId], 'claude-2', f.actor))?.planningClaimBy).toBe('claude-2');
  });

  it('claim beansprucht genau ein Ticket, lehnt einen lebenden fremden Claim ab, release gibt nur den eigenen frei', async () => {
    const f = await BoardFixture.create();
    await f.tickets.create(f.boardId, { title: 'A' }, f.actor);
    expect(await f.planning.claim('ARC-0001', 'planner-ARC-0001', f.actor)).toMatchObject({ planningClaimBy: 'planner-ARC-0001' });
    expect(await f.planning.next([f.boardId], 'claude', f.actor)).toBeNull();
    await expect(f.planning.claim('ARC-0001', 'claude', f.actor)).rejects.toMatchObject({ code: 'plan-claimed' });
    await f.planning.release('ARC-0001', 'claude', f.actor);
    expect((await f.tickets.get('ARC-0001')).planningClaimBy).toBe('planner-ARC-0001');
    await f.planning.release('ARC-0001', 'planner-ARC-0001', f.actor);
    expect((await f.tickets.get('ARC-0001')).planningClaimBy).toBeNull();
    await f.planning.claim('ARC-0001', 'planner-ARC-0001', f.actor);
    f.advance(16);
    expect((await f.planning.claim('ARC-0001', 'claude', f.actor)).planningClaimBy).toBe('claude');
    await expect(f.planning.claim('ARC-0001', ' ', f.actor)).rejects.toMatchObject({ code: 'agent-missing' });
  });

  it('überspringt ein Ticket, das zwischen Auswahl und Sperre beansprucht wurde', async () => {
    const f = await BoardFixture.create();
    await f.tickets.create(f.boardId, { title: 'Dringend', priority: 'urgent' }, f.actor);
    await f.tickets.create(f.boardId, { title: 'Normal' }, f.actor);
    // Gesperrtes Lesen sieht den Claim eines anderen Agenten, der nach der Auswahl bestätigt wurde
    const racing: BoardDatabase = {
      close: () => f.db.close(),
      changeMarker: (boardId) => f.db.changeMarker(boardId),
      questionMarker: (projectId) => f.db.questionMarker(projectId),
      transaction: (work) => f.db.transaction((u) => work({
        ...u,
        tickets: {
          ...u.tickets,
          byId: async (id, forUpdate) => {
            const t = await u.tickets.byId(id, forUpdate);
            return t && forUpdate && id === 'ARC-0001' ? { ...t, planningClaimBy: 'other', planningClaimAt: f.now.toISOString() } : t;
          },
        },
      })),
    };
    const planning = new PlanningService(racing, f.clock);
    expect(await planning.next([f.boardId], 'claude-1', f.actor)).toMatchObject({ id: 'ARC-0002', planningClaimBy: 'claude-1' });
  });

  it('liefert null, wenn beim Sperren kein Kandidat mehr übrig ist', async () => {
    const f = await BoardFixture.create();
    await f.tickets.create(f.boardId, { title: 'A' }, f.actor);
    const racing: BoardDatabase = {
      close: () => f.db.close(),
      changeMarker: (boardId) => f.db.changeMarker(boardId),
      questionMarker: (projectId) => f.db.questionMarker(projectId),
      transaction: (work) => f.db.transaction((u) => work({
        ...u,
        tickets: { ...u.tickets, byId: async (id, forUpdate) => (forUpdate ? null : u.tickets.byId(id)) },
      })),
    };
    expect(await new PlanningService(racing, f.clock).next([f.boardId], 'claude-1', f.actor)).toBeNull();
  });

  it('lehnt einen leeren Agentennamen ab und kürzt Leerzeichen', async () => {
    const f = await BoardFixture.create();
    await f.tickets.create(f.boardId, { title: 'A' }, f.actor);
    await expect(f.planning.next([f.boardId], '   ', f.actor)).rejects.toMatchObject({ code: 'agent-missing' });
    await expect(f.planning.next([f.boardId], 'a'.repeat(101), f.actor)).rejects.toMatchObject({ code: 'value-too-long' });
    expect(await f.planning.next([f.boardId], ' claude-1 ', f.actor)).toMatchObject({ planningClaimBy: 'claude-1' });
  });
});

describe('PlanningCandidate', () => {
  const now = Date.parse('2026-10-13T10:00:00.000Z');
  const base = {
    needsPlanning: true, status: 'backlog', blockedReason: null, planningClaimBy: null, planningClaimAt: null,
  } as unknown as Ticket;

  it('nimmt Tickets mit Planungsbedarf ohne Claim', () => {
    expect(PlanningCandidate.matches(base, now)).toBe(true);
  });

  it('verwirft fehlenden Planungsbedarf, In Planung, Nicht mehr nötig und offene Fragen; Rückfrage ist kein Sonderfall', () => {
    expect(PlanningCandidate.matches({ ...base, needsPlanning: false }, now)).toBe(false);
    expect(PlanningCandidate.matches({ ...base, status: 'in_planning' }, now)).toBe(false);
    expect(PlanningCandidate.matches({ ...base, status: 'not_needed' }, now)).toBe(false);
    expect(PlanningCandidate.matches(base, now, 1)).toBe(false);
    expect(PlanningCandidate.matches({ ...base, status: 'blocked', blockedReason: 'Rückfrage' }, now)).toBe(true);
    expect(PlanningCandidate.matches({ ...base, status: 'blocked', blockedReason: 'Warten' }, now)).toBe(true);
  });

  it('verwirft lebende Claims und nimmt abgelaufene', () => {
    expect(PlanningCandidate.matches({ ...base, planningClaimBy: 'x', planningClaimAt: new Date(now - 60_000).toISOString() }, now)).toBe(false);
    expect(PlanningCandidate.matches({ ...base, planningClaimBy: 'x', planningClaimAt: new Date(now - PLANNING_CLAIM_TTL_MS).toISOString() }, now)).toBe(true);
  });
});
