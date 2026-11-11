// Test-Hilfen des GTA-Themes: vollständiger Host, Board „Nova Shop“, vollständiges Spiel und Rendern mit beidem.
// fakeHost() hat kein aktuelles Board, damit ein GameProvider ohne falsches fetch nichts lädt; mit Board:
// fakeHost({ boards: { ...fakeHost().boards, current: { project: 'p1', board: NOVA_BOARD } } }) und fetch stubben.
import type { ReactElement } from 'react';
import { render } from '@testing-library/react';
import { vi } from 'vitest';
import { ThemeHostProvider, type NavItem, type ThemeHost, type UsageState } from '@nexus/theme-sdk';
import { GameContext, type Game } from '../../src/data/GameContext.js';
import { deriveGame } from '../../src/data/derive.js';
import type { BoardView, Card, Column, ColumnId } from '../../src/data/types.js';
import { resolveSettings } from '../../src/settings.js';

export const NOVA_BOARD = 1;

const COLUMN_TITLES: Record<ColumnId, string> = {
  backlog: 'Backlog',
  waiting: 'Wartet',
  in_planning: 'In Planung',
  planned: 'Geplant',
  in_progress: 'In Arbeit',
  blocked: 'Blockiert',
  done: 'Erledigt',
  not_needed: 'Nicht nötig',
};

const card = (id: string, title: string, column: ColumnId, over: Partial<Card> = {}): Card => ({
  id,
  title,
  column,
  blockedReason: null,
  stepsDone: 0,
  stepsTotal: 0,
  priority: 'normal',
  assignee: null,
  ...over,
});

const NOVA_CARDS: Card[] = [
  card('NOVA-0001', 'Produktliste', 'done', { stepsDone: 4, stepsTotal: 4 }),
  card('NOVA-0002', 'Warenkorb', 'done', { stepsDone: 3, stepsTotal: 3 }),
  card('NOVA-0003', 'Kasse', 'in_progress', { stepsDone: 2, stepsTotal: 5, assignee: 'junior-NOVA-0003', priority: 'high' }),
  card('NOVA-0004', 'Zahlungsanbieter', 'blocked', { stepsDone: 1, stepsTotal: 4, blockedReason: 'Zugang zur Sandbox fehlt' }),
  card('NOVA-0005', 'Gutscheine', 'planned', { stepsTotal: 3 }),
  card('NOVA-0006', 'Wunschliste', 'backlog'),
  card('NOVA-0007', 'Versandkosten', 'waiting'),
  card('NOVA-0008', 'Alter Checkout', 'not_needed'),
];

/** Board „Nova Shop“; `changes` ändert Tickets nach ID (z. B. `{ 'NOVA-0003': { column: 'done' } }`), die Spalten folgen */
export function fakeView(changes: Record<string, Partial<Card>> = {}): BoardView {
  const cards = NOVA_CARDS.map((c) => ({ ...c, ...changes[c.id] }));
  const columns: Column[] = (Object.keys(COLUMN_TITLES) as ColumnId[]).map((id) => ({
    id,
    title: COLUMN_TITLES[id],
    tickets: cards.filter((c) => c.column === id),
  }));
  return { boardId: NOVA_BOARD, columns };
}

export function fakeNav(active: NavItem['id'] = 'board', badges: Partial<Record<NavItem['id'], number>> = { inbox: 2, questions: 1 }): NavItem[] {
  const items: [NavItem['id'], string][] = [
    ['board', 'Board'],
    ['dashboard', 'Dashboard'],
    ['inbox', 'Meine Aufgaben'],
    ['sessions', 'Sessions'],
    ['questions', 'Fragen'],
    ['docs', 'Docs'],
    ['guide', 'Anleitung'],
    ['settings', 'Einstellungen'],
  ];
  return items.map(([id, label]) => ({ id, label, href: `/p/p1/${id}`, active: id === active, badge: badges[id] }));
}

export const fakeUsage = (fiveHour = 42, sevenDay = 18): UsageState => ({
  enabled: true,
  available: true,
  reason: null,
  fiveHour: { percent: fiveHour, resetsAt: null },
  sevenDay: { percent: sevenDay, resetsAt: null },
});

/** Vollständiger Host wie im Kern, Bausteine als erkennbare Platzhalter */
export function fakeHost(overrides: Partial<ThemeHost> = {}): ThemeHost {
  return {
    nav: fakeNav(),
    boards: {
      current: null,
      projects: [{ id: 'p1', name: 'Nova' }],
      boards: [
        { id: NOVA_BOARD, project: 'p1', name: 'Nova Shop' },
        { id: 2, project: 'p1', name: 'Nova App' },
      ],
      loading: false,
      choose: vi.fn(),
    },
    usage: fakeUsage(),
    settings: { values: null, loading: true, update: vi.fn(async () => {}) },
    themeSettings: { values: {}, loading: false, set: vi.fn(async () => {}) },
    colorScheme: { scheme: 'dark', themeId: 'gta', mode: 'gta' },
    navigate: vi.fn(),
    openSearch: vi.fn(),
    parts: {
      search: <button type="button">teil-suche</button>,
      boardSwitcher: <span>teil-board</span>,
      usage: <span>teil-verbrauch</span>,
      navItems: {},
      outlet: <span>teil-seite</span>,
      connection: <span>teil-banner</span>,
      status: <span>teil-status</span>,
    },
    ...overrides,
  };
}

/** Vollständiges Spiel mit den Werten von „Nova Shop“; Aktionen sind vi.fn() */
export function fakeGame(overrides: Partial<Game> = {}): Game {
  const nav = fakeNav();
  const state = deriveGame(fakeView(), fakeUsage(), nav);
  return {
    ...state,
    pid: 'p1',
    settings: { ...resolveSettings({}), loading: false },
    style: 'v',
    boardName: 'Nova Shop',
    nav,
    active: nav.find((item) => item.active),
    now: new Date(2026, 10, 11, 9, 41),
    wanted: state.blocked,
    moneyShown: state.money,
    banner: null,
    dismissBanner: vi.fn(),
    phone: null,
    declined: false,
    dismissPhone: vi.fn(),
    answerPhone: vi.fn(),
    zone: null,
    station: null,
    cheat: null,
    statsOpen: false,
    setStatsOpen: vi.fn(),
    ...overrides,
  };
}

/** Rendert in Host und Spiel */
export function renderWithGame(ui: ReactElement, game: Game = fakeGame(), host: ThemeHost = fakeHost()) {
  return render(
    <ThemeHostProvider value={host}>
      <GameContext.Provider value={game}>{ui}</GameContext.Provider>
    </ThemeHostProvider>,
  );
}
