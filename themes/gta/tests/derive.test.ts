// Spielwerte aus dem Board-Stand: Geld, Fortschritt, Kontingente, Badges, Blips
import { describe, expect, it } from 'vitest';
import type { NavItem, UsageState } from '@nexus/theme-sdk';
import { blipPosition, deriveGame, STEP_REWARD, TICKET_REWARD } from '../src/data/derive.js';
import type { BoardView, Card, ColumnId } from '../src/data/types.js';

const card = (id: string, column: ColumnId, stepsDone = 0, stepsTotal = 0, blockedReason: string | null = null): Card => ({
  id,
  title: `Titel ${id}`,
  column,
  blockedReason,
  stepsDone,
  stepsTotal,
  priority: 'normal',
  assignee: null,
});

const VIEW: BoardView = {
  boardId: 1,
  columns: [
    { id: 'backlog', title: 'Backlog', tickets: [card('N-1', 'backlog', 0, 4)] },
    { id: 'in_progress', title: 'In Arbeit', tickets: [card('N-2', 'in_progress', 2, 5), card('N-3', 'in_progress', 1, 1)] },
    { id: 'blocked', title: 'Blockiert', tickets: [card('N-4', 'blocked', 0, 2, 'Zugang fehlt')] },
    { id: 'done', title: 'Erledigt', tickets: [card('N-5', 'done', 3, 3), card('N-6', 'done', 2, 2)] },
    { id: 'not_needed', title: 'Nicht nötig', tickets: [card('N-7', 'not_needed', 1, 1)] },
  ],
};

const NO_USAGE: UsageState = { enabled: false, available: false, reason: 'aus', fiveHour: null, sevenDay: null };
const usage = (five: number, seven: number): UsageState => ({
  enabled: true,
  available: true,
  reason: null,
  fiveHour: { percent: five, resetsAt: null },
  sevenDay: { percent: seven, resetsAt: null },
});

const nav = (badges: Partial<Record<NavItem['id'], number>>): NavItem[] =>
  (['board', 'inbox', 'questions'] as const).map((id) => ({ id, label: id, href: `/p/p1/${id}`, active: false, badge: badges[id] }));

describe('deriveGame', () => {
  it('ohne Board-Stand: nichts geladen, alles leer', () => {
    expect(deriveGame(null, NO_USAGE, [])).toEqual({
      loaded: false,
      total: 0,
      done: 0,
      open: 0,
      blocked: 0,
      boardProgress: null,
      stepsDone: 0,
      stepsTotal: 0,
      money: 0,
      quota5h: null,
      quota7d: null,
      tasks: 0,
      questions: 0,
      columns: [],
      blips: [],
    });
  });

  it('zählt Tickets ohne not_needed, Schritte nur offener Tickets', () => {
    const game = deriveGame(VIEW, NO_USAGE, []);
    expect(game).toMatchObject({ loaded: true, total: 6, done: 2, open: 4, blocked: 1, stepsDone: 3, stepsTotal: 12 });
    expect(game.boardProgress).toBeCloseTo(2 / 6);
  });

  it('Geld: erledigte Tickets und Schritte offener Tickets', () => {
    expect(TICKET_REWARD).toBe(40_000);
    expect(STEP_REWARD).toBe(2_500);
    expect(deriveGame(VIEW, NO_USAGE, []).money).toBe(2 * 40_000 + 3 * 2_500);
  });

  it('Board ohne Tickets: Fortschritt null', () => {
    const empty: BoardView = { boardId: 1, columns: [{ id: 'backlog', title: 'Backlog', tickets: [] }] };
    expect(deriveGame(empty, NO_USAGE, [])).toMatchObject({ loaded: true, total: 0, boardProgress: null, money: 0 });
  });

  it('Spalten in Reihenfolge des Boards, ohne not_needed', () => {
    expect(deriveGame(VIEW, NO_USAGE, []).columns).toEqual([
      { id: 'backlog', title: 'Backlog', count: 1 },
      { id: 'in_progress', title: 'In Arbeit', count: 2 },
      { id: 'blocked', title: 'Blockiert', count: 1 },
      { id: 'done', title: 'Erledigt', count: 2 },
    ]);
  });

  it('Kontingente: Rest als 0..1, begrenzt; ohne Verbrauch null', () => {
    expect(deriveGame(VIEW, usage(25, 90), [])).toMatchObject({ quota5h: 0.75, quota7d: expect.closeTo(0.1) });
    expect(deriveGame(VIEW, usage(130, -10), [])).toMatchObject({ quota5h: 0, quota7d: 1 });
    expect(deriveGame(VIEW, { ...usage(10, 10), available: false }, [])).toMatchObject({ quota5h: null, quota7d: null });
    expect(deriveGame(VIEW, { ...usage(10, 10), sevenDay: null }, [])).toMatchObject({ quota5h: 0.9, quota7d: null });
  });

  it('Badges von „Meine Aufgaben“ und „Fragen“', () => {
    expect(deriveGame(null, NO_USAGE, nav({ inbox: 4, questions: 2, board: 9 }))).toMatchObject({ tasks: 4, questions: 2 });
    expect(deriveGame(null, NO_USAGE, nav({}))).toMatchObject({ tasks: 0, questions: 0 });
  });

  it('Blips für offene Tickets mit fester Position, blockierte markiert', () => {
    const blips = deriveGame(VIEW, NO_USAGE, []).blips;
    expect(blips.map((b) => [b.id, b.column, b.blocked])).toEqual([
      ['N-1', 'backlog', false],
      ['N-2', 'in_progress', false],
      ['N-3', 'in_progress', false],
      ['N-4', 'blocked', true],
    ]);
    expect(blips[0]).toMatchObject({ title: 'Titel N-1', ...blipPosition('N-1') });
  });
});

describe('blipPosition', () => {
  const ids = Array.from({ length: 400 }, (_, i) => `NOVA-${String(i).padStart(4, '0')}`);

  it('ist fest: dieselbe ID ergibt denselben Punkt', () => {
    expect(blipPosition('NOVA-0005')).toEqual(blipPosition('NOVA-0005'));
    expect(blipPosition('NOVA-0005')).not.toEqual(blipPosition('NOVA-0006'));
  });

  it('liegt immer im Kreis mit Radius 0,42 um die Mitte', () => {
    for (const id of [...ids, '', 'ü', 'x'.repeat(200)]) {
      const { x, y } = blipPosition(id);
      expect(Math.hypot(x - 0.5, y - 0.5), id).toBeLessThanOrEqual(0.42 + 1e-9);
    }
  });

  it('verteilt gut: jeder Quadrant bekommt einen fairen Teil', () => {
    const quadrants = [0, 0, 0, 0];
    for (const id of ids) {
      const { x, y } = blipPosition(id);
      quadrants[(x < 0.5 ? 0 : 1) + (y < 0.5 ? 0 : 2)]!++;
    }
    for (const n of quadrants) expect(n).toBeGreaterThan(60);
    const distinct = new Set(ids.map((id) => JSON.stringify(blipPosition(id))));
    expect(distinct.size).toBe(ids.length);
  });
});
