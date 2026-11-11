// @vitest-environment jsdom
// GameProvider: Werte aus Feed und Host, Banner, Anruf, Cheats, Tasten je Stil, Einblendungen
import { act, cleanup, render, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { ThemeHostProvider, type ThemeHost } from '@nexus/theme-sdk';
import { CHEAT_MONEY, DECLINED_MS, GameProvider, MAX_WANTED, useGame, type Game } from '../src/data/GameContext.js';
import { STEP_REWARD, TICKET_REWARD } from '../src/data/derive.js';
import type { BoardView } from '../src/data/types.js';
import { BANNER_MS } from '../src/shared/banners.js';
import { fakeHost, fakeNav, fakeUsage, fakeView, NOVA_BOARD } from './helpers/fakes.js';

class FakeEventSource {
  static all: FakeEventSource[] = [];
  readyState = 1;
  onmessage: ((event: { data: string }) => void) | null = null;
  constructor(readonly url: string) {
    FakeEventSource.all.push(this);
  }
  close() {
    this.readyState = 2;
  }
}

let serverView: BoardView;
let game: Game;

function Probe() {
  game = useGame();
  return null;
}

const flush = () =>
  act(async () => {
    for (let i = 0; i < 20; i++) await Promise.resolve();
  });

// Host mit „Nova Shop“ als aktuellem Board; values = Einstellungen des Themes
function host(over: Partial<ThemeHost> = {}, values: Record<string, unknown> = {}): ThemeHost {
  const base = fakeHost();
  return fakeHost({
    boards: { ...base.boards, current: { project: 'p1', board: NOVA_BOARD } },
    themeSettings: { ...base.themeSettings, values },
    ...over,
  });
}

const tree = (value: ThemeHost) => (
  <ThemeHostProvider value={value}>
    <GameProvider pid="p1">
      <Probe />
    </GameProvider>
  </ThemeHostProvider>
);

async function mount(value: ThemeHost = host()) {
  const view = render(tree(value));
  await flush();
  return { ...view, update: async (next: ThemeHost) => {
    view.rerender(tree(next));
    await flush();
  } };
}

// Das Board ändert sich auf dem Server und meldet sich über den Ereignisstrom
async function serverChanges(view: BoardView) {
  serverView = view;
  act(() => FakeEventSource.all[FakeEventSource.all.length - 1]!.onmessage?.({ data: JSON.stringify({ type: 'dbboard', boardId: NOVA_BOARD }) }));
  await flush();
}

function press(keys: string) {
  for (const key of keys) act(() => void window.dispatchEvent(new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true })));
}

const tick = (ms: number) => act(() => vi.advanceTimersByTime(ms));

beforeEach(() => {
  serverView = fakeView();
  FakeEventSource.all = [];
  vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout', 'setInterval', 'clearInterval'] });
  vi.stubGlobal('fetch', vi.fn(async () => ({ ok: true, status: 200, json: async () => serverView })));
  vi.stubGlobal('EventSource', FakeEventSource);
});

afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
  vi.useRealTimers();
});

describe('GameProvider: Werte', () => {
  it('liest Board-Stand, Verbrauch, Navigation und Einstellungen', async () => {
    await mount(host({ usage: fakeUsage(25, 50) }, { style: 'sa', cheats: false }));
    expect(fetch).toHaveBeenCalledWith('/api/db/boards/1/view', expect.anything());
    expect(game).toMatchObject({
      pid: 'p1',
      loaded: true,
      total: 7,
      done: 2,
      blocked: 1,
      money: 2 * TICKET_REWARD + 3 * STEP_REWARD,
      moneyShown: 2 * TICKET_REWARD + 3 * STEP_REWARD,
      wanted: 1,
      quota5h: 0.75,
      quota7d: 0.5,
      tasks: 2,
      questions: 1,
      style: 'sa',
      boardName: 'Nova Shop',
      banner: null,
      phone: null,
      declined: false,
      zone: null,
      station: null,
      cheat: null,
      statsOpen: false,
    });
    expect(game.settings).toMatchObject({ style: 'sa', cheats: false, banners: true, loading: false });
    expect(game.active?.id).toBe('board');
    expect(game.nav.map((n) => n.id)).toContain('questions');
    expect(game.now).toBeInstanceOf(Date);
  });

  it('ohne Board: nichts geladen, kein Board-Name', async () => {
    await mount(fakeHost());
    expect(fetch).not.toHaveBeenCalled();
    expect(game).toMatchObject({ loaded: false, boardName: null, money: 0 });
  });

  it('der Wert bleibt stabil, solange sich nichts ändert', async () => {
    const value = host();
    const { update } = await mount(value);
    const before = game;
    await update(value);
    expect(game).toBe(before);
  });

  it('Statistik öffnen und schließen', async () => {
    await mount();
    act(() => game.setStatsOpen(true));
    expect(game.statsOpen).toBe(true);
  });
});

describe('GameProvider: Banner', () => {
  it('Ticket blockiert bzw. erledigt → Banner nacheinander in Reihenfolge des Boards', async () => {
    await mount();
    await serverChanges(fakeView({ 'NOVA-0003': { column: 'done' }, 'NOVA-0005': { column: 'blocked', blockedReason: 'Wartet auf Freigabe' } }));
    expect(game.banner).toEqual({ kind: 'busted', ticketId: 'NOVA-0005', title: 'Gutscheine', reason: 'Wartet auf Freigabe' });
    act(() => game.dismissBanner());
    expect(game.banner).toEqual({ kind: 'passed', ticketId: 'NOVA-0003', title: 'Kasse', reward: TICKET_REWARD });
    tick(BANNER_MS);
    expect(game.banner).toBeNull();
  });

  it('5-Stunden-Kontingent erreicht 100 % → wasted', async () => {
    const { update } = await mount(host({ usage: fakeUsage(90) }));
    await update(host({ usage: fakeUsage(100) }));
    expect(game.banner).toEqual({ kind: 'wasted' });
  });

  it('Schalter „Banner“ aus: keine Banner, kein Anruf', async () => {
    const { update } = await mount(host({}, { banners: false }));
    await serverChanges(fakeView({ 'NOVA-0003': { column: 'done' } }));
    await update(host({ nav: fakeNav('board', { questions: 3 }) }, { banners: false }));
    expect(game.banner).toBeNull();
    expect(game.phone).toBeNull();
  });
});

describe('GameProvider: Anruf', () => {
  it('neue Fragen klingeln 15 s', async () => {
    const { update } = await mount();
    await update(host({ nav: fakeNav('board', { questions: 3 }) }));
    expect(game.phone).toEqual({ count: 2 });
    tick(15_000);
    expect(game.phone).toBeNull();
  });

  it('annehmen führt zu den Fragen, ablehnen schließt nur', async () => {
    const value = host();
    const { update } = await mount(value);
    await update(host({ navigate: value.navigate, nav: fakeNav('board', { questions: 2 }) }));
    act(() => game.answerPhone());
    expect(value.navigate).toHaveBeenCalledWith('/p/p1/questions');
    expect(game.phone).toBeNull();
    await update(host({ navigate: value.navigate, nav: fakeNav('board', { questions: 3 }) }));
    act(() => game.dismissPhone());
    expect(game.phone).toBeNull();
    expect(value.navigate).toHaveBeenCalledTimes(1);
  });

  it('nach dem Ablehnen meldet declined kurz den abgelehnten Anruf, nach dem Annehmen nicht', async () => {
    const value = host();
    const { update } = await mount(value);
    expect(game.declined).toBe(false);
    await update(host({ navigate: value.navigate, nav: fakeNav('board', { questions: 2 }) }));
    act(() => game.answerPhone());
    expect(game.declined).toBe(false);
    await update(host({ navigate: value.navigate, nav: fakeNav('board', { questions: 3 }) }));
    act(() => game.dismissPhone());
    expect(game.declined).toBe(true);
    tick(DECLINED_MS);
    expect(game.declined).toBe(false);
  });
});

describe('GameProvider: Cheats', () => {
  it('HESOYAM gibt Geld, AEZAKMI löscht und FUGITIVE setzt die Sterne – bis sich blocked ändert', async () => {
    await mount();
    const money = game.money;
    press('HESOYAM');
    expect(game.moneyShown).toBe(money + CHEAT_MONEY);
    expect(game.cheat).toBe('money');
    press('TURTLE');
    expect(game.moneyShown).toBe(money + 2 * CHEAT_MONEY);
    press('AEZAKMI');
    expect(game.wanted).toBe(0);
    expect(game.cheat).toBe('clear');
    press('FUGITIVE');
    expect(game.wanted).toBe(MAX_WANTED);
    tick(3000);
    expect(game.cheat).toBeNull();
    await serverChanges(fakeView({ 'NOVA-0005': { column: 'blocked' } }));
    expect(game.wanted).toBe(2);
    expect(game.moneyShown).toBe(game.money + 2 * CHEAT_MONEY);
  });

  it('Schalter „Cheats“ aus: nichts passiert', async () => {
    await mount(host({}, { cheats: false }));
    press('HESOYAM');
    press('BRINGITON');
    expect(game.moneyShown).toBe(game.money);
    expect(game.wanted).toBe(1);
    expect(game.cheat).toBeNull();
  });
});

describe('GameProvider: Tasten', () => {
  it('Stil v: Q und E wechseln die Seite, am Rand umlaufend', async () => {
    const value = host({ nav: fakeNav('board') });
    const { update } = await mount(value);
    press('q');
    expect(value.navigate).toHaveBeenLastCalledWith('/p/p1/settings');
    press('e');
    expect(value.navigate).toHaveBeenLastCalledWith('/p/p1/dashboard');
    await update(host({ navigate: value.navigate, nav: fakeNav('settings') }));
    press('e');
    expect(value.navigate).toHaveBeenLastCalledWith('/p/p1/board');
    // Y/N gelten im Stil v nicht, auch wenn das Handy klingelt
    await update(host({ navigate: value.navigate, nav: fakeNav('settings', { questions: 4 }) }));
    expect(game.phone).not.toBeNull();
    press('y');
    expect(value.navigate).toHaveBeenCalledTimes(3);
  });

  it('Stil sa: kein Q/E; Y/N nur bei klingelndem Handy', async () => {
    const value = host({}, { style: 'sa' });
    const { update } = await mount(value);
    press('qeyn');
    expect(value.navigate).not.toHaveBeenCalled();
    await update(host({ navigate: value.navigate, nav: fakeNav('board', { questions: 2 }) }, { style: 'sa' }));
    press('n');
    expect(game.phone).toBeNull();
    expect(value.navigate).not.toHaveBeenCalled();
    await update(host({ navigate: value.navigate, nav: fakeNav('board', { questions: 3 }) }, { style: 'sa' }));
    press('y');
    expect(value.navigate).toHaveBeenCalledWith('/p/p1/questions');
    expect(game.phone).toBeNull();
  });
});

describe('GameProvider: Einblendungen', () => {
  it('zone nach einem Seitenwechsel (2,5 s), station nach einem Board-Wechsel (3 s)', async () => {
    const value = host();
    const { update } = await mount(value);
    expect(game.zone).toBeNull();
    await update(host({ nav: fakeNav('questions') }));
    expect(game.zone).toBe('Fragen');
    tick(2500);
    expect(game.zone).toBeNull();
    await update(host({ nav: fakeNav('questions'), boards: { ...value.boards, current: { project: 'p1', board: 2 } } }));
    expect(game.station).toBe('Nova App');
    tick(2999);
    expect(game.station).toBe('Nova App');
    tick(1);
    expect(game.station).toBeNull();
  });
});

describe('useGame', () => {
  it('außerhalb des Providers eine klare Fehlermeldung', () => {
    vi.spyOn(console, 'error').mockImplementation(() => {});
    expect(() => renderHook(() => useGame())).toThrow(/useGame braucht den GameProvider/);
    vi.restoreAllMocks();
  });
});
