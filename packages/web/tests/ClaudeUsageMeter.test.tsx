// @vitest-environment jsdom
import { describe, it, expect, afterEach, vi } from 'vitest';
import { act, cleanup, render, screen } from '@testing-library/react';
import { MemoryRouter, Route, Routes } from 'react-router-dom';
import { ApiProvider } from '../src/api/ApiContext.js';
import { ClaudeUsageMeter, RETRY_MS, USAGE_SETTING_EVENT, initials, resetDetail, resetLabel } from '../src/components/ClaudeUsageMeter.js';
import type { AccountUsageEntryDto, ClaudeUsageDto, UsageOverviewDto } from '../src/api/usageTypes.js';
import { fakeApi } from './helpers/fakeApi.js';

afterEach(() => {
  cleanup();
  localStorage.clear();
});

const usage = (extra: Partial<ClaudeUsageDto>): ClaudeUsageDto => ({ available: true, reason: null, plan: 'team', fiveHour: null, sevenDay: null, fetchedAt: '2026-10-20T08:55:00.000Z', ...extra });
const entry = (id: string, value: ClaudeUsageDto, extra: Partial<AccountUsageEntryDto> = {}): AccountUsageEntryDto => ({ id, name: id, fallback: false, limitFiveHour: null, limitWeek: null, usage: value, ...extra });
const overview = (...accounts: AccountUsageEntryDto[]): UsageOverviewDto => ({ enabled: true, defaultLimit: 90, accounts });
const later = new Date(Date.now() + 3 * 3600_000).toISOString();
const both = usage({ fiveHour: { percent: 92, resetsAt: later }, sevenDay: { percent: 58, resetsAt: null } });
const three = overview(
  entry('Support', both, { limitFiveHour: 90, limitWeek: 75 }),
  entry('Privat', usage({ fiveHour: { percent: 8, resetsAt: null }, sevenDay: { percent: 71, resetsAt: null } }), { limitWeek: 60 }),
  entry('Benjamin', usage({ fiveHour: { percent: 25, resetsAt: null }, sevenDay: { percent: 36, resetsAt: null } }), { limitFiveHour: 80, limitWeek: 70 }),
);

const mount = (getUsageOverview: () => Promise<UsageOverviewDto>, variant: 'compact' | 'full' = 'full') =>
  render(
    <ApiProvider api={fakeApi({ getUsageOverview })}>
      <MemoryRouter initialEntries={['/p/p1/board']}>
        <Routes><Route path="/p/:pid/*" element={<ClaudeUsageMeter variant={variant} />} /></Routes>
      </MemoryRouter>
    </ApiProvider>,
  );
const show = (value: UsageOverviewDto | Error, variant: 'compact' | 'full' = 'full') => mount(async () => { if (value instanceof Error) throw value; return value; }, variant);
const region = () => screen.queryByRole('region', { name: 'Claude-Nutzungslimits' });
// Stellt die Fake-Uhr vor und wartet in act() ab, bis React fertig gerendert hat. Fake-Timer steuern nur die Uhr: React
// rendert danach über seinen eigenen Scheduler (echtes setImmediate), und ohne act() prüft der Test gelegentlich den alten Stand
const advance = (ms: number) => act(() => vi.advanceTimersByTimeAsync(ms));

describe('ClaudeUsageMeter', () => {
  it('zeigt mit Text je aktivem Account Ring, Name und Werte; über der Grenze orange, ab 85 % rot', async () => {
    const { container } = show(three);
    const section = await screen.findByRole('region', { name: 'Claude-Nutzungslimits' });
    expect(section.textContent).not.toContain('Strich = Grenze');
    expect(screen.getByRole('group', { name: 'Support' }).textContent).toBe('SUSupport92/90 %W 58/75 %');
    expect(screen.getByRole('group', { name: 'Privat' }).textContent).toBe('PRPrivat8 %W 71/60 %');
    expect(screen.getByRole('group', { name: 'Benjamin' }).textContent).toBe('BEBenjamin25/80 %W 36/70 %');
    // Striche: Support zwei, Privat nur innen, Benjamin zwei
    expect(container.querySelectorAll('.usage-ring-tick')).toHaveLength(5);
    expect(Array.from(container.querySelectorAll('.usage-num.crit')).map((n) => n.textContent)).toEqual(['92']);
    expect(Array.from(container.querySelectorAll('.usage-num.over')).map((n) => n.textContent)).toEqual(['71']);
    expect(container.querySelector('.usage-ring-arc.outer.crit')?.getAttribute('stroke-dasharray')).toBe('92 100');
    expect(container.querySelector('.usage-ring-arc.inner.over')?.getAttribute('stroke-dasharray')).toBe('71 100');
  });

  it('wird ab 85 % rot, auch unter der Grenze; darunter orange erst ab der Vorgabe des Dispatchers', async () => {
    const { container } = show({ ...overview(
      entry('a', usage({ fiveHour: { percent: 86, resetsAt: null }, sevenDay: { percent: 80, resetsAt: null } }), { limitFiveHour: 95 }),
    ), defaultLimit: 75 });
    await screen.findByRole('group', { name: 'a' });
    expect(container.querySelector('.usage-ring-arc.outer.crit')).toBeTruthy();
    expect(container.querySelector('.usage-ring-arc.inner.over')).toBeTruthy();
  });

  it('zeigt kompakt nur die Ringe; die genauen Werte stehen in der Hover-Karte, der Ring führt zu den Accounts', async () => {
    const { container } = show(three, 'compact');
    await screen.findByRole('region', { name: 'Claude-Nutzungslimits' });
    expect(container.querySelectorAll('.usage-ring')).toHaveLength(3);
    expect(container.querySelector('.usage-acc-text')).toBeNull();
    expect(Array.from(container.querySelectorAll('.usage-ring-initials')).map((t) => t.textContent)).toEqual(['SU', 'PR', 'BE']);
    const support = screen.getByRole('group', { name: 'Support' });
    const card = support.querySelector('[role="tooltip"]')!;
    expect(card.textContent).toContain('Support · team');
    expect(card.textContent).toMatch(/Sitzung92 % \/ 90 %Reset in 3 Std\. \(\d\d:\d\d\)/);
    expect(card.textContent).toContain('Woche58 % / 75 %');
    expect(screen.getByRole('group', { name: 'Privat' }).querySelector('[role="tooltip"]')!.textContent).toContain('Sitzung8 % / 90 %');
    expect(screen.getByRole('link', { name: 'Support: alle Accounts' }).getAttribute('href')).toBe('/p/p1/settings#accounts');
  });

  it('lässt Accounts ohne Anmeldung weg und bleibt unsichtbar ohne Werte, bei Fehlern und ausgeschaltet', async () => {
    show(overview(entry('a', both), entry('b', usage({ available: false, reason: 'no-login' }))));
    await screen.findByRole('group', { name: 'a' });
    expect(screen.queryByRole('group', { name: 'b' })).toBeNull();
    cleanup();
    for (const value of [overview(entry('b', usage({ available: false, reason: 'no-login' }))), new Error('kaputt'), { enabled: false, defaultLimit: 90, accounts: [] }]) {
      const { container } = show(value);
      await new Promise((r) => setTimeout(r, 10));
      expect(container.innerHTML).toBe('');
      cleanup();
    }
  });
});

describe('ClaudeUsageMeter ohne Werte', () => {
  it('zeigt Accounts mit vorübergehendem Grund als grauen Ring mit Grund – auch wenn keiner Werte hat', async () => {
    const { container } = show(overview(
      entry('Arbeit-Max', usage({ available: false, reason: 'token-expired', plan: 'max' })),
      entry('Privat-Max', usage({ available: false, reason: 'rate-limited' })),
    ));
    const card = await screen.findByRole('group', { name: 'Arbeit-Max' });
    expect(card.textContent).toBe('AMArbeit-MaxAnmeldung abgelaufen – beim nächsten Lauf erneuert Claude Code sie');
    expect(screen.getByRole('group', { name: 'Privat-Max' }).textContent).toContain('Anthropic drosselt die Abfrage');
    expect(container.querySelectorAll('.usage-ring.empty')).toHaveLength(2);
    expect(localStorage.getItem('harness:usage-ring')).toBe('1');
  });

  it('kompakt: grauer Ring, Grund in der Hover-Karte, neben einem Account mit Werten', async () => {
    show(overview(entry('a', both), entry('b', usage({ available: false, reason: 'token-expired' }))), 'compact');
    const dot = await screen.findByRole('group', { name: 'b' });
    expect(dot.className).toBe('usage-dot stale');
    expect(dot.querySelector('.usage-pop-why')?.textContent).toBe('Anmeldung abgelaufen – beim nächsten Lauf erneuert Claude Code sie');
    expect(screen.getByRole('group', { name: 'a' }).querySelector('.usage-pop-why')).toBeNull();
  });
});

describe('ClaudeUsageMeter Schalter', () => {
  it('lädt sofort neu, wenn die Einstellung sich ändert', async () => {
    let value: UsageOverviewDto = { enabled: false, defaultLimit: 90, accounts: [] };
    const { container } = mount(async () => value);
    await new Promise((r) => setTimeout(r, 10));
    expect(container.innerHTML).toBe('');
    value = three;
    window.dispatchEvent(new Event(USAGE_SETTING_EVENT));
    expect(await screen.findByRole('region', { name: 'Claude-Nutzungslimits' })).toBeTruthy();
  });
});

describe('ClaudeUsageMeter verschwindet nicht', () => {
  it('behält den letzten Stand eines Accounts, wenn seine Abfrage kurz scheitert, und zeigt ihn gedimmt', async () => {
    vi.useFakeTimers();
    try {
      let value: UsageOverviewDto = three;
      let calls = 0;
      const { container } = mount(async () => { calls++; return value; });
      await advance(10);
      expect(region()).toBeTruthy();
      value = overview(entry('Support', usage({ available: false, reason: 'request-failed' }), { limitFiveHour: 90, limitWeek: 75 }));
      window.dispatchEvent(new Event(USAGE_SETTING_EVENT));
      await advance(10);
      expect(screen.getByRole('group', { name: 'Support' }).textContent).toBe('SUSupport92/90 %W 58/75 %');
      expect(container.querySelector('.usage-acc.stale')).toBeTruthy();
      // die schnelle Wiederholung holt frische Werte
      value = three;
      await advance(RETRY_MS + 10);
      expect(calls).toBe(3);
      await advance(100);
      expect(container.querySelector('.usage-acc.stale')).toBeNull();
    } finally {
      vi.useRealTimers();
    }
  });

  it('bleibt stehen, wenn der Server kurz nicht antwortet', async () => {
    vi.useFakeTimers();
    try {
      let value: UsageOverviewDto | Error = three;
      mount(async () => { if (value instanceof Error) throw value; return value; });
      await advance(10);
      value = new Error('weg');
      document.dispatchEvent(new Event('visibilitychange'));
      await advance(RETRY_MS * 3);
      expect(region()).toBeTruthy();
      expect(screen.getAllByRole('group')).toHaveLength(3);
    } finally {
      vi.useRealTimers();
    }
  });

  it('zählt einen alten Stand nach dem Zurücksetzen seines Fensters als 0 %', async () => {
    vi.useFakeTimers();
    try {
      const past = new Date(Date.now() + 60_000).toISOString();
      let value = overview(entry('a', usage({ fiveHour: { percent: 92, resetsAt: past }, sevenDay: { percent: 10, resetsAt: null } })));
      mount(async () => value);
      await advance(10);
      value = overview(entry('a', usage({ available: false, reason: 'token-expired' })));
      await advance(120_000 + 10);
      await advance(100);
      expect(screen.getByRole('group', { name: 'a' }).textContent).toBe('Aa0 %W 10 %');
    } finally {
      vi.useRealTimers();
    }
  });

  it('wiederholt nach einem vorübergehenden Fehlschlag schnell statt erst nach zwei Minuten', async () => {
    vi.useFakeTimers();
    try {
      let value: UsageOverviewDto | Error = new Error('kaputt');
      mount(async () => { if (value instanceof Error) throw value; return value; });
      await advance(10);
      expect(region()).toBeNull();
      value = three;
      await advance(RETRY_MS + 10);
      expect(region()).toBeTruthy();
    } finally {
      vi.useRealTimers();
    }
  });

  it('behält den gemerkten Platz bei vorübergehendem Fehler und vergisst ihn bei fehlender Anmeldung', async () => {
    localStorage.setItem('harness:usage-ring', '1');
    show(new Error('kaputt'), 'compact');
    await new Promise((r) => setTimeout(r, 10));
    expect(localStorage.getItem('harness:usage-ring')).toBe('1');
    cleanup();
    show(overview(entry('a', usage({ available: false, reason: 'no-login' }))), 'compact');
    await new Promise((r) => setTimeout(r, 10));
    expect(localStorage.getItem('harness:usage-ring')).toBe('0');
  });
});

describe('initials', () => {
  it('nimmt die Anfangsbuchstaben der Teile, bei einem Wort die ersten beiden, bei einer E-Mail den Teil vor dem @', () => {
    expect(initials('Privat-Max')).toBe('PM');
    expect(initials('Arbeit Max Pro')).toBe('AM');
    expect(initials('Support')).toBe('SU');
    expect(initials('ben.biber@example.com')).toBe('BB');
    expect(initials('über_alles')).toBe('ÜA');
    expect(initials('')).toBe('');
  });
});

describe('resetLabel', () => {
  const now = new Date(2026, 9, 20, 10, 0);
  it('zeigt innerhalb von 24 Stunden die Restzeit, danach Wochentag und Uhrzeit; leer bei fehlendem Zeitpunkt', () => {
    expect(resetLabel(new Date(2026, 9, 20, 13, 12).toISOString(), now)).toBe('in 3 Std. 12 Min.');
    expect(resetLabel(new Date(2026, 9, 20, 12, 0).toISOString(), now)).toBe('in 2 Std.');
    expect(resetLabel(new Date(2026, 9, 20, 10, 20).toISOString(), now)).toBe('in 20 Min.');
    expect(resetLabel(new Date(2026, 9, 22, 16, 0).toISOString(), now)).toMatch(/^am Do\., 16:00$/);
    expect(resetLabel(null, now)).toBe('');
    expect(resetLabel('kaputt', now)).toBe('');
  });

  it('hängt für die Hover-Karte innerhalb von 24 Stunden die Uhrzeit an', () => {
    expect(resetDetail(new Date(2026, 9, 20, 10, 25).toISOString(), now)).toBe('Reset in 25 Min. (10:25)');
    expect(resetDetail(new Date(2026, 9, 22, 16, 0).toISOString(), now)).toMatch(/^Reset am Do\., 16:00$/);
    expect(resetDetail(null, now)).toBe('');
  });
});
