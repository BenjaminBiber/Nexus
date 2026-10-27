// @vitest-environment jsdom
import { describe, it, expect, afterEach, vi } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';
import { ApiProvider } from '../src/api/ApiContext.js';
import { ClaudeUsageMeter, RETRY_MS, USAGE_SETTING_EVENT, resetLabel } from '../src/components/ClaudeUsageMeter.js';
import type { ClaudeUsageDto } from '../src/api/usageTypes.js';
import { fakeApi } from './helpers/fakeApi.js';

afterEach(() => {
  cleanup();
  localStorage.clear();
});

const usage = (extra: Partial<ClaudeUsageDto>): ClaudeUsageDto => ({ available: true, reason: null, plan: 'team', fiveHour: null, sevenDay: null, fetchedAt: '2026-10-20T08:55:00.000Z', ...extra });
const show = (value: ClaudeUsageDto | Error) =>
  render(<ApiProvider api={fakeApi({ getClaudeUsage: async () => { if (value instanceof Error) throw value; return value; } })}><ClaudeUsageMeter /></ApiProvider>);
const soon = new Date(Date.now() + (3 * 60 + 12) * 60_000 + 30_000);
const both = usage({ fiveHour: { percent: 79, resetsAt: soon.toISOString() }, sevenDay: { percent: 95, resetsAt: null } });

describe('ClaudeUsageMeter', () => {
  it('zeigt je Limit Name, Prozent, Balken mit Farbstufe und Zeitpunkt des Zurücksetzens', async () => {
    const { container } = show(both);
    const section = await screen.findByRole('region', { name: 'Claude-Nutzungslimits' });
    expect(section.textContent).toContain('Deine Nutzungslimits · team');
    expect(screen.getByText('Sitzungslimit')).toBeTruthy();
    expect(screen.getByText('Wöchentlich · alle Modelle')).toBeTruthy();
    expect(section.textContent).toContain('79 %');
    expect(section.textContent).toContain('95 %');
    expect(section.textContent).toContain('Zurücksetzung in 3 Std. 13 Min.');
    expect(screen.getByRole('progressbar', { name: 'Sitzungslimit' }).getAttribute('aria-valuenow')).toBe('79');
    expect(Array.from(container.querySelectorAll('.usage-fill')).map((f) => [f.getAttribute('class'), (f as HTMLElement).style.width])).toEqual([
      ['usage-fill warn', '79%'],
      ['usage-fill crit', '95%'],
    ]);
  });

  it('zeigt nur das vorhandene Limit und deckelt den Balken bei 100 %', async () => {
    const { container } = show(usage({ sevenDay: { percent: 130, resetsAt: null } }));
    await screen.findByRole('progressbar', { name: 'Wöchentlich · alle Modelle' });
    expect(screen.queryByText('Sitzungslimit')).toBeNull();
    expect((container.querySelector('.usage-fill') as HTMLElement).style.width).toBe('100%');
  });

  it('bleibt unsichtbar ohne Werte und bei Fehlern', async () => {
    const a = show(usage({ available: false, reason: 'no-login' }));
    await new Promise((r) => setTimeout(r, 10));
    expect(a.container.innerHTML).toBe('');
    cleanup();
    const b = show(new Error('kaputt'));
    await new Promise((r) => setTimeout(r, 10));
    expect(b.container.innerHTML).toBe('');
  });
});

describe('ClaudeUsageMeter Schalter', () => {
  it('lädt sofort neu, wenn die Einstellung sich ändert', async () => {
    let value: ClaudeUsageDto = usage({ available: false, reason: 'disabled' });
    const { container } = render(<ApiProvider api={fakeApi({ getClaudeUsage: async () => value })}><ClaudeUsageMeter /></ApiProvider>);
    await new Promise((r) => setTimeout(r, 10));
    expect(container.innerHTML).toBe('');
    value = both;
    window.dispatchEvent(new Event(USAGE_SETTING_EVENT));
    expect(await screen.findByRole('region', { name: 'Claude-Nutzungslimits' })).toBeTruthy();
  });
});

describe('ClaudeUsageMeter Wiederholung', () => {
  it('lädt neu, sobald der Tab wieder sichtbar wird', async () => {
    let value: ClaudeUsageDto = usage({ available: false, reason: 'request-failed' });
    const { container } = render(<ApiProvider api={fakeApi({ getClaudeUsage: async () => value })}><ClaudeUsageMeter /></ApiProvider>);
    await new Promise((r) => setTimeout(r, 10));
    expect(container.querySelector('.usage-meter:not(.usage-skel)')).toBeNull();
    value = both;
    document.dispatchEvent(new Event('visibilitychange'));
    expect(await screen.findByRole('region', { name: 'Claude-Nutzungslimits' })).toBeTruthy();
  });

  it('wiederholt nach einem vorübergehenden Fehlschlag schnell statt erst nach zwei Minuten', async () => {
    vi.useFakeTimers();
    try {
      let value: ClaudeUsageDto | Error = new Error('kaputt');
      render(<ApiProvider api={fakeApi({ getClaudeUsage: async () => { if (value instanceof Error) throw value; return value; } })}><ClaudeUsageMeter /></ApiProvider>);
      await vi.advanceTimersByTimeAsync(10);
      expect(screen.queryByRole('region', { name: 'Claude-Nutzungslimits' })).toBeNull();
      value = both;
      await vi.advanceTimersByTimeAsync(RETRY_MS + 10);
      expect(screen.queryByRole('region', { name: 'Claude-Nutzungslimits' })).toBeTruthy();
    } finally {
      vi.useRealTimers();
    }
  });

  it('behält den gemerkten Platz bei vorübergehendem Fehler und vergisst ihn bei fehlender Anmeldung', async () => {
    localStorage.setItem('harness:usage-ring', '1');
    show(new Error('kaputt'));
    await new Promise((r) => setTimeout(r, 10));
    expect(localStorage.getItem('harness:usage-ring')).toBe('1');
    cleanup();
    show(usage({ available: false, reason: 'no-login' }));
    await new Promise((r) => setTimeout(r, 10));
    expect(localStorage.getItem('harness:usage-ring')).toBe('0');
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
});
