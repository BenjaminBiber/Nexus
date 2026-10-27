// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { ApiProvider } from '../src/api/ApiContext.js';
import { ApiError } from '../src/api/ApiError.js';
import type { DispatchConfigViewDto, SuggestionListDto } from '../src/api/dispatchTypes.js';
import { OptimizationSection } from '../src/components/dispatch/OptimizationSection.js';
import { fakeApi, suggestion, suggestionList } from './helpers/fakeApi.js';

afterEach(cleanup);

const VIEW: DispatchConfigViewDto = { file: 'C:/h/dispatch.yaml', exists: true, raw: {}, config: null, error: null };
const LIST = suggestionList({
  basis: { runs: 142, projects: 4, since: '2026-04-25T00:00:00.000Z' },
  suggestions: [
    suggestion(),
    suggestion({ id: 'bbbbbbbbbbbbbbbb', source: 'claude', kind: 'claude', title: 'Regel 1 direkt auf gross', patch: [{ path: 'rules.0.profile', from: 'klein', to: 'gross' }] }),
    suggestion({ id: 'cccccccccccccccc', kind: 'context-full', title: 'Kontext läuft voll · gross', patch: null }),
  ],
  insufficient: [{ profile: 'klein', runs: 2, needed: 5 }],
  analysis: { at: '2026-10-22T08:00:00.000Z', costUsd: 0.04, stale: true, count: 1, skipped: 2 },
});

function open(overrides: Record<string, unknown> = {}, props: { locked?: boolean; onApplied?: (v: DispatchConfigViewDto) => void } = {}) {
  const api = fakeApi({ getDispatchSuggestions: async () => LIST, ...overrides });
  render(<ApiProvider api={api}><OptimizationSection locked={props.locked ?? false} onApplied={props.onApplied ?? (() => {})} /></ApiProvider>);
  return api;
}

describe('OptimizationSection', () => {
  it('zeigt Basis, Empfehlungen mit Änderung, Hinweise, Claude-Kennzeichen und den Fußbereich', async () => {
    open();
    expect(await screen.findByText(/142 Läufe aus 4 Projekten/)).toBeTruthy();
    const recommended = screen.getByRole('region', { name: 'Empfehlungen' });
    expect(within(recommended).getAllByRole('article').map((a) => a.getAttribute('aria-label'))).toEqual(['Schritte zu knapp · standard', 'Regel 1 direkt auf gross']);
    expect(within(recommended).getByText('standard: Schritte 80 → 120')).toBeTruthy();
    expect(within(recommended).getByText('Claude')).toBeTruthy();
    const hints = screen.getByRole('region', { name: 'Hinweise' });
    expect(within(hints).queryByRole('button', { name: 'Übernehmen' })).toBeNull();
    expect(screen.getByText('Zu wenig Daten: klein (2/5)')).toBeTruthy();
    expect(screen.getByText(/Letzte Claude-Analyse: .* · 1 Vorschlag · 0,04 \$ · 2 Antworten unbrauchbar · veraltet/)).toBeTruthy();
    expect(screen.getByText(/Claude liest die Kennzahlen/)).toBeTruthy();
  });

  it('übernimmt einen Vorschlag, meldet das Ergebnis und gibt die neue Einstellung weiter', async () => {
    const apply = vi.fn(async () => ({ view: VIEW, list: suggestionList() }));
    const onApplied = vi.fn();
    open({ applyDispatchSuggestion: apply }, { onApplied });
    const card = await screen.findByRole('article', { name: 'Schritte zu knapp · standard' });
    await userEvent.click(within(card).getByRole('button', { name: 'Übernehmen' }));
    expect(apply).toHaveBeenCalledWith('aaaaaaaaaaaaaaaa', 'hash1');
    expect(onApplied).toHaveBeenCalledWith(VIEW);
    expect(await screen.findByText('Übernommen: standard: Schritte 80 → 120')).toBeTruthy();
    expect(screen.getByText('Keine Auffälligkeiten – der Dispatcher läuft rund.')).toBeTruthy();
  });

  it('sperrt Übernehmen, solange das Formular ungespeicherte Änderungen hat', async () => {
    open({}, { locked: true });
    const card = await screen.findByRole('article', { name: 'Schritte zu knapp · standard' });
    expect((within(card).getByRole('button', { name: 'Übernehmen' }) as HTMLButtonElement).disabled).toBe(true);
  });

  it('lädt bei geänderter Einstellung neu und sagt Bescheid', async () => {
    const load = vi.fn(async () => LIST);
    open({
      getDispatchSuggestions: load,
      applyDispatchSuggestion: async () => { throw new ApiError('conflict', 'Die Dispatcher-Einstellung wurde inzwischen geändert.', 409); },
    });
    const card = await screen.findByRole('article', { name: 'Schritte zu knapp · standard' });
    await userEvent.click(within(card).getByRole('button', { name: 'Übernehmen' }));
    expect(await screen.findByText('Die Dispatcher-Einstellung wurde inzwischen geändert.')).toBeTruthy();
    expect(load).toHaveBeenCalledTimes(2);
  });

  it('blendet aus und zeigt Ausgeblendete auf Wunsch zum Wiedereinblenden', async () => {
    const hidden: SuggestionListDto = { ...LIST, suggestions: LIST.suggestions.slice(1), dismissed: [LIST.suggestions[0]!] };
    const dismiss = vi.fn(async () => hidden);
    const restore = vi.fn(async () => LIST);
    open({ dismissDispatchSuggestion: dismiss, restoreDispatchSuggestion: restore });
    const card = await screen.findByRole('article', { name: 'Schritte zu knapp · standard' });
    await userEvent.click(within(card).getByRole('button', { name: 'Ausblenden' }));
    expect(dismiss).toHaveBeenCalledWith('aaaaaaaaaaaaaaaa');
    await userEvent.click(await screen.findByRole('button', { name: '1 ausgeblendet · anzeigen' }));
    await userEvent.click(screen.getByRole('button', { name: 'Wieder einblenden' }));
    expect(restore).toHaveBeenCalledWith('aaaaaaaaaaaaaaaa');
  });

  it('startet die Claude-Analyse und zeigt Fehler an', async () => {
    let fail = false;
    const analyze = vi.fn(async () => { if (fail) throw new ApiError('analysis-failed', 'Antwort nicht lesbar: …', 422); return LIST; });
    open({ analyzeDispatchSuggestions: analyze });
    await userEvent.click(await screen.findByRole('button', { name: 'Tiefer analysieren' }));
    expect(analyze).toHaveBeenCalledTimes(1);
    fail = true;
    await userEvent.click(screen.getByRole('button', { name: 'Tiefer analysieren' }));
    expect((await screen.findByRole('alert')).textContent).toContain('Antwort nicht lesbar');
  });

  it('meldet nach der Analyse, was Claude gefunden hat', async () => {
    const result = suggestionList({
      suggestions: [
        suggestion({ id: 'bbbbbbbbbbbbbbbb', source: 'claude', kind: 'claude', title: 'Mehr Zeit', patch: [{ path: 'rules.0.profile', from: 'klein', to: 'gross' }] }),
        suggestion({ id: 'cccccccccccccccc', source: 'claude', kind: 'claude', title: 'Nur ein Hinweis', patch: null }),
      ],
      analysis: { at: '2026-10-23T08:00:00.000Z', costUsd: 0.04, stale: false, count: 2, skipped: 1 },
    });
    open({ analyzeDispatchSuggestions: async () => result });
    await userEvent.click(await screen.findByRole('button', { name: 'Tiefer analysieren' }));
    expect(await screen.findByText('Claude: 2 Vorschläge (1 Empfehlung, 1 Hinweis) · 0,04 $ · 1 Antwort unbrauchbar')).toBeTruthy();
  });

  it('sagt auch Bescheid, wenn Claude nichts zu verbessern findet', async () => {
    const result = suggestionList({ analysis: { at: '2026-10-23T08:00:00.000Z', costUsd: 0.04, stale: false, count: 0, skipped: 0 } });
    open({ analyzeDispatchSuggestions: async () => result });
    await userEvent.click(await screen.findByRole('button', { name: 'Tiefer analysieren' }));
    expect(await screen.findByText('Claude sieht keine Verbesserung – die Einstellung passt zu den Zahlen. · 0,04 $')).toBeTruthy();
  });

  it('zeigt während des Laufs eine Statuszeile und sperrt den Knopf', async () => {
    let finish: (l: SuggestionListDto) => void = () => {};
    open({ analyzeDispatchSuggestions: () => new Promise<SuggestionListDto>((resolve) => { finish = resolve; }) });
    const button = await screen.findByRole('button', { name: 'Tiefer analysieren' });
    await userEvent.click(button);
    const status = await screen.findByRole('status');
    expect(status.textContent).toContain('Claude analysiert');
    expect(status.textContent).toContain('von max. 5:00');
    expect((screen.getByRole('button', { name: 'Claude analysiert …' }) as HTMLButtonElement).disabled).toBe(true);
    finish(suggestionList({ analysis: { at: '2026-10-23T08:00:00.000Z', costUsd: null, stale: false, count: 0, skipped: 0 } }));
    expect(await screen.findByText('Claude sieht keine Verbesserung – die Einstellung passt zu den Zahlen.')).toBeTruthy();
    expect(screen.queryByRole('status')).toBeNull();
  });

  it('zeigt ohne Vorschläge den leeren Zustand', async () => {
    open({ getDispatchSuggestions: async () => suggestionList() });
    expect(await screen.findByText('Keine Auffälligkeiten – der Dispatcher läuft rund.')).toBeTruthy();
  });
});
