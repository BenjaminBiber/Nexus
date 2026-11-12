// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { act, cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { ApiProvider } from '../src/api/ApiContext.js';
import type { DispatchRunLogDto, DispatchRunLogPartsDto, DispatchRunShotDto } from '../src/api/dispatchTypes.js';
import type { TranscriptEntry } from '../src/api/sessionDetailTypes.js';
import { AgentRuns } from '../src/components/db/AgentRuns.js';
import { TRANSCRIPT_PAGE_SIZE } from '../src/hooks/useSessionDetail.js';
import { formatDuration } from '../src/sessions/format.js';
import { DETAIL, openPanel } from './helpers/dbPanel.js';
import { fakeApi, transcriptPage } from './helpers/fakeApi.js';

vi.mock('mermaid', () => ({ default: { initialize: vi.fn(), render: vi.fn(async () => ({ svg: '<svg></svg>' })) } }));
afterEach(cleanup);

const NEW = 'ARC-0001-20261016-101500.log';
const OLD = 'ARC-0001-20261015-090000-merge.log';
const AT = '2026-10-16T08:16:00.000Z';
const run = (name: string, kind: DispatchRunLogDto['kind'], running: boolean, extra: Partial<DispatchRunLogDto> = {}): DispatchRunLogDto =>
  ({ name, kind, started: '2026-10-16T08:15:00.000Z', running, ...extra });
const parts = (name: string, body: string, prompt: string | null = 'Du bist junior-ARC-0001. Setze ARC-0001 um.'): DispatchRunLogPartsDto =>
  ({ name, header: '[dispatch] junior-ARC-0001 startet', prompt, body });
// Eine Zeile des Lauf-Protokolls mit Text des Agenten
const say = (time: string, text: string) => `${time}  [say] ${text}\n`;
// Einträge eines Transcripts
const text = (id: string, value: string): TranscriptEntry => ({ kind: 'text', id, at: AT, text: value });
const tool = (id: string, name: string, summary: string, extra: Partial<Extract<TranscriptEntry, { kind: 'tool' }>> = {}): TranscriptEntry =>
  ({ kind: 'tool', id, at: AT, name, summary, input: null, result: { text: 'ok', isError: false, truncated: false }, ...extra });
const SHOT: DispatchRunShotDto = { file: '001.png', toolUseId: 't9', at: AT, mediaType: 'image/png', bytes: 12 };

describe('Reiter „Agent“ im Ticket-Panel', () => {
  it('steht nach „Verlauf“ und zeigt die Läufe, den neuesten offen, die Arbeitsanweisung getrennt', async () => {
    const getDispatchRuns = vi.fn(async () => [run(NEW, 'work', false), run(OLD, 'merge', false)]);
    const getDispatchRunLog = vi.fn(async (name: string) => parts(name, name === NEW ? '09:00:01  [start] Claude startet (opus)\n' : 'Merge'));
    openPanel({ getDispatchRuns, getDispatchRunLog });
    const tabs = (await screen.findAllByRole('tab')).map((t) => t.textContent);
    expect(tabs.slice(-2)).toEqual(['Verlauf', 'Agent']);
    await userEvent.click(screen.getByRole('tab', { name: 'Agent' }));
    expect(getDispatchRuns).toHaveBeenCalledWith('ARC-0001');
    await screen.findByText(/Lauf vom/);
    const runs = [...document.querySelectorAll<HTMLDetailsElement>('details.agent-run')];
    const newest = runs.find((d) => d.textContent?.includes('Lauf vom'))!;
    expect(newest.open).toBe(true);
    expect(within(newest).getByText('beendet')).toBeTruthy();
    const line = await within(newest).findByText(/Claude startet \(opus\)/);
    const scroll = line.closest('.run-scroll')!;
    expect(scroll).not.toBeNull();
    expect(scroll.textContent).not.toContain('Setze ARC-0001 um');
    const prompt = within(newest).getByText('Arbeitsanweisung an den Agent').closest('details')!;
    expect(prompt.open).toBe(false);
    expect(within(prompt).getByText(/Setze ARC-0001 um/)).toBeTruthy();
    const merge = runs.find((d) => d.textContent?.includes('Merge-Agent vom'))!;
    expect(merge.open).toBe(false);
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
    let body = '09:00:01  [start] Claude startet (opus)\n';
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
      body += say('09:00:05', 'Ich lese das Ticket.');
      await act(async () => { await vi.advanceTimersByTimeAsync(3000); });
      expect(await screen.findByText(/Ich lese das Ticket/)).toBeTruthy();
      expect(getDispatchRuns).toHaveBeenCalledTimes(2);
      running = false;
      body += '09:10:00  [done] fertig, 3 Schritte, 10 min\n';
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
    let body = say('09:00:01', 'Zeile-1');
    const getDispatchRunLog = vi.fn(async (name: string) => parts(name, body));
    vi.useFakeTimers({ shouldAdvanceTime: true });
    try {
      render(
        <ApiProvider api={fakeApi({ getDispatchRuns: async () => [run(NEW, 'work', true)], getDispatchRunLog })}>
          <AgentRuns id="ARC-0001" />
        </ApiProvider>,
      );
      const scroll = (await screen.findByText('Zeile-1')).closest<HTMLElement>('.run-scroll')!;
      // jsdom rechnet kein Layout: Höhe wächst mit den Einträgen, Scrollstand von Hand
      let top = 0;
      Object.defineProperty(scroll, 'scrollHeight', { configurable: true, get: () => 100 * scroll.querySelectorAll('.tx-entry').length });
      Object.defineProperty(scroll, 'clientHeight', { configurable: true, get: () => 50 });
      Object.defineProperty(scroll, 'scrollTop', { configurable: true, get: () => top, set: (v: number) => { top = v; } });
      // Nah am Ende: der Verlauf bleibt am Ende
      top = scroll.scrollHeight - 50 - 10;
      fireEvent.scroll(scroll);
      body += say('09:00:02', 'Zeile-2');
      await act(async () => { await vi.advanceTimersByTimeAsync(3000); });
      await screen.findByText('Zeile-2');
      expect(top).toBe(scroll.scrollHeight);
      // Hochgescrollt: die Stelle bleibt
      top = 0;
      fireEvent.scroll(scroll);
      body += say('09:00:03', 'Zeile-3');
      await act(async () => { await vi.advanceTimersByTimeAsync(3000); });
      await screen.findByText('Zeile-3');
      expect(top).toBe(0);
    } finally {
      vi.useRealTimers();
    }
  });

  it('zeigt ohne Transcript das Lauf-Protokoll als Verlauf, ohne Umschalter', async () => {
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
    expect(await screen.findByText('Ich lese das Ticket.')).toBeTruthy();
    const failed = screen.getByText('a.tsx').closest<HTMLElement>('.tx-tool')!;
    expect(failed.classList.contains('failed')).toBe(true);
    expect(within(failed).getByText('Fehler')).toBeTruthy();
    const pending = screen.getByText('Tests (alle)').closest<HTMLElement>('.tx-tool')!;
    expect(within(pending).getByText('ohne Ergebnis')).toBeTruthy();
    // Der genaue Befehl steht im aufgeklappten Aufruf
    expect(screen.queryByLabelText('Befehl')).toBeNull();
    await userEvent.click(screen.getByText('liest Ticket ARC-0001'));
    expect(screen.getByLabelText('Befehl').textContent).toBe('harness board show ARC-0001');
    // Die alte Darstellung ist weg
    expect(screen.queryByRole('button', { name: 'Detailliert' })).toBeNull();
    expect(screen.queryByRole('button', { name: 'Kompakt' })).toBeNull();
    expect(document.querySelector('.log-view-toggle')).toBeNull();
    expect(document.querySelector('.run-dot')).toBeNull();
    expect(document.querySelector<HTMLDetailsElement>('details.agent-run')!.open).toBe(true);
  });

  it('lädt bei neuem refresh nach und klappt einen neu begonnenen Lauf auf', async () => {
    let list = [run(OLD, 'merge', false)];
    const getDispatchRuns = vi.fn(async () => list);
    const getDispatchRunLog = vi.fn(async (name: string) => parts(name, say('09:00:01', name === OLD ? 'Alter Lauf' : 'Neuer Lauf')));
    const api = fakeApi({ getDispatchRuns, getDispatchRunLog });
    const view = render(<ApiProvider api={api}><AgentRuns id="ARC-0001" refresh="planned" /></ApiProvider>);
    await screen.findByText('Alter Lauf');
    list = [run(NEW, 'work', true), ...list];
    view.rerender(<ApiProvider api={api}><AgentRuns id="ARC-0001" refresh="in_progress" /></ApiProvider>);
    expect(await screen.findByText('Neuer Lauf')).toBeTruthy();
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
    expect(screen.getByRole('button', { name: 'Rohprotokoll' })).toBeTruthy();
    cleanup();
    render(
      <ApiProvider api={fakeApi({ getDispatchRuns: async () => { throw new Error('Server weg'); } })}>
        <AgentRuns id="ARC-0001" />
      </ApiProvider>,
    );
    expect((await screen.findByRole('alert')).textContent).toContain('Server weg');
  });
});

describe('AgentRuns (Transcript)', () => {
  it('zeigt den Verlauf aus dem Transcript, faltet unauffällige Schritte und nennt Ergebnis, Dauer und Modell', async () => {
    const entries: TranscriptEntry[] = [
      { kind: 'prompt', id: 'p1', at: AT, text: 'Du bist junior-ARC-0001. Setze ARC-0001 um.' },
      text('x1', 'Ich prüfe die Dateien.'),
      tool('r1', 'Read', 'a.ts'),
      tool('r2', 'Read', 'b.ts'),
      tool('g1', 'Grep', 'useApi'),
      tool('b1', 'Bash', 'npm test', { input: { command: 'npm test' }, result: { text: '1 failed', isError: true, truncated: false } }),
      text('x2', 'Die Tests scheitern.'),
    ];
    const getDispatchRunTranscript = vi.fn(async () => transcriptPage(entries));
    const ended = run(NEW, 'work', false, { outcome: 'abort', abortReason: 'Zeitlimit nach 60 min', durationMs: 720_000, model: 'claude-opus-5-5' });
    render(
      <ApiProvider api={fakeApi({
        getDispatchRuns: async () => [ended],
        getDispatchRunLog: async (name: string) => parts(name, say('09:00:02', 'Text aus dem Protokoll')),
        getDispatchRunTranscript,
      })}>
        <AgentRuns id="ARC-0001" />
      </ApiProvider>,
    );
    expect(await screen.findByText('Ich prüfe die Dateien.')).toBeTruthy();
    expect(screen.getByText('Die Tests scheitern.')).toBeTruthy();
    // Das Protokoll ist nur der Rückfall, die Arbeitsanweisung steht nicht im Verlauf
    expect(screen.queryByText('Text aus dem Protokoll')).toBeNull();
    expect(document.querySelector('.run-scroll .tx-prompt')).toBeNull();
    expect(getDispatchRunTranscript).toHaveBeenCalledWith(NEW, null, TRANSCRIPT_PAGE_SIZE);
    // Drei unauffällige Aufrufe sind eine zugeklappte Zeile
    const group = screen.getByText('3 Schritte · 2× Read · 1× Grep').closest('button')!;
    expect(group.getAttribute('aria-expanded')).toBe('false');
    expect(screen.queryByText('a.ts')).toBeNull();
    await userEvent.click(group);
    expect(screen.getByText('a.ts')).toBeTruthy();
    // Der gescheiterte Aufruf steht für sich
    expect(screen.getByText('npm test').closest('.tx-tool')!.classList.contains('failed')).toBe(true);
    // Kopf: Ergebnis, Dauer, Modell; darunter der Abbruchgrund
    expect(screen.getByText('Abbruch').className).toBe('badge outcome-error');
    expect(screen.getByText(`${formatDuration(720_000)} · opus-5-5`)).toBeTruthy();
    expect(screen.getByText('Zeitlimit nach 60 min').className).toBe('run-abort-reason');
  });

  it('zeigt Screenshots als Leiste und am Aufruf und öffnet die Großansicht', async () => {
    const entries = [text('x1', 'Ich sehe mir die Seite an.'), tool('t9', 'mcp__browser__screenshot', 'Startseite')];
    render(
      <ApiProvider api={fakeApi({
        getDispatchRuns: async () => [run(NEW, 'work', false, { outcome: 'done' })],
        getDispatchRunLog: async (name: string) => parts(name, ''),
        getDispatchRunTranscript: async () => transcriptPage(entries),
        getDispatchRunShots: async () => [SHOT],
      })}>
        <AgentRuns id="ARC-0001" />
      </ApiProvider>,
    );
    const strip = await screen.findByRole('group', { name: 'Screenshots des Laufs' });
    expect(within(strip).getByText('1 Screenshot')).toBeTruthy();
    const thumbs = screen.getAllByRole('button', { name: 'Screenshot 1 von 1 vergrößern' });
    expect(thumbs).toHaveLength(2);
    expect(thumbs.some((thumb) => thumb.closest('.run-shot-row') !== null)).toBe(true);
    await userEvent.click(within(strip).getByRole('button', { name: 'Screenshot 1 von 1 vergrößern' }));
    const dialog = screen.getByRole('dialog', { name: 'Screenshot 1 von 1' });
    expect(dialog.querySelector('img.run-shot-full')!.getAttribute('src')).toBe(`/shots/${NEW}/001.png`);
    await userEvent.click(within(dialog).getByRole('button', { name: 'Schließen' }));
    expect(screen.queryByRole('dialog')).toBeNull();
  });

  it('öffnet das unveränderte Protokoll über „Rohprotokoll“', async () => {
    const getDispatchLog = vi.fn(async () => 'ROH');
    render(
      <ApiProvider api={fakeApi({
        getDispatchRuns: async () => [run(NEW, 'work', false)],
        getDispatchRunLog: async (name: string) => parts(name, say('09:00:01', 'Fertig.')),
        getDispatchLog,
      })}>
        <AgentRuns id="ARC-0001" />
      </ApiProvider>,
    );
    await userEvent.click(await screen.findByRole('button', { name: 'Rohprotokoll' }));
    expect(await screen.findByRole('heading', { name: `Protokoll ${NEW}` })).toBeTruthy();
    expect(await screen.findByText('ROH')).toBeTruthy();
    expect(getDispatchLog).toHaveBeenCalledWith(NEW);
    await userEvent.click(screen.getByRole('button', { name: 'Schließen' }));
    expect(screen.queryByRole('dialog')).toBeNull();
  });
});
