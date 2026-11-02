// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, cleanup, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter, Route, Routes } from 'react-router-dom';
import { ApiProvider } from '../src/api/ApiContext.js';
import { ApiError } from '../src/api/ApiError.js';
import type { DbDecisionInput, DbQuestionDetailDto } from '../src/api/dbTypes.js';
import { LiveProvider } from '../src/live/LiveProvider.js';
import { DbQuestionDetailPage } from '../src/pages/DbQuestionDetailPage.js';
import { FakeEventSource } from './helpers/FakeEventSource.js';
import { dbQuestion, dbQuestionDetail, fakeApi } from './helpers/fakeApi.js';

vi.mock('mermaid', () => ({ default: { initialize: vi.fn(), render: vi.fn(async () => ({ svg: '<svg></svg>' })) } }));

beforeEach(() => FakeEventSource.reset());
afterEach(cleanup);

const DETAIL: DbQuestionDetailDto = dbQuestionDetail(
  dbQuestion('ARC-F0001', 'Welcher Speicher?', {
    rev: 3, description: 'Wohin mit den Daten?', tags: ['speicher'], originSession: 's-1', occasion: 'Planung ARC-0007',
    ticketIds: ['ARC-0007'], tickets: [{ id: 'ARC-0007', title: 'Export', status: 'backlog', boardId: 1 }],
    dependsOn: ['ARC-F0002'], waitingOn: [{ id: 'ARC-F0002', state: 'open' }], ready: false, researched: true, optionCount: 2,
  }),
  {
    research: [{
      id: 1, questionId: 'ARC-F0001', at: '2026-10-14T10:00:00.000Z', byId: 'u1', byName: 'claude', kind: 'session', context: 'Heute: Datei',
      options: [
        { id: 'O1', title: 'Datei', text: 'JSON-Datei', pros: ['einfach'], cons: [], recommended: false },
        { id: 'O2', title: 'DB', text: 'SQL', pros: ['geteilt'], cons: ['Schema'], recommended: true },
      ],
    }],
    history: [{ questionId: 'ARC-F0001', at: '2026-10-14T09:00:00.000Z', byId: 'u1', byName: 'claude', byKind: 'session', type: 'created', text: 'Frage angelegt' }],
    comments: [{ id: 1, questionId: 'ARC-F0001', authorId: 'u1', authorName: 'Florian', authorKind: 'human', body: 'Bitte bis Freitag', createdAt: '2026-10-14T11:00:00.000Z' }],
  },
);

// Entscheidung zu einer Detailfassung
const decided = (base: DbQuestionDetailDto, rev: number, extra: Partial<DbQuestionDetailDto['decision'] & object> = {}): DbQuestionDetailDto => ({
  ...base, state: 'answered', rev,
  decision: { outcome: 'answered', option: 'O2', text: null, reason: 'geteilt', byId: 'u1', byName: 'Florian', at: '2026-10-14T12:00:00.000Z', ...extra },
});

// Rendert die Detailseite mit Live-Anbieter
function open(overrides: Parameters<typeof fakeApi>[0] = {}) {
  const api = fakeApi({ getDbQuestion: vi.fn(async () => DETAIL), openEvents: vi.fn(() => new FakeEventSource()), ...overrides });
  render(
    <ApiProvider api={api}>
      <LiveProvider pid="p1">
        <MemoryRouter initialEntries={['/p/p1/questions/ARC-F0001']}>
          <Routes>
            <Route path="/p/:pid/questions/:id" element={<DbQuestionDetailPage />} />
          </Routes>
        </MemoryRouter>
      </LiveProvider>
    </ApiProvider>,
  );
  return api;
}

// Wählt Option O2 und gibt eine Begründung ein
async function pickO2(reason = 'geteilt') {
  const research = screen.getByRole('region', { name: 'Aktuelle Recherche' });
  await userEvent.click(within(research).getAllByRole('button', { name: 'Diese Option wählen' })[1]);
  await userEvent.type(screen.getByLabelText('Begründung'), reason);
}

describe('DbQuestionDetailPage – Recherche starten', () => {
  it('startet die Recherche, zeigt den Hinweis und lädt die Frage nach, wenn sie endet', async () => {
    const run = { id: 'r1', kind: 'research' as const, ticketId: 'ARC-F0001', questionId: 'ARC-F0001', title: 'Welcher Speicher?', profile: 'standard', model: null, effort: null, startedAt: '2026-10-15T07:00:00.000Z', log: 'ARC-F0001-20261015-090000-research.log' };
    let running: typeof run | null = null;
    const getQuestionResearch = vi.fn(async () => ({ questionId: 'ARC-F0001', run: running, pausedUntil: null, lastStartedAt: null, count: 0 }));
    const startQuestionResearch = vi.fn(async () => {
      running = run;
      return { started: run };
    });
    const getDbQuestion = vi.fn(async () => DETAIL);
    vi.useFakeTimers({ shouldAdvanceTime: true });
    try {
      open({ getQuestionResearch, startQuestionResearch, getDbQuestion });
      const button = await screen.findByRole('button', { name: 'Recherche starten' });
      await userEvent.click(button);
      expect(startQuestionResearch).toHaveBeenCalledWith('ARC-F0001');
      const card = screen.getByRole('region', { name: 'Recherche anfordern' });
      expect((await within(card).findByRole('status')).textContent).toContain('Recherche läuft – Ergebnis erscheint hier automatisch');
      expect((screen.getByRole('button', { name: 'Recherche starten' }) as HTMLButtonElement).disabled).toBe(true);
      const loads = getDbQuestion.mock.calls.length;
      running = null;
      await act(async () => { await vi.advanceTimersByTimeAsync(10_000); });
      await waitFor(() => expect(getDbQuestion.mock.calls.length).toBeGreaterThan(loads));
      await waitFor(() => expect((screen.getByRole('button', { name: 'Recherche starten' }) as HTMLButtonElement).disabled).toBe(false));
    } finally {
      vi.useRealTimers();
    }
  });

  it('zeigt Fehler beim Start sichtbar an', async () => {
    open({
      getQuestionResearch: async () => ({ questionId: 'ARC-F0001', run: null, pausedUntil: null, lastStartedAt: null, count: 0 }),
      startQuestionResearch: async () => { throw new ApiError('research-no-repo', 'Für Frage ARC-F0001 ist kein eigenes Board mit Repo-Pfad bekannt', 422); },
    });
    await userEvent.click(await screen.findByRole('button', { name: 'Recherche starten' }));
    expect((await screen.findByRole('alert')).textContent).toContain('kein eigenes Board mit Repo-Pfad');
  });

  it('zeigt die Recherche-Läufe mit Protokoll und Arbeitsanweisung als eigenen Abschnitt', async () => {
    const getDispatchRuns = vi.fn(async () => [{ name: 'ARC-F0001-20261016-120000-research.log', kind: 'research' as const, started: '2026-10-16T10:00:00.000Z', running: false }]);
    const getDispatchRunLog = vi.fn(async (name: string) => ({ name, header: '[dispatch] Start', prompt: 'Recherchiere ARC-F0001.', body: '12:00:01  Claude startet (opus)\n' }));
    open({ getDispatchRuns, getDispatchRunLog });
    const section = await screen.findByRole('region', { name: 'Recherche-Läufe' });
    expect(await within(section).findByText(/Claude startet \(opus\)/)).toBeTruthy();
    expect(within(section).getByText(/Recherche vom/)).toBeTruthy();
    expect(within(section).getByText('Arbeitsanweisung an den Agent')).toBeTruthy();
    expect(getDispatchRuns).toHaveBeenCalledWith('ARC-F0001');
  });

  it('meldet im Abschnitt Recherche-Läufe, wenn es noch keinen gab', async () => {
    open();
    const section = await screen.findByRole('region', { name: 'Recherche-Läufe' });
    expect(await within(section).findByText('Zu dieser Frage gab es noch keinen Recherche-Lauf.')).toBeTruthy();
  });

  it('zeigt am Knopf, dass der Dispatcher pausiert', async () => {
    const until = new Date(2026, 9, 15, 11, 30).toISOString();
    open({ getQuestionResearch: async () => ({ questionId: 'ARC-F0001', run: null, pausedUntil: until, lastStartedAt: null, count: 0 }) });
    const card = await screen.findByRole('region', { name: 'Recherche anfordern' });
    expect(await within(card).findByText('Der Dispatcher pausiert bis 11:30 (z. B. Nutzungslimit) – eine Recherche startet erst danach.')).toBeTruthy();
  });
});

describe('DbQuestionDetailPage', () => {
  it('zeigt Kopf, Beschreibung, Recherche, Verlauf mit Kommentaren und Bezüge', async () => {
    open();
    expect(await screen.findByRole('heading', { name: 'Welcher Speicher?', level: 1 })).toBeTruthy();
    expect(screen.getByText('Wohin mit den Daten?')).toBeTruthy();
    expect(screen.getByText(/Herkunft: claude \(Session s-1\)/)).toBeTruthy();
    expect(screen.getByRole('link', { name: 'Zurück zu den Fragen' }).getAttribute('href')).toBe('/p/p1/questions?project=ARC');
    const research = screen.getByRole('region', { name: 'Aktuelle Recherche' });
    expect(within(research).getByText('Datei')).toBeTruthy();
    expect(within(research).getByText('empfohlen')).toBeTruthy();
    expect(screen.getByText('/nexus:board-question ARC-F0001')).toBeTruthy();
    const history = screen.getByRole('region', { name: 'Verlauf' });
    expect(within(history).getByText('Frage angelegt')).toBeTruthy();
    expect(within(history).getByText('Bitte bis Freitag')).toBeTruthy();
    const tickets = screen.getByRole('region', { name: 'Betroffene Tickets' });
    expect(within(tickets).getByRole('link', { name: /ARC-0007/ }).getAttribute('href')).toBe('/p/p1/board/t/ARC-0007');
    const deps = screen.getByRole('region', { name: 'Abhängig von' });
    expect(within(deps).getByRole('link', { name: 'ARC-F0002' }).getAttribute('href')).toBe('/p/p1/questions/ARC-F0002');
    expect(within(deps).getByText('noch offen')).toBeTruthy();
  });

  it('entscheidet mit einer Option der neuesten Recherche und Begründung', async () => {
    const decideDbQuestion = vi.fn(async (_id: string, input: DbDecisionInput) => decided(DETAIL, 4, { option: input.option ?? null, reason: input.reason }));
    open({ decideDbQuestion });
    await screen.findByRole('heading', { name: 'Welcher Speicher?', level: 1 });
    await pickO2();
    await userEvent.click(screen.getByRole('button', { name: 'Entscheiden' }));
    expect(decideDbQuestion).toHaveBeenCalledWith('ARC-F0001', { rev: 3, option: 'O2', reason: 'geteilt' });
    const box = await screen.findByRole('region', { name: 'Entscheidung' });
    expect(within(box).getByText('Option O2 · DB')).toBeTruthy();
  });

  it('entscheidet auch ohne Begründung', async () => {
    const decideDbQuestion = vi.fn(async (_id: string, input: DbDecisionInput) => decided(DETAIL, 4, { option: input.option ?? null, reason: '' }));
    open({ decideDbQuestion });
    await screen.findByRole('heading', { name: 'Welcher Speicher?', level: 1 });
    const research = screen.getByRole('region', { name: 'Aktuelle Recherche' });
    await userEvent.click(within(research).getAllByRole('button', { name: 'Diese Option wählen' })[1]);
    const button = screen.getByRole('button', { name: 'Entscheiden' }) as HTMLButtonElement;
    expect(button.disabled).toBe(false);
    await userEvent.click(button);
    expect(decideDbQuestion).toHaveBeenCalledWith('ARC-F0001', { rev: 3, option: 'O2', reason: '' });
  });

  it('verlangt beim Verwerfen eine Begründung und öffnet wieder', async () => {
    const discardDbQuestion = vi.fn(async () => ({ ...decided(DETAIL, 4, { outcome: 'discarded', option: null, reason: 'doppelt' }), state: 'discarded' as const }));
    const reopenDbQuestion = vi.fn(async () => ({ ...DETAIL, rev: 5 }));
    open({ discardDbQuestion, reopenDbQuestion });
    await screen.findByRole('heading', { name: 'Welcher Speicher?', level: 1 });
    await userEvent.click(screen.getByRole('button', { name: 'Verwerfen' }));
    expect(await screen.findByText('Bitte eine Begründung angeben.')).toBeTruthy();
    expect(discardDbQuestion).not.toHaveBeenCalled();
    await userEvent.type(screen.getByLabelText('Begründung'), 'doppelt');
    await userEvent.click(screen.getByRole('button', { name: 'Verwerfen' }));
    expect(discardDbQuestion).toHaveBeenCalledWith('ARC-F0001', 3, 'doppelt');
    await userEvent.click(await screen.findByRole('button', { name: 'Wieder öffnen' }));
    expect(reopenDbQuestion).toHaveBeenCalledWith('ARC-F0001', 4, undefined);
    expect(await screen.findByRole('region', { name: 'Entscheiden' })).toBeTruthy();
  });

  it('bietet bei einem Konflikt „Meine Fassung behalten“ und entscheidet dann mit der neuen Version', async () => {
    const fresh: DbQuestionDetailDto = { ...DETAIL, rev: 4, title: 'Neuer Titel' };
    const decideDbQuestion = vi.fn()
      .mockRejectedValueOnce(new ApiError('conflict', 'Frage ARC-F0001 wurde inzwischen geändert.', 409, fresh))
      .mockResolvedValueOnce(decided(fresh, 5));
    open({ decideDbQuestion });
    await screen.findByRole('heading', { name: 'Welcher Speicher?', level: 1 });
    await pickO2();
    await userEvent.click(screen.getByRole('button', { name: 'Entscheiden' }));
    const dialog = await screen.findByRole('alertdialog', { name: 'Speicherkonflikt' });
    await userEvent.click(within(dialog).getByRole('button', { name: 'Meine Fassung behalten' }));
    expect(decideDbQuestion).toHaveBeenLastCalledWith('ARC-F0001', { rev: 4, option: 'O2', reason: 'geteilt' });
    expect(await screen.findByRole('region', { name: 'Entscheidung' })).toBeTruthy();
    expect(screen.queryByRole('alertdialog')).toBeNull();
  });

  it('ändert betroffene Tickets und Abhängigkeiten und zeigt Kreise als Fehler', async () => {
    const putDbQuestionTickets = vi.fn(async (_id: string, rev: number, ticketIds: string[]) => ({
      ...DETAIL, rev: rev + 1, ticketIds, tickets: ticketIds.map((id) => ({ id, title: id, status: 'backlog' as const, boardId: 1 })),
    }));
    const putDbQuestionDepends = vi.fn(async () => { throw new ApiError('dependency-cycle', 'Abhängigkeiten liefen im Kreis: ARC-F0001 -> ARC-F0003 -> ARC-F0001.', 422); });
    open({ putDbQuestionTickets, putDbQuestionDepends });
    await screen.findByRole('heading', { name: 'Welcher Speicher?', level: 1 });
    const tickets = () => screen.getByRole('region', { name: 'Betroffene Tickets' });
    await userEvent.type(within(tickets()).getByLabelText('Ticket hinzufügen'), 'arc-0008');
    await userEvent.click(within(tickets()).getByRole('button', { name: 'Hinzufügen' }));
    expect(putDbQuestionTickets).toHaveBeenCalledWith('ARC-F0001', 3, ['ARC-0007', 'ARC-0008']);
    await userEvent.click(await within(tickets()).findByRole('button', { name: 'ARC-0007 entfernen' }));
    expect(putDbQuestionTickets).toHaveBeenLastCalledWith('ARC-F0001', 4, ['ARC-0008']);
    const deps = screen.getByRole('region', { name: 'Abhängig von' });
    await userEvent.type(within(deps).getByLabelText('Abhängigkeit hinzufügen'), 'ARC-F0003');
    await userEvent.click(within(deps).getByRole('button', { name: 'Hinzufügen' }));
    expect(putDbQuestionDepends).toHaveBeenCalledWith('ARC-F0001', 5, ['ARC-F0002', 'ARC-F0003']);
    expect((await screen.findByRole('alert')).textContent).toContain('Kreis');
  });

  it('schreibt Kommentare und lädt bei einer Live-Meldung des Projekts nach', async () => {
    const addDbQuestionComment = vi.fn(async (_id: string, body: string) => ({
      ...DETAIL, rev: 4,
      comments: [...DETAIL.comments, { id: 2, questionId: 'ARC-F0001', authorId: 'u1', authorName: 'Florian', authorKind: 'human' as const, body, createdAt: '2026-10-14T12:00:00.000Z' }],
    }));
    const api = open({ addDbQuestionComment });
    await screen.findByRole('heading', { name: 'Welcher Speicher?', level: 1 });
    expect(screen.getByText('Sessions sehen Kommentare mit „harness board question show“.')).toBeTruthy();
    await userEvent.type(screen.getByLabelText('Neuer Kommentar'), 'Noch eine Idee');
    await userEvent.click(screen.getByRole('button', { name: 'Kommentar senden' }));
    expect(addDbQuestionComment).toHaveBeenCalledWith('ARC-F0001', 'Noch eine Idee');
    expect(await screen.findByText('Noch eine Idee')).toBeTruthy();
    await waitFor(() => expect(api.openEvents).toHaveBeenLastCalledWith('p1', null, 1));
    act(() => FakeEventSource.current().emit({ type: 'dbquestions', projectId: 1 }));
    await waitFor(() => expect(api.getDbQuestion).toHaveBeenCalledTimes(2));
  });

  it('setzt beim Wechsel zu einer anderen Frage Auswahl, Begründung und Eingaben zurück', async () => {
    const second: DbQuestionDetailDto = { ...DETAIL, id: 'ARC-F0002', number: 2, title: 'Zweite Frage', dependsOn: [], waitingOn: [] };
    open({ getDbQuestion: vi.fn(async (id: string) => (id === 'ARC-F0002' ? second : DETAIL)) });
    await screen.findByRole('heading', { name: 'Welcher Speicher?', level: 1 });
    await pickO2('mein Grund');
    await userEvent.type(within(screen.getByRole('region', { name: 'Betroffene Tickets' })).getByLabelText('Ticket hinzufügen'), 'ARC-0009');
    await userEvent.click(within(screen.getByRole('region', { name: 'Abhängig von' })).getByRole('link', { name: 'ARC-F0002' }));
    expect(await screen.findByRole('heading', { name: 'Zweite Frage', level: 1 })).toBeTruthy();
    expect(screen.queryByRole('button', { name: 'Gewählt' })).toBeNull();
    expect(screen.getByText('Noch keine Option gewählt.')).toBeTruthy();
    expect((screen.getByLabelText('Begründung') as HTMLTextAreaElement).value).toBe('');
    expect((within(screen.getByRole('region', { name: 'Betroffene Tickets' })).getByLabelText('Ticket hinzufügen') as HTMLInputElement).value).toBe('');
  });

  it('hebt die Auswahl auf, wenn eine neue Recherche die gewählte Option nicht mehr enthält', async () => {
    const getDbQuestion = vi.fn(async () => DETAIL);
    open({ getDbQuestion });
    await screen.findByRole('heading', { name: 'Welcher Speicher?', level: 1 });
    await pickO2();
    expect(screen.getByText('Gewählt: Option O2 · DB')).toBeTruthy();
    const newer: DbQuestionDetailDto = {
      ...DETAIL, rev: 4,
      research: [...DETAIL.research, {
        id: 2, questionId: 'ARC-F0001', at: '2026-10-15T10:00:00.000Z', byId: 'u1', byName: 'claude', kind: 'session', context: 'Neu',
        options: [{ id: 'O1', title: 'Cache', text: 'Redis', pros: [], cons: [], recommended: true }],
      }],
    };
    getDbQuestion.mockResolvedValue(newer);
    await waitFor(() => expect(FakeEventSource.current()).toBeTruthy());
    act(() => FakeEventSource.current().emit({ type: 'dbquestions', projectId: 1 }));
    expect(await screen.findByText('Noch keine Option gewählt.')).toBeTruthy();
  });

  it('zeigt den Hinweis bei nicht verfügbarer Board-Datenbank', async () => {
    open({ getDbQuestion: async () => { throw new ApiError('board-schema-outdated', 'Schema-Version 1, erwartet 2', 503); } });
    expect(await screen.findByText('Board-Datenbank nicht verfügbar')).toBeTruthy();
  });
});
