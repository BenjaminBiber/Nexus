// @vitest-environment jsdom
import { describe, it, expect, afterEach, beforeEach, vi } from 'vitest';
import { act, cleanup, fireEvent, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter } from 'react-router-dom';
import { App } from '../src/App.js';
import { ApiProvider } from '../src/api/ApiContext.js';
import type { FileDiff, TranscriptEntry } from '../src/api/sessionDetailTypes.js';
import { mergeEntries, TRANSCRIPT_PAGE_SIZE } from '../src/hooks/useSessionDetail.js';
import { FakeEventSource } from './helpers/FakeEventSource.js';
import { fakeApi, sessionChanges, sessionDetail, sessionDto, transcriptPage } from './helpers/fakeApi.js';

beforeEach(() => FakeEventSource.reset());
afterEach(cleanup);

const SID = 'aaaaaaaa-1111-2222-3333-444444444444';
const session = sessionDto(SID, {
  title: 'Login bauen', model: 'claude-opus-4-1', tickets: ['T-0042'], ticketInfo: [{ id: 'T-0042', title: 'Login bauen', status: 'In Progress' }],
  client: 'nexus', runName: 'run-2026-11-04-T-0042',
});
const other = sessionDto('bbbbbbbb-2', { title: 'Anderes', state: 'idle', displayState: 'idle' });

const at = '2026-11-04T10:00:00.000Z';
const entries: TranscriptEntry[] = [
  { kind: 'prompt', id: 'e1', at, text: 'Bitte baue den Login.' },
  { kind: 'thinking', id: 'e2', at, text: 'Erst die Tests ansehen.', durationMs: 2000 },
  { kind: 'text', id: 'e3', at, text: 'Ich schaue mir **die Tests** an.' },
  { kind: 'tool', id: 'e4', at, name: 'Bash', summary: 'Bash: npm test', input: { command: 'npm test', description: 'Tests ausführen' }, result: { text: '12 passed', isError: false, truncated: false } },
  { kind: 'tool', id: 'e5', at, name: 'Read', summary: 'Read src/fehlt.ts', input: { file_path: 'src/fehlt.ts' }, result: { text: 'File does not exist.', isError: true, truncated: false } },
  { kind: 'system', id: 'e6', at, text: 'Kontext wurde kompaktiert.', level: 'info' },
];

const diff: FileDiff = {
  path: 'src/login.ts',
  truncated: false,
  hunks: [{ header: '@@ -1,2 +1,3 @@', lines: [{ kind: 'context', text: 'import x;' }, { kind: 'remove', text: 'alt();' }, { kind: 'add', text: 'neu();' }, { kind: 'add', text: 'mehr();' }] }],
};

// Rendert die ganze Oberfläche unter `path` mit austauschbaren Abrufen
function open(path: string, overrides: Parameters<typeof fakeApi>[0] = {}) {
  const calls = {
    getSessionDetail: vi.fn(async () => sessionDetail(session, {
      branch: 'ticket/T-0042-login', claudeVersion: '2.1.3', costUsd: 0.4231, durationMs: 95000,
      tokens: { input: 12345, output: 6789, cacheRead: 2_500_000, cacheWrite: 4000 },
      context: { usedTokens: 84000, limitTokens: 200000 },
      todos: [{ content: 'Tests schreiben', status: 'completed' }, { content: 'Login bauen', status: 'in_progress' }, { content: 'Doku', status: 'pending' }],
      run: { name: 'run-2026-11-04-T-0042', kind: 'implement', profile: 'standard', account: 'work', outcome: null, abortReason: null, startedAt: at, endedAt: null },
    })),
    getSessionTranscript: vi.fn(async (_pid: string, _sid: string, after?: string | null) => (after ? transcriptPage([], { cursor: after }) : transcriptPage(entries))),
    getSessionChanges: vi.fn(async () => sessionChanges({
      note: 'Änderungen per Bash sind nicht enthalten.',
      files: [{ path: 'src/login.ts', status: 'modified', added: 2, removed: 1 }, { path: 'src/neu.ts', status: 'added', added: 10, removed: 0 }],
    })),
    getSessionFileDiff: vi.fn(async () => diff),
    getDispatchLog: vi.fn(async () => 'Protokollzeile 1'),
  };
  const api = fakeApi({ getSessions: async () => [session, other], ...calls, ...overrides });
  render(
    <ApiProvider api={api}>
      <MemoryRouter initialEntries={[path]}>
        <App />
      </MemoryRouter>
    </ApiProvider>,
  );
  return { ...calls, ...overrides } as typeof calls;
}

const dialog = () => screen.findByRole('dialog', { name: 'Login bauen' });

describe('Session-Details', () => {
  it('öffnet per Klick auf den Titel, schließt mit Esc und gibt den Fokus zurück', async () => {
    open('/p/p1/sessions');
    const link = await screen.findByRole('link', { name: 'Login bauen · Session aaaaaaaa öffnen' });
    await userEvent.click(link);
    const panel = await dialog();
    expect(panel.getAttribute('aria-modal')).toBe('true');
    expect(within(panel.querySelector('header')!).getByText('Nexus')).toBeTruthy();
    expect(await within(panel).findByText('Bitte baue den Login.')).toBeTruthy();
    // Die Liste bleibt dahinter stehen
    expect(screen.getByRole('region', { name: 'T-0042 Login bauen' })).toBeTruthy();
    await userEvent.keyboard('{Escape}');
    expect(screen.queryByRole('dialog')).toBeNull();
    expect(document.activeElement).toBe(screen.getByRole('link', { name: 'Login bauen · Session aaaaaaaa öffnen' }));
  });

  it('öffnet direkt per URL und schließt über den Knopf zur Liste', async () => {
    open(`/p/p1/sessions/${SID}`);
    const panel = await dialog();
    const head = within(panel.querySelector('header')!);
    expect(await head.findByText('ticket/T-0042-login')).toBeTruthy();
    expect(head.getByText('0,42 $')).toBeTruthy();
    expect(head.getByRole('link', { name: 'T-0042' }).getAttribute('href')).toBe('/p/p1/board/t/T-0042');
    await userEvent.click(within(panel).getByRole('button', { name: 'Schließen' }));
    expect(screen.queryByRole('dialog')).toBeNull();
    expect(await screen.findByRole('heading', { name: 'Sessions' })).toBeTruthy();
  });

  it('wechselt die Tabs per Klick und Pfeiltasten', async () => {
    open(`/p/p1/sessions/${SID}`);
    const panel = await dialog();
    const tabs = within(panel).getByRole('tablist', { name: 'Session-Details' });
    expect(within(tabs).getByRole('tab', { name: 'Verlauf' }).getAttribute('aria-selected')).toBe('true');
    await userEvent.click(within(tabs).getByRole('tab', { name: 'Lauf' }));
    expect(within(tabs).getByRole('tab', { name: 'Lauf' }).getAttribute('aria-selected')).toBe('true');
    const run = within(panel).getByRole('tabpanel', { name: 'Lauf' });
    expect(within(run).getByText('12.345')).toBeTruthy();
    expect(within(run).getByText('2,5 Mio.')).toBeTruthy();
    expect(within(run).getByText('1 von 3 erledigt')).toBeTruthy();
    expect(within(run).getByRole('img', { name: 'Kontextauslastung 42 %' })).toBeTruthy();
    expect(within(run).getByText('implement')).toBeTruthy();
    // Protokoll des Nexus-Laufs als Dialog
    await userEvent.click(within(run).getByRole('button', { name: 'Protokoll öffnen' }));
    expect(await screen.findByText('Protokollzeile 1')).toBeTruthy();
    await userEvent.keyboard('{Escape}');
    expect(screen.queryByText('Protokollzeile 1')).toBeNull();
    expect(screen.getByRole('dialog', { name: 'Login bauen' })).toBeTruthy();
    // Pfeil nach links: Änderungen
    within(tabs).getByRole('tab', { name: 'Lauf' }).focus();
    await userEvent.keyboard('{ArrowLeft}');
    const changesTab = within(tabs).getByRole('tab', { name: 'Änderungen (2)' });
    expect(changesTab.getAttribute('aria-selected')).toBe('true');
    expect(document.activeElement).toBe(changesTab);
  });

  it('zeigt Tool-Aufrufe eingeklappt mit Kurzinfo und aufgeklappt mit Befehl und Ergebnis; Fehler sind markiert', async () => {
    open(`/p/p1/sessions/${SID}`);
    const panel = await dialog();
    const bash = await within(panel).findByRole('button', { name: /Bash.*npm test/ });
    expect(bash.getAttribute('aria-expanded')).toBe('false');
    expect(within(panel).queryByText('12 passed')).toBeNull();
    await userEvent.click(bash);
    expect(bash.getAttribute('aria-expanded')).toBe('true');
    expect(within(panel).getByLabelText('Befehl').textContent).toBe('npm test');
    expect(within(panel).getByText('Tests ausführen')).toBeTruthy();
    expect(within(panel).getByText('12 passed')).toBeTruthy();
    const failed = within(panel).getByRole('button', { name: /Read.*src\/fehlt\.ts.*Fehler/ });
    expect(failed.closest('.tx-tool')!.className).toContain('failed');
    // Antwort als Markdown, Systemhinweis dezent
    expect(within(panel).getByText('die Tests').tagName).toBe('STRONG');
    expect(within(panel).getByText('Kontext wurde kompaktiert.')).toBeTruthy();
  });

  it('klappt Thinking erst auf Wunsch auf', async () => {
    open(`/p/p1/sessions/${SID}`);
    const panel = await dialog();
    const thinking = await within(panel).findByRole('button', { name: 'Nachgedacht · 2 s' });
    expect(within(panel).queryByText('Erst die Tests ansehen.')).toBeNull();
    await userEvent.click(thinking);
    expect(within(panel).getByText('Erst die Tests ansehen.')).toBeTruthy();
  });

  it('lädt bei session-activity dieser Session den Verlauf ab dem Cursor nach, andere Sessions bleiben unbeachtet', async () => {
    const calls = open(`/p/p1/sessions/${SID}`);
    const panel = await dialog();
    await within(panel).findByText('Bitte baue den Login.');
    calls.getSessionTranscript.mockImplementation(async (_pid: string, _sid: string, after?: string | null) =>
      after === 'c1' ? transcriptPage([{ kind: 'text', id: 'e7', at, text: 'Alle Tests grün.' }], { cursor: 'c2' }) : transcriptPage([], { cursor: after ?? 'c1' }),
    );
    const before = { transcript: calls.getSessionTranscript.mock.calls.length, detail: calls.getSessionDetail.mock.calls.length, changes: calls.getSessionChanges.mock.calls.length };
    act(() => FakeEventSource.current().emit({ type: 'session-activity', sessionId: 'bbbbbbbb-2' }));
    expect(calls.getSessionTranscript.mock.calls.length).toBe(before.transcript);
    act(() => FakeEventSource.current().emit({ type: 'session-activity', sessionId: SID }));
    expect(await within(panel).findByText('Alle Tests grün.')).toBeTruthy();
    expect(calls.getSessionTranscript.mock.calls.at(-1)?.[2]).toBe('c1');
    await vi.waitFor(() => expect(calls.getSessionDetail.mock.calls.length).toBeGreaterThan(before.detail));
    await vi.waitFor(() => expect(calls.getSessionChanges.mock.calls.length).toBeGreaterThan(before.changes));
    // Bekannte Einträge bleiben einmal stehen
    expect(within(panel).getAllByText('Bitte baue den Login.')).toHaveLength(1);
  });

  it('zeigt „Neue Einträge ↓“, wenn der Nutzer nicht am Ende ist', async () => {
    const calls = open(`/p/p1/sessions/${SID}`);
    const panel = await dialog();
    await within(panel).findByText('Bitte baue den Login.');
    const scroller = within(panel).getByLabelText('Verlauf der Session');
    Object.defineProperty(scroller, 'scrollHeight', { configurable: true, value: 2000 });
    Object.defineProperty(scroller, 'clientHeight', { configurable: true, value: 400 });
    scroller.scrollTop = 100;
    fireEvent.scroll(scroller);
    calls.getSessionTranscript.mockImplementation(async () => transcriptPage([{ kind: 'text', id: 'e8', at, text: 'Neu dazu.' }], { cursor: 'c2' }));
    act(() => FakeEventSource.current().emit({ type: 'session-activity', sessionId: SID }));
    await within(panel).findByText('Neu dazu.');
    const jump = within(panel).getByRole('button', { name: 'Neue Einträge ↓' });
    await userEvent.click(jump);
    expect(within(panel).queryByRole('button', { name: 'Neue Einträge ↓' })).toBeNull();
  });

  it('klappt den Diff einer Datei auf und zeigt Herkunft und Hinweis', async () => {
    const calls = open(`/p/p1/sessions/${SID}`);
    const panel = await dialog();
    await userEvent.click(await within(panel).findByRole('tab', { name: 'Änderungen (2)' }));
    const tab = within(panel).getByRole('tabpanel', { name: 'Änderungen (2)' });
    expect(within(tab).getByText(/Aus Git/).textContent).toContain('main');
    expect(within(tab).getByText('Änderungen per Bash sind nicht enthalten.')).toBeTruthy();
    const file = within(tab).getByRole('button', { name: /login\.ts/ });
    expect(file.getAttribute('aria-expanded')).toBe('false');
    await userEvent.click(file);
    const view = await within(tab).findByLabelText('Diff src/login.ts');
    expect(calls.getSessionFileDiff).toHaveBeenCalledWith('p1', SID, 'src/login.ts');
    expect(within(view).getByText('@@ -1,2 +1,3 @@')).toBeTruthy();
    expect(within(view).getByLabelText('entfernt: alt();').className).toBe('diff-line diff-remove');
    expect(within(view).getByLabelText('hinzugefügt: neu();').className).toBe('diff-line diff-add');
    expect(within(view).getByLabelText('unverändert: import x;').className).toBe('diff-line diff-context');
  });

  it('meldet ein fehlendes Transcript; Lauf funktioniert weiter', async () => {
    open(`/p/p1/sessions/${SID}`, { getSessionTranscript: vi.fn(async () => transcriptPage([], { available: false, cursor: '' })) });
    const panel = await dialog();
    expect(await within(panel).findByText('Kein Transcript gefunden')).toBeTruthy();
    expect(within(panel).getByText(/Gelöscht oder auf einem anderen Rechner/)).toBeTruthy();
    await userEvent.click(within(panel).getByRole('tab', { name: 'Lauf' }));
    expect(within(panel).getByText('Tests schreiben')).toBeTruthy();
  });

  it('lädt lange Verläufe in Seiten nach, bis das Ende erreicht ist', async () => {
    const full = Array.from({ length: TRANSCRIPT_PAGE_SIZE }, (_, i): TranscriptEntry => ({ kind: 'system', id: `s${i}`, at, text: `Zeile ${i}`, level: 'info' }));
    const getSessionTranscript = vi.fn(async (_pid: string, _sid: string, after?: string | null) =>
      after === 'p1' ? transcriptPage([{ kind: 'prompt', id: 'last', at, text: 'Letzter Prompt' }], { cursor: 'p2' }) : transcriptPage(full, { cursor: 'p1' }),
    );
    open(`/p/p1/sessions/${SID}`, { getSessionTranscript });
    const panel = await dialog();
    expect(await within(panel).findByText('Letzter Prompt')).toBeTruthy();
    expect(getSessionTranscript.mock.calls.map((c) => (c as unknown[]).slice(2).map((v) => v ?? null))).toEqual([[null, TRANSCRIPT_PAGE_SIZE], ['p1', TRANSCRIPT_PAGE_SIZE]]);
    expect(within(panel).getByText('Zeile 0')).toBeTruthy();
    expect(within(panel).queryByText('Weitere Einträge werden geladen …')).toBeNull();
  });
});

describe('mergeEntries', () => {
  it('hängt neue Einträge an und ersetzt bekannte an ihrer Stelle', () => {
    const tool: TranscriptEntry = { kind: 'tool', id: 't1', at, name: 'Bash', summary: 'Bash: ls', input: {}, result: null };
    const done: TranscriptEntry = { ...tool, result: { text: 'ok', isError: false, truncated: false } };
    const text: TranscriptEntry = { kind: 'text', id: 'x', at, text: 'Hallo' };
    const merged = mergeEntries([tool, text], [done, { kind: 'text', id: 'y', at, text: 'Neu' }]);
    expect(merged.map((e) => e.id)).toEqual(['t1', 'x', 'y']);
    expect(merged[0]).toBe(done);
    expect(merged[1]).toBe(text);
  });
});
