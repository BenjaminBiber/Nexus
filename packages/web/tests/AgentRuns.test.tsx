// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { act, cleanup, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { ApiProvider } from '../src/api/ApiContext.js';
import type { DispatchRunLogDto, DispatchRunLogPartsDto } from '../src/api/dispatchTypes.js';
import { AgentRuns } from '../src/components/db/AgentRuns.js';
import { DETAIL, openPanel } from './helpers/dbPanel.js';
import { fakeApi } from './helpers/fakeApi.js';

vi.mock('mermaid', () => ({ default: { initialize: vi.fn(), render: vi.fn(async () => ({ svg: '<svg></svg>' })) } }));
afterEach(cleanup);

const NEW = 'ARC-0001-20261016-101500.log';
const OLD = 'ARC-0001-20261015-090000-merge.log';
const run = (name: string, kind: DispatchRunLogDto['kind'], running: boolean): DispatchRunLogDto => ({ name, kind, started: '2026-10-16T08:15:00.000Z', running });
const parts = (name: string, body: string, prompt: string | null = 'Du bist junior-ARC-0001. Setze ARC-0001 um.'): DispatchRunLogPartsDto =>
  ({ name, header: '[dispatch] junior-ARC-0001 startet', prompt, body });

describe('Reiter „Agent“ im Ticket-Panel', () => {
  it('steht nach „Verlauf“ und zeigt die Läufe, den neuesten offen, die Arbeitsanweisung getrennt', async () => {
    const getDispatchRuns = vi.fn(async () => [run(NEW, 'work', false), run(OLD, 'merge', false)]);
    const getDispatchRunLog = vi.fn(async (name: string) => parts(name, name === NEW ? '09:00:01  Claude startet (opus)\n' : 'Merge'));
    openPanel({ getDispatchRuns, getDispatchRunLog });
    const tabs = (await screen.findAllByRole('tab')).map((t) => t.textContent);
    expect(tabs.slice(-2)).toEqual(['Verlauf', 'Agent']);
    await userEvent.click(screen.getByRole('tab', { name: 'Agent' }));
    expect(getDispatchRuns).toHaveBeenCalledWith('ARC-0001');
    await screen.findByText(/Lauf vom/);
    const runs = [...document.querySelectorAll<HTMLDetailsElement>('details.agent-run')];
    const newest = runs.find((d) => d.textContent?.includes('Lauf vom'))!;
    expect((newest as HTMLDetailsElement).open).toBe(true);
    expect(within(newest).getByText('beendet')).toBeTruthy();
    const log = await within(newest).findByText(/Claude startet \(opus\)/);
    expect(log.closest('.dispatch-log')).not.toBeNull();
    expect(log.textContent).not.toContain('Setze ARC-0001 um');
    const prompt = within(newest).getByText('Arbeitsanweisung an den Agent').closest('details')!;
    expect(prompt.open).toBe(false);
    expect(within(prompt).getByText(/Setze ARC-0001 um/)).toBeTruthy();
    const merge = runs.find((d) => d.textContent?.includes('Merge-Agent vom'))!;
    expect((merge as HTMLDetailsElement).open).toBe(false);
    expect(getDispatchRunLog).toHaveBeenCalledTimes(1);
    expect(getDispatchRunLog).toHaveBeenCalledWith(NEW);
  });

  it('meldet, wenn es noch keinen Lauf gab', async () => {
    openPanel();
    await userEvent.click(await screen.findByRole('tab', { name: 'Agent' }));
    expect(await screen.findByText('Zu diesem Ticket gab es noch keinen Agent-Lauf.')).toBeTruthy();
  });

  it('zeigt einen Punkt am Reiter, solange ein Lauf läuft oder ein Agent das Ticket in Arbeit hat', async () => {
    openPanel({ getDispatchRuns: async () => [run(NEW, 'work', true)] });
    const tab = await screen.findByRole('tab', { name: 'Agent' });
    await waitFor(() => expect(tab.querySelector('.db-tab-dot')).not.toBeNull());
    cleanup();
    openPanel({}, { ...DETAIL, status: 'in_progress', column: 'in_progress', assignee: 'junior-ARC-0001' }, [], { agentPrefix: 'junior-' });
    expect((await screen.findByRole('tab', { name: 'Agent' })).querySelector('.db-tab-dot')).not.toBeNull();
    cleanup();
    openPanel({}, { ...DETAIL, status: 'in_progress', column: 'in_progress', assignee: 'Florian' }, [], { agentPrefix: 'junior-' });
    const plain = await screen.findByRole('tab', { name: 'Agent' });
    await act(async () => { await Promise.resolve(); });
    expect(plain.querySelector('.db-tab-dot')).toBeNull();
  });
});

describe('AgentRuns (Live)', () => {
  it('lädt alle 3 s nach, solange ein Lauf läuft, und hört danach auf; der Zeitgeber endet beim Schließen', async () => {
    let running = true;
    let body = '09:00:01  Claude startet (opus)\n';
    const getDispatchRuns = vi.fn(async () => [run(NEW, 'work', running)]);
    const getDispatchRunLog = vi.fn(async (name: string) => parts(name, body));
    vi.useFakeTimers({ shouldAdvanceTime: true });
    try {
      const view = render(
        <ApiProvider api={fakeApi({ getDispatchRuns, getDispatchRunLog })}>
          <AgentRuns id="ARC-0001" />
        </ApiProvider>,
      );
      expect(await screen.findByText('läuft')).toBeTruthy();
      await screen.findByText(/Claude startet/);
      body += '09:00:05  Claude: Ich lese das Ticket.\n';
      await act(async () => { await vi.advanceTimersByTimeAsync(3000); });
      expect(await screen.findByText(/Ich lese das Ticket/)).toBeTruthy();
      expect(getDispatchRuns).toHaveBeenCalledTimes(2);
      running = false;
      body += '09:10:00  fertig, 3 Schritte, 10 min\n';
      await act(async () => { await vi.advanceTimersByTimeAsync(3000); });
      expect(await screen.findByText('beendet')).toBeTruthy();
      expect(await screen.findByText(/fertig, 3 Schritte/)).toBeTruthy();
      const calls = getDispatchRuns.mock.calls.length;
      await act(async () => { await vi.advanceTimersByTimeAsync(10_000); });
      expect(getDispatchRuns).toHaveBeenCalledTimes(calls);

      // Erneut laufend, dann ausgehängt: kein weiteres Nachladen
      running = true;
      view.rerender(
        <ApiProvider api={fakeApi({ getDispatchRuns, getDispatchRunLog })}>
          <AgentRuns id="ARC-0002" />
        </ApiProvider>,
      );
      await act(async () => { await vi.advanceTimersByTimeAsync(100); });
      view.unmount();
      const after = getDispatchRuns.mock.calls.length;
      await act(async () => { await vi.advanceTimersByTimeAsync(10_000); });
      expect(getDispatchRuns).toHaveBeenCalledTimes(after);
    } finally {
      vi.useRealTimers();
    }
  });

  it('bleibt beim Nachladen am Ende kleben, außer der Nutzer hat hochgescrollt', async () => {
    localStorage.setItem('harness.agentRuns.view', 'detailed');
    let body = 'Zeile-1\n';
    const getDispatchRunLog = vi.fn(async (name: string) => parts(name, body));
    vi.useFakeTimers({ shouldAdvanceTime: true });
    try {
      render(
        <ApiProvider api={fakeApi({ getDispatchRuns: async () => [run(NEW, 'work', true)], getDispatchRunLog })}>
          <AgentRuns id="ARC-0001" />
        </ApiProvider>,
      );
      const pre = (await screen.findByText(/Zeile-1/)) as HTMLPreElement;
      // jsdom rechnet kein Layout: Höhe wächst mit den Zeilen, Scrollstand von Hand
      let top = 0;
      Object.defineProperty(pre, 'scrollHeight', { configurable: true, get: () => 100 * (pre.textContent ?? '').split('\n').length });
      Object.defineProperty(pre, 'clientHeight', { configurable: true, get: () => 50 });
      Object.defineProperty(pre, 'scrollTop', { configurable: true, get: () => top, set: (v: number) => { top = v; } });
      top = pre.scrollHeight - 50 - 10;
      body += 'Zeile-2\n';
      await act(async () => { await vi.advanceTimersByTimeAsync(3000); });
      await screen.findByText(/Zeile-2/);
      expect(top).toBe(pre.scrollHeight);
      top = 0;
      body += 'Zeile-3\n';
      await act(async () => { await vi.advanceTimersByTimeAsync(3000); });
      await screen.findByText(/Zeile-3/);
      expect(top).toBe(0);
    } finally {
      vi.useRealTimers();
      localStorage.clear();
    }
  });

  it('zeigt kompakt Schritte mit farbigen Kreisen und detailliert die genauen Befehle, umschaltbar und gemerkt', async () => {
    const body = [
      '09:00:01  [start] Claude startet (opus)',
      '09:00:02  [say] Ich lese das Ticket.',
      '09:00:03  [run#t1] liest Ticket ARC-0001\t"$ harness board show ARC-0001"',
      '09:00:04  [ok#t1] ',
      '09:00:05  [edit#t2] ändert a.tsx\t"Edit C:/wt/a.tsx"',
      '09:00:06  [fail#t2] String nicht gefunden\t"String nicht gefunden"',
      '09:00:07  [test#t3] Tests (alle)\t"$ npm test"',
    ].join('\n');
    render(
      <ApiProvider api={fakeApi({ getDispatchRuns: async () => [run(NEW, 'work', true)], getDispatchRunLog: async (name: string) => parts(name, body) })}>
        <AgentRuns id="ARC-0001" />
      </ApiProvider>,
    );
    const step = (await screen.findByText('liest Ticket ARC-0001')).closest('li')!;
    expect(step.querySelector('.run-dot.ok')).not.toBeNull();
    expect(screen.getByText('ändert a.tsx').closest('li')!.querySelector('.run-dot.error')).not.toBeNull();
    expect(screen.getByText('Tests (alle)').closest('li')!.querySelector('.run-dot.running')).not.toBeNull();
    expect(screen.getByText('Ich lese das Ticket.').closest('li')!.querySelector('.run-dot')).toBeNull();
    expect(screen.queryByText(/harness board show/)).toBeNull();
    await userEvent.click(screen.getByRole('button', { name: 'Detailliert' }));
    expect(screen.getByText(/\$ harness board show ARC-0001/).tagName).toBe('PRE');
    expect(document.querySelector('.run-dot')).toBeNull();
    // Der Umschalter steht in der Kopfzeile des Laufs und klappt ihn nicht zu
    expect(screen.getByRole('button', { name: 'Detailliert' }).closest('summary')).not.toBeNull();
    expect(document.querySelector<HTMLDetailsElement>('details.agent-run')!.open).toBe(true);
    expect(localStorage.getItem('harness.agentRuns.view')).toBe('detailed');
    localStorage.clear();
  });

  it('lädt bei neuem refresh nach und klappt einen neu begonnenen Lauf auf', async () => {
    let list = [run(OLD, 'merge', false)];
    const getDispatchRuns = vi.fn(async () => list);
    const getDispatchRunLog = vi.fn(async (name: string) => parts(name, `Protokoll ${name}`));
    const api = fakeApi({ getDispatchRuns, getDispatchRunLog });
    const view = render(<ApiProvider api={api}><AgentRuns id="ARC-0001" refresh="planned" /></ApiProvider>);
    await screen.findByText(`Protokoll ${OLD}`);
    list = [run(NEW, 'work', true), ...list];
    view.rerender(<ApiProvider api={api}><AgentRuns id="ARC-0001" refresh="in_progress" /></ApiProvider>);
    expect(await screen.findByText(`Protokoll ${NEW}`)).toBeTruthy();
    const details = [...document.querySelectorAll<HTMLDetailsElement>('details.agent-run')];
    expect(details.map((d) => d.open)).toEqual([true, true]);
    expect(screen.getByText('läuft')).toBeTruthy();
  });

  it('zeigt bei langen Protokollen nur das Ende mit Hinweis und Fehler sichtbar', async () => {
    render(
      <ApiProvider api={fakeApi({ getDispatchRuns: async () => [run(NEW, 'research', false)], getDispatchRunLog: async (name: string) => parts(name, '…Ende', null) })}>
        <AgentRuns id="ARC-F0002" />
      </ApiProvider>,
    );
    expect(await screen.findByText(/Recherche vom/)).toBeTruthy();
    expect(await screen.findByText('Nur das Ende des Protokolls – es ist sehr lang.')).toBeTruthy();
    expect(screen.queryByText('Arbeitsanweisung an den Agent')).toBeNull();
    cleanup();
    render(
      <ApiProvider api={fakeApi({ getDispatchRuns: async () => { throw new Error('Server weg'); } })}>
        <AgentRuns id="ARC-0001" />
      </ApiProvider>,
    );
    expect((await screen.findByRole('alert')).textContent).toContain('Server weg');
  });
});
