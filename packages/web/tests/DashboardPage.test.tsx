// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter, Route, Routes } from 'react-router-dom';
import { ApiProvider } from '../src/api/ApiContext.js';
import type { ApiClient } from '../src/api/ApiClient.js';
import { ApiError } from '../src/api/ApiError.js';
import type { DbNoteDto } from '../src/api/dbTypes.js';
import type { DispatchStatusDto } from '../src/api/dispatchTypes.js';
import type { InboxListDto } from '../src/api/inboxTypes.js';
import type { DispatchStatsDto, DispatchStatsFilter } from '../src/api/statsTypes.js';
import { DashboardPage } from '../src/pages/DashboardPage.js';
import { BoardProjectProvider } from '../src/dbboard/boardProject.js';
import { dispatchStatus, fakeApi, inboxItem, inboxList } from './helpers/fakeApi.js';

afterEach(cleanup);

const TODAY = new Date(2026, 9, 19, 12);

function stats(over: Partial<DispatchStatsDto> = {}): DispatchStatsDto {
  return {
    filter: { project: 'ARC', boardId: null, from: '2026-09-22', to: '2026-10-19', kind: 'work' },
    kind: 'work',
    runsSince: '2026-10-01T08:00:00.000Z',
    backfilledRuns: 0,
    configChangedAt: '2026-10-12T08:00:00.000Z',
    kpis: { runs: 10, subjects: 6, done: 6, aborted: 3, solved: 3, successRate: 3 / 4, firstTryRate: 0.5, escalationRate: 0.4, finalFailures: 1 },
    series: { unit: 'day', points: [{ start: '2026-10-18', done: 2, aborted: 1, other: 1 }, { start: '2026-10-19', done: 4, aborted: 2, other: 0 }] },
    byProfile: [
      { profile: 'extra', runs: 1, done: 1, escalated: 0, final: 0, other: 0 },
      { profile: 'klein', runs: 6, done: 3, escalated: 2, final: 0, other: 1 },
      { profile: 'standard', runs: 3, done: 2, escalated: 0, final: 1, other: 0 },
    ],
    ladder: { finished: 4, firstTry: 0.5, afterEscalation: 0.25, final: 0.25 },
    abortReasons: [{ category: 'max-turns', label: 'Schrittlimit', count: 2 }, { category: 'vanished', label: 'Worker verschwunden', count: 1 }],
    configurableAbortShare: 2 / 3,
    usagePauses: 2,
    staleRuns: 1,
    ...over,
  };
}

interface Fakes {
  getDispatchStats?: (f: DispatchStatsFilter) => Promise<DispatchStatsDto>;
  getDispatchStatus?: () => Promise<DispatchStatusDto>;
  getInbox?: () => Promise<InboxListDto>;
  getDbNotes?: ApiClient['getDbNotes'];
  takeDbNote?: ApiClient['takeDbNote'];
}

function open(path: string, fakes: Fakes) {
  return render(
    <ApiProvider api={fakeApi({
      getDispatchConfig: async () => ({ file: 'd.yaml', exists: true, raw: {}, config: { enabled: true, staleMinutes: 30, ladder: ['klein', 'standard', 'schwer'] }, error: null }),
      ...fakes,
    })}>
      <MemoryRouter initialEntries={[path]}>
        <BoardProjectProvider>
          <Routes>
            <Route path="/p/:pid/dashboard" element={<DashboardPage today={TODAY} />} />
          </Routes>
        </BoardProjectProvider>
      </MemoryRouter>
    </ApiProvider>,
  );
}

describe('DashboardPage', () => {
  it('lädt Arbeitsläufe der letzten 4 Wochen und zeigt Kennzahlen, Leiter, Abbruchgründe und Hindernisse', async () => {
    const load = vi.fn(async () => stats());
    open('/p/p1/dashboard', {
      getDispatchStats: load,
      getDispatchStatus: async () => dispatchStatus({
        enabled: true, state: 'running', wip: 3, active: 2,
        tickets: [
          { id: 'ARC-0005', title: 'A', reason: 'Wartet auf ARC-0002' },
          { id: 'ARC-0006', title: 'B', reason: 'wartet auf Merge von ARC-0003' },
          { id: 'ARC-0007', title: 'C', reason: 'Label „manuell“' },
          { id: 'ARC-0008', title: 'D', reason: 'bereit – wartet auf freien Platz' },
        ],
      }),
    });
    expect(await screen.findByRole('region', { name: 'Läufe' })).toBeTruthy();
    expect(load).toHaveBeenCalledWith({ project: 'ARC', board: null, from: '2026-09-22', to: '2026-10-19', kind: 'work' });
    // Erläuterung nur als Tooltip
    expect(screen.getByRole('region', { name: 'Läufe' }).title).toBe('6 Tickets im Zeitraum');
    expect(screen.queryByText('6 Tickets im Zeitraum')).toBeNull();
    expect(within(screen.getByRole('region', { name: 'Erfolgsquote' })).getByText('75 %')).toBeTruthy();
    expect(screen.getByRole('region', { name: 'Erfolgsquote' }).title).toBe('3 gelöst · 1 endgültig gescheitert');
    expect(within(screen.getByRole('region', { name: 'Erstversuch-Quote' })).getByText('50 %')).toBeTruthy();
    // Über 30 % Eskalation: orange
    expect(within(screen.getByRole('region', { name: 'Eskalationsquote' })).getByText('40 %').className).toContain('tone-warn');
    expect(within(screen.getByRole('region', { name: 'Endgültig gescheitert' })).getByText('1').className).toContain('tone-bad');
    // Kein Hinweis auf den Beginn der Erfassung
    expect(screen.queryByRole('note')).toBeNull();
    expect(screen.getByRole('figure', { name: /Läufe je Tag: 10 in 2 Tagen/ })).toBeTruthy();

    // Leiter in der Reihenfolge der Einstellung, Profile außerhalb danach
    const ladder = screen.getByRole('region', { name: 'Eskalation entlang der Leiter' });
    expect(within(ladder).getAllByText(/^(klein|standard|schwer|extra)$/).map((e) => e.textContent)).toEqual(['klein', 'standard', 'schwer', 'extra']);
    // Je Profil Anteil gelöst; Details im Tooltip
    const klein = within(ladder).getByText('klein').closest('li')!;
    expect(within(klein).getByText('50 %')).toBeTruthy();
    expect(klein.title).toBe('50 % gelöst (3 Läufe) · 33 % eskaliert weiter (2 Läufe) · 17 % Frage / blockiert (1 Lauf)');
    expect(within(ladder).getByText('standard').closest('li')!.title).toBe('67 % gelöst (2 Läufe) · 33 % endgültig gescheitert (1 Lauf)');
    // Ausgang der abgeschlossenen Tickets
    expect(within(ladder).getByText('im 1. Versuch gelöst').previousSibling?.textContent).toBe('50 %');
    expect(within(ladder).getByText('endgültig gescheitert').previousSibling?.textContent).toBe('25 %');

    const reasons = screen.getByRole('region', { name: 'Häufigste Abbruchgründe' });
    expect(within(reasons).getByText('Schrittlimit')).toBeTruthy();
    // Anteil an allen Abbrüchen; Zahlen und beeinflussbarer Anteil im Tooltip
    const turns = within(reasons).getByText('Schrittlimit').closest('li')!;
    expect(within(turns).getByText('67 %')).toBeTruthy();
    expect(within(turns).getByText('2 Läufe')).toBeTruthy();
    expect(turns.title).toBe('Schrittlimit: 2 Abbrüche von 3 (67 %)');
    expect(within(reasons).getByRole('heading').title).toMatch(/67 % der Abbrüche/);

    const obstacles = screen.getByRole('region', { name: 'Hindernisse' });
    expect(within(obstacles).getByText('bereit, warten auf ARC-0002, ARC-0003')).toBeTruthy();
    expect(within(obstacles).getByText('Label „manuell“')).toBeTruthy();
    // Dispatcher-Knopf wie auf dem Board; öffnet die Übersicht
    expect(screen.queryByText(/Projekt ARC/)).toBeNull();
    await userEvent.click(await screen.findByRole('button', { name: /^Dispatcher:/ }));
    expect(await screen.findByRole('dialog')).toBeTruthy();
  });

  it('wechselt Lauf-Art und Zeitraum und übernimmt sie aus der Adresse', async () => {
    const load = vi.fn(async () => stats());
    open('/p/p1/dashboard?project=ARC&board=2&kind=merge&period=7', { getDispatchStats: load });
    await screen.findByRole('region', { name: 'Läufe' });
    expect(load).toHaveBeenLastCalledWith({ project: 'ARC', board: 2, from: '2026-10-13', to: '2026-10-19', kind: 'merge' });
    await userEvent.click(screen.getByRole('button', { name: 'Recherche' }));
    await waitFor(() => expect(load).toHaveBeenLastCalledWith(expect.objectContaining({ kind: 'research' })));
    await userEvent.selectOptions(screen.getByLabelText('Zeitraum'), 'Letzte 8 Wochen');
    await waitFor(() => expect(load).toHaveBeenLastCalledWith(expect.objectContaining({ from: '2026-08-25' })));
    await userEvent.selectOptions(screen.getByLabelText('Zeitraum'), 'cfg');
    await waitFor(() => expect(load).toHaveBeenLastCalledWith(expect.objectContaining({ from: 'config' })));
    // Board: nur aktuelles oder alle
    const board = screen.getByRole('group', { name: 'Board' });
    expect(within(board).getAllByRole('button').map((b) => b.textContent)).toEqual(['Aktuelles Board', 'Alle Boards']);
    expect(within(board).getByRole('button', { name: 'Aktuelles Board' }).title).toBe('Nächster (Florian)');
    await userEvent.click(within(board).getByRole('button', { name: 'Alle Boards' }));
    await waitFor(() => expect(load).toHaveBeenLastCalledWith(expect.objectContaining({ board: null })));
    // Ohne Board-Filter gilt das aktive Board als aktuelles
    await userEvent.click(within(board).getByRole('button', { name: 'Aktuelles Board' }));
    await waitFor(() => expect(load).toHaveBeenLastCalledWith(expect.objectContaining({ board: 1 })));
    // Planung gibt es als Lauf-Art nicht
    expect(screen.queryByRole('button', { name: 'Planung' })).toBeNull();
  });

  it('übernimmt einen benutzerdefinierten Zeitraum aus der Adresse und ändert ihn über die Datumsfelder', async () => {
    const load = vi.fn(async () => stats());
    open('/p/p1/dashboard?project=ARC&period=custom&from=2026-10-10&to=2026-09-01', { getDispatchStats: load });
    await screen.findByRole('region', { name: 'Läufe' });
    // Vertauschte Grenzen werden getauscht
    expect(load).toHaveBeenLastCalledWith(expect.objectContaining({ from: '2026-09-01', to: '2026-10-10' }));
    expect((screen.getByLabelText('Zeitraum') as HTMLSelectElement).value).toBe('custom');
    fireEvent.change(screen.getByLabelText('Bis'), { target: { value: '2026-10-15' } });
    await waitFor(() => expect(load).toHaveBeenLastCalledWith(expect.objectContaining({ from: '2026-09-01', to: '2026-10-15' })));
    // Beginn nach dem Ende: das Ende zieht mit
    fireEvent.change(screen.getByLabelText('Von'), { target: { value: '2026-10-17' } });
    await waitFor(() => expect(load).toHaveBeenLastCalledWith(expect.objectContaining({ from: '2026-10-17', to: '2026-10-17' })));
  });

  it('startet „Benutzerdefiniert“ mit dem zuletzt geladenen Zeitraum', async () => {
    const load = vi.fn(async () => stats());
    open('/p/p1/dashboard', { getDispatchStats: load });
    await screen.findByRole('region', { name: 'Läufe' });
    expect(screen.queryByLabelText('Von')).toBeNull();
    await userEvent.selectOptions(screen.getByLabelText('Zeitraum'), 'custom');
    expect((await screen.findByLabelText('Von') as HTMLInputElement).value).toBe('2026-09-22');
    expect((screen.getByLabelText('Bis') as HTMLInputElement).value).toBe('2026-10-19');
  });

  it('sperrt „Seit Konfig-Änderung“, solange keine Fassung der Einstellung erfasst ist', async () => {
    open('/p/p1/dashboard', { getDispatchStats: async () => stats({ configChangedAt: null }) });
    await screen.findByRole('region', { name: 'Läufe' });
    expect((screen.getByRole('option', { name: 'Seit Konfig-Änderung (noch keine)' }) as HTMLOptionElement).disabled).toBe(true);
  });

  it('gruppiert die Aufgaben, markiert alte Blockierte und klappt Gruppen ein', async () => {
    open('/p/p1/dashboard', {
      getDispatchStats: async () => stats(),
      getInbox: async () => inboxList([
        inboxItem('ticket:ARC-0001', 'blocked', 'Alt blockiert', { since: '2026-10-17T10:00:00.000Z', detail: 'Zugang fehlt' }),
        inboxItem('ticket:ARC-0002', 'merge', 'Merge hakt'),
        inboxItem('question:Q-0001', 'question', 'Welche Farbe?', { waitingTickets: 2 }),
        inboxItem('ticket:ARC-0003', 'accept', 'Bitte abnehmen'),
      ]),
    });
    const blocked = (await screen.findByRole('button', { name: /Blockierte/ })).closest('section')!;
    expect(within(blocked).getAllByRole('listitem').map((li) => li.className)).toEqual(['todo-row flagged urgent', 'todo-row flagged']);
    expect(within(blocked).getByText(/seit 2 Tage · Zugang fehlt/)).toBeTruthy();
    expect(within(blocked).getByRole('button', { name: 'Erneut mergen' })).toBeTruthy();
    const questions = screen.getByRole('button', { name: /Fragen/ }).closest('section')!;
    expect(within(questions).getByRole('link', { name: 'Antworten' }).getAttribute('href')).toBe('/p/p1/questions/Q-0001');
    expect(within(questions).getByText(/2 Tickets warten/)).toBeTruthy();
    expect(within(screen.getByRole('button', { name: /Freigaben/ }).closest('section')!).getByRole('button', { name: 'Abnehmen' })).toBeTruthy();
    // Leere Gruppe ist eingeklappt und lässt sich aufklappen
    const notes = screen.getByRole('button', { name: /Notizen/ });
    expect(notes.getAttribute('aria-expanded')).toBe('false');
    fireEvent.click(notes);
    expect(within(notes.closest('section')!).getByText('Nichts offen.')).toBeTruthy();
    // Eingeklappt: Zeilen weg, „!“ im Kopf
    fireEvent.click(screen.getByRole('button', { name: /Blockierte/ }));
    expect(within(blocked).queryAllByRole('listitem')).toHaveLength(0);
    expect(within(blocked).getByTitle('1 seit über 24 Std.')).toBeTruthy();
  });

  it('öffnet eine Notiz im Slide-In statt auf dem Board und macht daraus ein Ticket', async () => {
    const note: DbNoteDto = {
      id: 7, boardId: 1, authorId: 'u1', authorName: 'Florian', title: 'Tooltip fehlt', body: 'Im **Kopf** fehlt der Tooltip.', source: 'ARC-0004',
      labels: ['ui'], seenCount: 2, seenIn: ['ARC-0005'], state: 'open', ticketId: null, reason: null, createdAt: '2026-10-18T10:00:00.000Z',
    };
    const getDbNotes = vi.fn(async () => [note]);
    const takeDbNote = vi.fn(async () => ({ note: { ...note, state: 'taken' as const, ticketId: 'ARC-0009' }, ticket: {} as never }));
    open('/p/p1/dashboard', {
      getDispatchStats: async () => stats(),
      getInbox: async () => inboxList([inboxItem('note:7', 'note', 'Tooltip fehlt')]),
      getDbNotes, takeDbNote,
    });
    await userEvent.click(await screen.findByRole('button', { name: 'Tooltip fehlt' }));
    const slide = await screen.findByRole('dialog', { name: 'Notiz N-7' });
    expect(getDbNotes).toHaveBeenCalledWith(1, 'all');
    expect(await within(slide).findByText('Kopf')).toBeTruthy();
    expect(within(slide).getByText('2× aufgefallen')).toBeTruthy();
    expect(within(slide).getByRole('button', { name: 'aus ARC-0004' })).toBeTruthy();
    await userEvent.click(within(slide).getByRole('button', { name: 'Ticket daraus machen' }));
    expect((within(slide).getByLabelText('Titel des Tickets') as HTMLInputElement).value).toBe('Tooltip fehlt');
    await userEvent.click(within(slide).getByRole('button', { name: 'Ticket anlegen' }));
    expect(takeDbNote).toHaveBeenCalledWith(7, { title: 'Tooltip fehlt' });
    fireEvent.keyDown(slide, { key: 'Escape' });
    expect(screen.queryByRole('dialog', { name: 'Notiz N-7' })).toBeNull();
  });

  it('meldet, wenn nichts zu tun ist, und zeigt den Hinweis ohne Datenbank', async () => {
    open('/p/p1/dashboard', { getDispatchStats: async () => stats() });
    expect(await screen.findByText('Nichts zu tun – der Dispatcher kommt ohne dich weiter.')).toBeTruthy();
    cleanup();
    open('/p/p1/dashboard', { getDispatchStats: async () => { throw new ApiError('board-db-unreachable', 'Datenbank nicht erreichbar', 503); } });
    expect(await screen.findByRole('heading', { name: 'Board-Datenbank nicht verfügbar' })).toBeTruthy();
  });
});
