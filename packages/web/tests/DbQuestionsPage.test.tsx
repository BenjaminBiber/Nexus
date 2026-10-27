// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, cleanup, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter, Route, Routes } from 'react-router-dom';
import { ApiProvider } from '../src/api/ApiContext.js';
import { ApiError } from '../src/api/ApiError.js';
import type { DbNewQuestionInput } from '../src/api/dbTypes.js';
import { DbAskQuestionDialog } from '../src/components/db/DbAskQuestionDialog.js';
import { LiveProvider } from '../src/live/LiveProvider.js';
import { DbQuestionsPage } from '../src/pages/DbQuestionsPage.js';
import { BoardProjectSwitcher } from '../src/components/BoardProjectSwitcher.js';
import { BoardProjectProvider } from '../src/dbboard/boardProject.js';
import { boardPickerTrigger, pickBoard } from './helpers/boardPicker.js';
import { FakeEventSource } from './helpers/FakeEventSource.js';
import { dbQuestion, dbQuestionDetail, dbQuestionList, fakeApi } from './helpers/fakeApi.js';

beforeEach(() => FakeEventSource.reset());
afterEach(cleanup);

const LIST = dbQuestionList([
  dbQuestion('ARC-F0002', 'Welcher Speicher?', {
    tags: ['speicher'], ticketIds: ['ARC-0007'], tickets: [{ id: 'ARC-0007', title: 'Export', status: 'backlog', boardId: 1 }], researched: true, optionCount: 3,
  }),
  dbQuestion('ARC-F0001', 'Wartende Frage', { ready: false, waitingOn: [{ id: 'ARC-F0002', state: 'open' }], originKind: 'human' }),
  dbQuestion('ARC-F0003', 'Alte Entscheidung', { state: 'answered', ready: false }),
  dbQuestion('ARC-F0004', 'Frage eines Kollegen', { mine: false }),
]);

// Rendert die Seite mit Live-Anbieter und der Board-Projekt-Auswahl der Seitenleiste unter /p/p1/questions
function open(overrides: Parameters<typeof fakeApi>[0] = {}, path = '/p/p1/questions') {
  const api = fakeApi({ getDbQuestions: vi.fn(async () => LIST), openEvents: vi.fn(() => new FakeEventSource()), ...overrides });
  render(
    <ApiProvider api={api}>
      <LiveProvider pid="p1">
        <MemoryRouter initialEntries={[path]}>
          <BoardProjectProvider>
            <BoardProjectSwitcher pid="p1" />
            <Routes>
              <Route path="/p/:pid/questions" element={<DbQuestionsPage />} />
              <Route path="*" element={<div>Andere Seite</div>} />
            </Routes>
          </BoardProjectProvider>
        </MemoryRouter>
      </LiveProvider>
    </ApiProvider>,
  );
  return api;
}

describe('DbQuestionsPage', () => {
  it('zeigt die offenen Fragen des Projekts des aktiven Boards mit Kennzeichen und Zählern', async () => {
    const api = open();
    expect(await screen.findByText('Welcher Speicher?')).toBeTruthy();
    expect(api.getDbQuestions).toHaveBeenCalledWith('ARC');
    expect(boardPickerTrigger().getAttribute('aria-label')).toMatch(/^Board wählen: Arches › /);
    const row = screen.getByText('Welcher Speicher?').closest('li')!;
    expect(within(row).getByRole('link', { name: 'Welcher Speicher?' }).getAttribute('href')).toBe('/p/p1/questions/ARC-F0002');
    expect(within(row).getByRole('link', { name: /ARC-0007/ }).getAttribute('href')).toBe('/p/p1/board/t/ARC-0007');
    for (const text of ['bereit', 'recherchiert', '3 Optionen', 'Session', 'speicher']) expect(within(row).getByText(text), text).toBeTruthy();
    expect(within(screen.getByText('Wartende Frage').closest('li')!).getByText('wartet auf ARC-F0002')).toBeTruthy();
    expect(screen.queryByText('Alte Entscheidung')).toBeNull();
    expect(screen.queryByText('Frage eines Kollegen')).toBeNull();
    const tabs = screen.getByRole('tablist', { name: 'Zustand' });
    expect(within(tabs).getAllByRole('tab').map((t) => t.getAttribute('aria-label'))).toEqual(['Eigene (2)', 'Andere offene (1)', 'Beantwortet (1)', 'Verworfen (0)', 'Alle (4)']);
    expect(screen.getByRole('tab', { name: 'Eigene (2)' }).getAttribute('aria-selected')).toBe('true');
  });

  it('zeigt unter „Andere offene“ die offenen Fragen, die den Benutzer nicht betreffen (Kennzeichnung vom Server)', async () => {
    open();
    await screen.findByText('Welcher Speicher?');
    await userEvent.click(screen.getByRole('tab', { name: /Andere offene/ }));
    expect(screen.getByText('Frage eines Kollegen')).toBeTruthy();
    expect(screen.queryByText('Welcher Speicher?')).toBeNull();
    expect(screen.queryByText('Alte Entscheidung')).toBeNull();
  });

  it('wechselt Zustand, Suche und Projekt', async () => {
    const api = open();
    await screen.findByText('Welcher Speicher?');
    await userEvent.click(screen.getByRole('tab', { name: /Beantwortet/ }));
    expect(screen.getByText('Alte Entscheidung')).toBeTruthy();
    expect(screen.queryByText('Welcher Speicher?')).toBeNull();
    await userEvent.click(screen.getByRole('tab', { name: /Alle/ }));
    await userEvent.type(screen.getByLabelText('Suche'), 'speicher');
    expect(screen.getByText('Welcher Speicher?')).toBeTruthy();
    expect(screen.queryByText('Wartende Frage')).toBeNull();
    await pickBoard('Fremd', 'OTH');
    await waitFor(() => expect(api.getDbQuestions).toHaveBeenLastCalledWith('OTH'));
  });

  it('hat zugängliche Zustandsreiter mit Panel und Pfeiltasten', async () => {
    open();
    await screen.findByText('Welcher Speicher?');
    const open_ = screen.getByRole('tab', { name: 'Eigene (2)' });
    const panel = screen.getByRole('tabpanel');
    expect(open_.getAttribute('aria-controls')).toBe(panel.id);
    expect(panel.getAttribute('aria-labelledby')).toBe(open_.id);
    expect(open_.tabIndex).toBe(0);
    expect(screen.getByRole('tab', { name: 'Beantwortet (1)' }).tabIndex).toBe(-1);
    open_.focus();
    await userEvent.keyboard('{ArrowRight}{ArrowRight}');
    const answered = screen.getByRole('tab', { name: 'Beantwortet (1)' });
    expect(document.activeElement).toBe(answered);
    expect(answered.getAttribute('aria-selected')).toBe('true');
    expect(screen.getByText('Alte Entscheidung')).toBeTruthy();
    await userEvent.keyboard('{ArrowLeft}{ArrowLeft}{ArrowLeft}');
    expect(document.activeElement).toBe(screen.getByRole('tab', { name: 'Alle (4)' }));
  });

  it('fällt bei unbekanntem Projekt auf das Projekt des aktiven Boards zurück und sagt es', async () => {
    const api = open({}, '/p/p1/questions?project=XYZ');
    expect(await screen.findByText('Welcher Speicher?')).toBeTruthy();
    expect(api.getDbQuestions).toHaveBeenCalledWith('ARC');
    expect(api.getDbQuestions).not.toHaveBeenCalledWith('XYZ');
    expect(boardPickerTrigger().getAttribute('aria-label')).toMatch(/^Board wählen: Arches › /);
    expect(screen.getByRole('status').textContent).toContain('XYZ');
  });

  it('behält bei 503 während des Nachladens per Live-Meldung die Liste und zeigt einen Hinweis', async () => {
    const getDbQuestions = vi.fn(async () => LIST);
    const api = open({ getDbQuestions });
    await screen.findByText('Welcher Speicher?');
    await waitFor(() => expect(api.openEvents).toHaveBeenLastCalledWith('p1', null, 1));
    getDbQuestions.mockRejectedValue(new ApiError('board-db-unreachable', 'Board-Datenbank ist nicht erreichbar', 503));
    act(() => FakeEventSource.current().emit({ type: 'dbquestions', projectId: 1 }));
    expect((await screen.findByRole('alert')).textContent).toContain('nicht erreichbar');
    expect(screen.getByText('Welcher Speicher?')).toBeTruthy();
    expect(screen.queryByText('Board-Datenbank nicht verfügbar')).toBeNull();
    getDbQuestions.mockResolvedValue(LIST);
    act(() => FakeEventSource.current().emit({ type: 'dbquestions', projectId: 1 }));
    await waitFor(() => expect(screen.queryByRole('alert')).toBeNull());
  });

  it('zeigt einen Leerzustand ohne offene Fragen', async () => {
    open({ getDbQuestions: vi.fn(async () => dbQuestionList([])) });
    expect(await screen.findByText('Keine eigenen offenen Fragen')).toBeTruthy();
  });

  it('lädt bei einer Live-Meldung zum beobachteten Projekt neu, nicht bei anderen Projekten', async () => {
    const api = open();
    await screen.findByText('Welcher Speicher?');
    await waitFor(() => expect(api.openEvents).toHaveBeenLastCalledWith('p1', null, 1));
    act(() => FakeEventSource.current().emit({ type: 'dbquestions', projectId: 1 }));
    await waitFor(() => expect(api.getDbQuestions).toHaveBeenCalledTimes(2));
    act(() => FakeEventSource.current().emit({ type: 'dbquestions', projectId: 2 }));
    expect(api.getDbQuestions).toHaveBeenCalledTimes(2);
  });

  it('zeigt den Hinweis bei nicht verfügbarer Board-Datenbank', async () => {
    open({ getDbProjects: async () => { throw new ApiError('board-db-unreachable', 'Board-Datenbank ist nicht erreichbar', 503); } });
    expect(await screen.findByText('Board-Datenbank nicht verfügbar')).toBeTruthy();
    expect(screen.getByRole('link', { name: /Board-Datenbank/ }).getAttribute('href')).toBe('/p/p1/settings#board-db');
  });

  it('stellt eine neue Frage und hebt sie in der Liste hervor', async () => {
    const createDbQuestion = vi.fn(async (_key: string, input: DbNewQuestionInput) => dbQuestionDetail(dbQuestion('ARC-F0004', input.title)));
    const getDbQuestions = vi.fn(async () => LIST);
    open({ createDbQuestion, getDbQuestions });
    await screen.findByText('Welcher Speicher?');
    getDbQuestions.mockResolvedValue(dbQuestionList([...LIST.questions, dbQuestion('ARC-F0004', 'Neue Frage hier')]));
    await userEvent.click(screen.getByRole('button', { name: 'Neue Frage' }));
    const dialog = screen.getByRole('dialog', { name: 'Neue Frage in ARC' });
    await userEvent.type(within(dialog).getByLabelText('Titel'), 'Neue Frage hier');
    await userEvent.click(within(dialog).getByRole('button', { name: 'Frage stellen' }));
    expect(createDbQuestion).toHaveBeenCalledWith('ARC', { title: 'Neue Frage hier', occasion: null, tags: [], ticketIds: [] });
    const row = (await screen.findByText('Neue Frage hier')).closest('li')!;
    expect(within(row).getByText('neu')).toBeTruthy();
    expect(screen.queryByRole('dialog')).toBeNull();
  });
});

describe('DbQuestionsPage: Filter wie auf der alten Fragenseite', () => {
  const titles = () => screen.queryAllByRole('link', { name: /Speicher|Wartende|Alte/ }).map((l) => l.textContent);

  it('filtert nach Tag', async () => {
    open();
    await screen.findByText('Welcher Speicher?');
    await userEvent.selectOptions(screen.getByLabelText('Tag'), 'speicher');
    expect(titles()).toEqual(['Welcher Speicher?']);
  });

  it('filtert nach betroffenem Ticket', async () => {
    open();
    await screen.findByText('Welcher Speicher?');
    await userEvent.selectOptions(screen.getByLabelText('Bezug'), 'ARC-0007');
    expect(titles()).toEqual(['Welcher Speicher?']);
  });

  it('filtert nach Herkunft', async () => {
    open();
    await screen.findByText('Welcher Speicher?');
    await userEvent.selectOptions(screen.getByLabelText('Herkunft'), 'human');
    expect(titles()).toEqual(['Wartende Frage']);
    await userEvent.selectOptions(screen.getByLabelText('Herkunft'), 'session');
    expect(titles()).toEqual(['Welcher Speicher?']);
  });

  it('filtert nach „wartet auf“ und „recherchiert“', async () => {
    open();
    await screen.findByText('Welcher Speicher?');
    await userEvent.click(screen.getByRole('button', { name: /wartet auf/ }));
    expect(titles()).toEqual(['Wartende Frage']);
    await userEvent.click(screen.getByRole('button', { name: /wartet auf/ }));
    await userEvent.click(screen.getByRole('button', { name: 'recherchiert' }));
    expect(titles()).toEqual(['Welcher Speicher?']);
  });

  it('filtert per Textsuche und setzt die Filter zurück', async () => {
    open();
    await screen.findByText('Welcher Speicher?');
    await userEvent.type(screen.getByLabelText('Suche'), 'wartende');
    expect(titles()).toEqual(['Wartende Frage']);
    await userEvent.type(screen.getByLabelText('Suche'), 'xyz');
    expect(screen.getByText('Keine Fragen mit diesen Filtern')).toBeTruthy();
    await userEvent.click(screen.getByRole('button', { name: 'Filter zurücksetzen' }));
    expect(titles()).toEqual(['Welcher Speicher?', 'Wartende Frage']);
    expect((screen.getByLabelText('Suche') as HTMLInputElement).value).toBe('');
  });
});

describe('DbQuestionsPage: Schnellanlage', () => {
  it('legt eine Frage per Enter als Mensch an, leert das Feld und hebt sie hervor', async () => {
    const createDbQuestion = vi.fn(async (_key: string, input: DbNewQuestionInput) => dbQuestionDetail(dbQuestion('ARC-F0004', input.title, { originKind: 'human' })));
    const getDbQuestions = vi.fn(async () => LIST);
    open({ createDbQuestion, getDbQuestions });
    await screen.findByText('Welcher Speicher?');
    getDbQuestions.mockResolvedValue(dbQuestionList([...LIST.questions, dbQuestion('ARC-F0004', 'Schnelle Frage', { originKind: 'human' })]));
    const field = screen.getByLabelText('Neue Frage') as HTMLInputElement;
    await userEvent.type(field, 'Schnelle Frage{Enter}');
    expect(createDbQuestion).toHaveBeenCalledWith('ARC', { title: 'Schnelle Frage', occasion: 'spontan' });
    const row = (await screen.findByText('Schnelle Frage')).closest('li')!;
    expect(within(row).getByText('neu')).toBeTruthy();
    await waitFor(() => expect(field.value).toBe(''));
  });

  it('legt auch per Knopf „Anlegen“ an und verlangt einen Titel', async () => {
    const createDbQuestion = vi.fn(async (_key: string, input: DbNewQuestionInput) => dbQuestionDetail(dbQuestion('ARC-F0004', input.title)));
    open({ createDbQuestion });
    await screen.findByText('Welcher Speicher?');
    await userEvent.click(screen.getByRole('button', { name: 'Anlegen' }));
    expect((await screen.findByRole('alert')).textContent).toContain('Titel');
    expect(createDbQuestion).not.toHaveBeenCalled();
    await userEvent.type(screen.getByLabelText('Neue Frage'), 'Per Knopf');
    await userEvent.click(screen.getByRole('button', { name: 'Anlegen' }));
    expect(createDbQuestion).toHaveBeenCalledWith('ARC', { title: 'Per Knopf', occasion: 'spontan' });
  });

  it('sendet nur einmal, solange die Anlage läuft', async () => {
    let finish: (value: ReturnType<typeof dbQuestionDetail>) => void = () => {};
    const createDbQuestion = vi.fn(() => new Promise<ReturnType<typeof dbQuestionDetail>>((resolve) => { finish = resolve; }));
    open({ createDbQuestion });
    await screen.findByText('Welcher Speicher?');
    await userEvent.type(screen.getByLabelText('Neue Frage'), 'Doppelt{Enter}');
    await userEvent.click(screen.getByRole('button', { name: 'Anlegen' }));
    expect(createDbQuestion).toHaveBeenCalledTimes(1);
    expect((screen.getByRole('button', { name: 'Anlegen' }) as HTMLButtonElement).disabled).toBe(true);
    await act(async () => finish(dbQuestionDetail(dbQuestion('ARC-F0006', 'Doppelt'))));
    expect(createDbQuestion).toHaveBeenCalledTimes(1);
  });

  it('zeigt einen Fehler des Servers in der Zeile und behält die Eingabe', async () => {
    const createDbQuestion = vi.fn(async () => { throw new ApiError('validation', 'Titel ist zu lang.', 422); });
    open({ createDbQuestion });
    await screen.findByText('Welcher Speicher?');
    await userEvent.type(screen.getByLabelText('Neue Frage'), 'Zu lang{Enter}');
    const alert = await screen.findByRole('alert');
    expect(alert.textContent).toContain('Titel ist zu lang.');
    expect(alert.closest('form')).toBeTruthy();
    expect((screen.getByLabelText('Neue Frage') as HTMLInputElement).value).toBe('Zu lang');
  });
});

describe('DbAskQuestionDialog', () => {
  it('stellt eine Frage zu einem Ticket mit allen Feldern', async () => {
    const createDbQuestion = vi.fn(async () => dbQuestionDetail(dbQuestion('ARC-F0005', 'x')));
    const onCreated = vi.fn();
    render(<ApiProvider api={fakeApi({ createDbQuestion })}><DbAskQuestionDialog projectKey="ARC" ticketId="ARC-0007" onClose={vi.fn()} onCreated={onCreated} /></ApiProvider>);
    expect(screen.getByRole('dialog', { name: 'Frage zu ARC-0007 stellen' })).toBeTruthy();
    expect((screen.getByLabelText('Betroffene Tickets') as HTMLInputElement).value).toBe('ARC-0007');
    await userEvent.type(screen.getByLabelText('Titel'), 'Welcher Speicher?');
    await userEvent.type(screen.getByLabelText('Beschreibung'), 'Kontext');
    await userEvent.type(screen.getByLabelText('Tags'), 'speicher, ui');
    await userEvent.click(screen.getByRole('button', { name: 'Frage stellen' }));
    expect(createDbQuestion).toHaveBeenCalledWith('ARC', {
      title: 'Welcher Speicher?', description: 'Kontext', occasion: 'Ticket ARC-0007', tags: ['speicher', 'ui'], ticketIds: ['ARC-0007'],
    });
    await waitFor(() => expect(onCreated).toHaveBeenCalledWith(expect.objectContaining({ id: 'ARC-F0005' })));
  });

  it('verlangt einen Titel, zeigt Fehler des Servers und schließt mit Esc', async () => {
    const createDbQuestion = vi.fn(async () => { throw new ApiError('question-ticket-unknown', 'Ticket ARC-0099 gibt es nicht.', 422); });
    const onClose = vi.fn();
    render(<ApiProvider api={fakeApi({ createDbQuestion })}><DbAskQuestionDialog projectKey="ARC" onClose={onClose} onCreated={vi.fn()} /></ApiProvider>);
    await userEvent.click(screen.getByRole('button', { name: 'Frage stellen' }));
    expect((await screen.findByRole('alert')).textContent).toContain('Titel');
    expect(createDbQuestion).not.toHaveBeenCalled();
    await userEvent.type(screen.getByLabelText('Titel'), 'A');
    await userEvent.type(screen.getByLabelText('Betroffene Tickets'), 'ARC-0099');
    await userEvent.click(screen.getByRole('button', { name: 'Frage stellen' }));
    expect((await screen.findByRole('alert')).textContent).toContain('ARC-0099');
    // Mit Eingaben fragt Esc erst nach (Konvention aus Teilprojekt 2); „Verwerfen“ schließt
    await userEvent.keyboard('{Escape}');
    expect(onClose).not.toHaveBeenCalled();
    await userEvent.click(within(screen.getByRole('alertdialog', { name: 'Eingaben verwerfen' })).getByRole('button', { name: 'Verwerfen' }));
    expect(onClose).toHaveBeenCalled();
  });

  it('schließt ohne eigene Eingaben sofort, auch mit vorbelegtem Ticket', async () => {
    const onClose = vi.fn();
    render(<ApiProvider api={fakeApi()}><DbAskQuestionDialog projectKey="ARC" ticketId="ARC-0007" onClose={onClose} onCreated={vi.fn()} /></ApiProvider>);
    await userEvent.keyboard('{Escape}');
    expect(onClose).toHaveBeenCalled();
  });

  it('fragt bei Abbrechen mit Eingaben nach und bearbeitet auf Wunsch weiter', async () => {
    const onClose = vi.fn();
    render(<ApiProvider api={fakeApi()}><DbAskQuestionDialog projectKey="ARC" onClose={onClose} onCreated={vi.fn()} /></ApiProvider>);
    await userEvent.type(screen.getByLabelText('Titel'), 'Entwurf');
    await userEvent.click(screen.getByRole('button', { name: 'Abbrechen' }));
    await userEvent.click(screen.getByRole('button', { name: 'Weiter bearbeiten' }));
    expect(onClose).not.toHaveBeenCalled();
    expect((screen.getByLabelText('Titel') as HTMLInputElement).value).toBe('Entwurf');
  });

  it('sperrt die Knöpfe während des Sendens und sendet nur einmal', async () => {
    let finish: (value: ReturnType<typeof dbQuestionDetail>) => void = () => {};
    const createDbQuestion = vi.fn(() => new Promise<ReturnType<typeof dbQuestionDetail>>((resolve) => { finish = resolve; }));
    const onCreated = vi.fn();
    render(<ApiProvider api={fakeApi({ createDbQuestion })}><DbAskQuestionDialog projectKey="ARC" onClose={vi.fn()} onCreated={onCreated} /></ApiProvider>);
    await userEvent.type(screen.getByLabelText('Titel'), 'Doppelt{Enter}{Enter}');
    const send = screen.getByRole('button', { name: 'Frage stellen' }) as HTMLButtonElement;
    expect(send.disabled).toBe(true);
    expect((screen.getByRole('button', { name: 'Abbrechen' }) as HTMLButtonElement).disabled).toBe(true);
    expect(createDbQuestion).toHaveBeenCalledTimes(1);
    await act(async () => finish(dbQuestionDetail(dbQuestion('ARC-F0006', 'Doppelt'))));
    expect(onCreated).toHaveBeenCalledTimes(1);
  });
});
