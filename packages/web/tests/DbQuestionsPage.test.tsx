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

vi.mock('mermaid', () => ({ default: { initialize: vi.fn(), render: vi.fn(async () => ({ svg: '<svg></svg>' })) } }));

beforeEach(() => {
  FakeEventSource.reset();
  window.localStorage.clear();
});
afterEach(cleanup);

const LIST = dbQuestionList([
  dbQuestion('ARC-F0002', 'Welcher Speicher?', {
    tags: ['speicher'], ticketIds: ['ARC-0007'], tickets: [{ id: 'ARC-0007', title: 'Export', status: 'backlog', boardId: 1 }], researched: true, optionCount: 3,
  }),
  dbQuestion('ARC-F0001', 'Wartende Frage', { ready: false, waitingOn: [{ id: 'ARC-F0002', state: 'open' }], originKind: 'human' }),
  dbQuestion('ARC-F0003', 'Alte Entscheidung', {
    state: 'answered', ready: false,
    decision: { outcome: 'answered', option: null, text: 'ja', reason: '', byId: 'u1', byName: 'Florian', at: '2026-10-16T12:00:00.000Z' },
  }),
  dbQuestion('ARC-F0004', 'Frage eines Kollegen', { mine: false }),
]);

// Detail zu einer Frage der Liste
const detailOf = async (id: string) => dbQuestionDetail(LIST.questions.find((q) => q.id === id) ?? dbQuestion(id, `Frage ${id}`));

// Rendert die Seite mit Live-Anbieter und der Board-Projekt-Auswahl der Seitenleiste unter /p/p1/questions
function open(overrides: Parameters<typeof fakeApi>[0] = {}, path = '/p/p1/questions') {
  const api = fakeApi({ getDbQuestions: vi.fn(async () => LIST), getDbQuestion: vi.fn(detailOf), openEvents: vi.fn(() => new FakeEventSource()), ...overrides });
  render(
    <ApiProvider api={api}>
      <LiveProvider pid="p1">
        <MemoryRouter initialEntries={[path]}>
          <BoardProjectProvider>
            <BoardProjectSwitcher pid="p1" />
            <Routes>
              <Route path="/p/:pid/questions/:id?" element={<DbQuestionsPage />} />
              <Route path="*" element={<div>Andere Seite</div>} />
            </Routes>
          </BoardProjectProvider>
        </MemoryRouter>
      </LiveProvider>
    </ApiProvider>,
  );
  return api;
}

const listRegion = () => screen.getByRole('region', { name: 'Fragenliste' });
const rowOf = (title: string) => within(listRegion()).getByText(title).closest('li')!;

describe('DbQuestionsPage', () => {
  it('zeigt die eigenen offenen Fragen des Projekts des aktiven Boards gruppiert, mit Zählern in den Reitern', async () => {
    const api = open();
    expect(await screen.findByText('Welcher Speicher?')).toBeTruthy();
    expect(api.getDbQuestions).toHaveBeenCalledWith('ARC');
    expect(boardPickerTrigger().getAttribute('aria-label')).toMatch(/^Board wählen: Arches › /);
    expect(within(listRegion()).getAllByRole('heading').map((h) => h.textContent)).toEqual(['Bereit · 1', 'Wartet · 1']);
    const row = rowOf('Welcher Speicher?');
    expect(within(row).getByRole('link', { name: 'Welcher Speicher?' }).getAttribute('href')).toBe('/p/p1/questions/ARC-F0002');
    expect(row.textContent).toContain('ARC-F0002');
    expect(row.textContent).toContain('3 Optionen');
    expect(within(row).getByText('1 Ticket wartet')).toBeTruthy();
    expect(rowOf('Wartende Frage').textContent).toContain('wartet auf ARC-F0002');
    expect(screen.queryByText('Alte Entscheidung')).toBeNull();
    expect(screen.queryByText('Frage eines Kollegen')).toBeNull();
    const tabs = screen.getByRole('tablist', { name: 'Zustand' });
    expect(within(tabs).getAllByRole('tab').map((t) => t.getAttribute('aria-label'))).toEqual(['Eigene (2)', 'Andere (1)', 'Erledigt (1)']);
    expect(screen.getByRole('tab', { name: 'Eigene (2)' }).getAttribute('aria-selected')).toBe('true');
  });

  it('wählt ohne Auswahl die erste Frage der Liste und beim Reiterwechsel die erste des Reiters', async () => {
    const api = open();
    expect(await screen.findByRole('heading', { name: 'Welcher Speicher?', level: 2 })).toBeTruthy();
    expect(api.getDbQuestion).toHaveBeenCalledWith('ARC-F0002');
    expect(within(listRegion()).getByRole('link', { name: 'Welcher Speicher?' }).getAttribute('aria-current')).toBe('page');
    expect(screen.queryByRole('heading', { name: 'Keine Frage gewählt' })).toBeNull();
    await userEvent.click(screen.getByRole('tab', { name: 'Erledigt (1)' }));
    expect(await screen.findByRole('heading', { name: 'Alte Entscheidung', level: 2 })).toBeTruthy();
    expect(screen.getByRole('tab', { name: 'Erledigt (1)' }).getAttribute('aria-selected')).toBe('true');
  });

  it('zeigt unter „Andere“ die offenen Fragen, die den Benutzer nicht betreffen (Kennzeichnung vom Server)', async () => {
    open();
    await screen.findByText('Welcher Speicher?');
    await userEvent.click(screen.getByRole('tab', { name: 'Andere (1)' }));
    const list = within(listRegion());
    expect(list.getByText('Frage eines Kollegen')).toBeTruthy();
    expect(list.queryByText('Welcher Speicher?')).toBeNull();
    expect(list.queryByText('Alte Entscheidung')).toBeNull();
  });

  it('wechselt Zustand, Suche und Projekt', async () => {
    const api = open();
    await screen.findByText('Welcher Speicher?');
    await userEvent.click(screen.getByRole('tab', { name: 'Erledigt (1)' }));
    expect(within(listRegion()).getByRole('heading', { name: 'Beantwortet · 1' })).toBeTruthy();
    expect(rowOf('Alte Entscheidung').textContent).toContain('entschieden 16.10.');
    expect(within(listRegion()).queryByText('Welcher Speicher?')).toBeNull();
    await userEvent.click(screen.getByRole('tab', { name: 'Eigene (2)' }));
    await userEvent.type(screen.getByLabelText('Fragen durchsuchen'), 'speicher');
    expect(within(listRegion()).getByText('Welcher Speicher?')).toBeTruthy();
    expect(within(listRegion()).queryByText('Wartende Frage')).toBeNull();
    await pickBoard('Fremd', 'OTH');
    await waitFor(() => expect(api.getDbQuestions).toHaveBeenLastCalledWith('OTH'));
  });

  it('hat zugängliche Zustandsreiter mit Panel und Pfeiltasten', async () => {
    open();
    await screen.findByText('Welcher Speicher?');
    const mine = screen.getByRole('tab', { name: 'Eigene (2)' });
    const panel = within(listRegion()).getByRole('tabpanel');
    expect(mine.getAttribute('aria-controls')).toBe(panel.id);
    expect(panel.getAttribute('aria-labelledby')).toBe(mine.id);
    expect(mine.tabIndex).toBe(0);
    expect(screen.getByRole('tab', { name: 'Erledigt (1)' }).tabIndex).toBe(-1);
    mine.focus();
    await userEvent.keyboard('{ArrowRight}{ArrowRight}');
    const done = screen.getByRole('tab', { name: 'Erledigt (1)' });
    expect(document.activeElement).toBe(done);
    expect(done.getAttribute('aria-selected')).toBe('true');
    expect(within(listRegion()).getByText('Alte Entscheidung')).toBeTruthy();
    await userEvent.keyboard('{ArrowRight}');
    expect(document.activeElement).toBe(mine);
    await userEvent.keyboard('{End}');
    expect(document.activeElement).toBe(done);
  });

  it('öffnet eine Frage aus der Liste daneben und markiert ihre Zeile', async () => {
    const api = open();
    await userEvent.click(await screen.findByRole('link', { name: 'Welcher Speicher?' }));
    expect(await screen.findByRole('heading', { name: 'Welcher Speicher?', level: 2 })).toBeTruthy();
    expect(api.getDbQuestion).toHaveBeenCalledWith('ARC-F0002');
    expect(screen.getByRole('link', { name: 'Welcher Speicher?' }).getAttribute('aria-current')).toBe('page');
    expect(screen.getByRole('link', { name: 'Alle Fragen' }).getAttribute('href')).toBe('/p/p1/questions?project=ARC');
    expect(screen.queryByRole('heading', { name: 'Keine Frage gewählt' })).toBeNull();
  });

  it('wählt beim Öffnen einer erledigten Frage den Reiter „Erledigt“ und das Projekt der Nummer', async () => {
    const api = open({}, '/p/p1/questions/ARC-F0003');
    await waitFor(() => expect(screen.getByRole('tab', { name: 'Erledigt (1)' }).getAttribute('aria-selected')).toBe('true'));
    expect(api.getDbQuestions).toHaveBeenCalledWith('ARC');
    expect(screen.getByRole('link', { name: 'Alte Entscheidung' }).getAttribute('aria-current')).toBe('page');
  });

  it('klappt Fragenliste und rechte Leiste ein und merkt sich das', async () => {
    open({}, '/p/p1/questions/ARC-F0002');
    await screen.findByRole('heading', { name: 'Welcher Speicher?', level: 2 });
    await userEvent.click(screen.getByRole('button', { name: 'Fragenliste einklappen' }));
    const expand = screen.getByRole('button', { name: 'Fragenliste ausklappen – 2 Fragen' });
    expect(expand.getAttribute('aria-expanded')).toBe('false');
    expect(listRegion().className).toContain('closed');
    expect(window.localStorage.getItem('nexus.questions.list')).toBe('0');
    await userEvent.click(expand);
    expect(screen.getByRole('button', { name: 'Fragenliste einklappen' }).getAttribute('aria-expanded')).toBe('true');
    expect(window.localStorage.getItem('nexus.questions.list')).toBe('1');

    await userEvent.click(screen.getByRole('button', { name: 'Bezüge & Details einklappen' }));
    expect(screen.getByRole('button', { name: 'Bezüge und Details ausklappen – 1 Ticket wartet' })).toBeTruthy();
    expect(window.localStorage.getItem('nexus.questions.rail')).toBe('0');
  });

  it('startet mit der gemerkten eingeklappten Fragenliste', async () => {
    window.localStorage.setItem('nexus.questions.list', '0');
    open();
    expect(await screen.findByRole('button', { name: 'Fragenliste ausklappen – 2 Fragen' })).toBeTruthy();
  });

  it('fällt bei unbekanntem Projekt auf das Projekt des aktiven Boards zurück und sagt es', async () => {
    const api = open({}, '/p/p1/questions?project=XYZ');
    expect(await screen.findByText('Welcher Speicher?')).toBeTruthy();
    expect(api.getDbQuestions).toHaveBeenCalledWith('ARC');
    expect(api.getDbQuestions).not.toHaveBeenCalledWith('XYZ');
    expect(boardPickerTrigger().getAttribute('aria-label')).toMatch(/^Board wählen: Arches › /);
    expect(screen.getByRole('status').textContent).toContain('XYZ');
    expect(screen.getByRole('heading', { name: 'Keine Frage gewählt' })).toBeTruthy();
  });

  it('behält bei 503 während des Nachladens per Live-Meldung die Liste und zeigt einen Hinweis', async () => {
    const getDbQuestions = vi.fn(async () => LIST);
    const api = open({ getDbQuestions });
    await screen.findByText('Welcher Speicher?');
    await waitFor(() => expect(api.openEvents).toHaveBeenLastCalledWith('p1', null, 1));
    getDbQuestions.mockRejectedValue(new ApiError('board-db-unreachable', 'Board-Datenbank ist nicht erreichbar', 503));
    act(() => FakeEventSource.current().emit({ type: 'dbquestions', projectId: 1 }));
    expect((await screen.findByRole('alert')).textContent).toContain('nicht erreichbar');
    expect(within(listRegion()).getByText('Welcher Speicher?')).toBeTruthy();
    expect(screen.queryByText('Board-Datenbank nicht verfügbar')).toBeNull();
    getDbQuestions.mockResolvedValue(LIST);
    act(() => FakeEventSource.current().emit({ type: 'dbquestions', projectId: 1 }));
    await waitFor(() => expect(screen.queryByRole('alert')).toBeNull());
  });

  it('zeigt Leerzustände ohne Fragen', async () => {
    open({ getDbQuestions: vi.fn(async () => dbQuestionList([])) });
    expect(await screen.findByText('Keine eigenen offenen Fragen')).toBeTruthy();
    expect(await screen.findByRole('tab', { name: 'Eigene (0)' })).toBeTruthy();
    await userEvent.click(screen.getByRole('tab', { name: 'Erledigt (0)' }));
    expect(screen.getByText('Noch nichts erledigt')).toBeTruthy();
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

  it('stellt eine neue Frage, hebt sie in der Liste hervor und öffnet sie', async () => {
    const createDbQuestion = vi.fn(async (_key: string, input: DbNewQuestionInput) => dbQuestionDetail(dbQuestion('ARC-F0005', input.title)));
    const getDbQuestions = vi.fn(async () => LIST);
    const api = open({ createDbQuestion, getDbQuestions });
    await screen.findByText('Welcher Speicher?');
    getDbQuestions.mockResolvedValue(dbQuestionList([...LIST.questions, dbQuestion('ARC-F0005', 'Neue Frage hier')]));
    await userEvent.click(screen.getByRole('button', { name: 'Neue Frage' }));
    const dialog = screen.getByRole('dialog', { name: 'Neue Frage in ARC' });
    await userEvent.type(within(dialog).getByLabelText('Titel'), 'Neue Frage hier');
    await userEvent.click(within(dialog).getByRole('button', { name: 'Frage stellen' }));
    expect(createDbQuestion).toHaveBeenCalledWith('ARC', { title: 'Neue Frage hier', occasion: null, tags: [], ticketIds: [] });
    const row = (await within(listRegion()).findByText('Neue Frage hier')).closest('li')!;
    expect(within(row).getByText('neu')).toBeTruthy();
    expect(screen.queryByRole('dialog')).toBeNull();
    await waitFor(() => expect(api.getDbQuestion).toHaveBeenCalledWith('ARC-F0005'));
    expect(within(row).getByRole('link', { name: 'Neue Frage hier' }).getAttribute('aria-current')).toBe('page');
  });
});

describe('DbQuestionsPage: Filter', () => {
  const titles = () => within(listRegion()).queryAllByRole('link').map((l) => l.textContent);

  // Klappt die Filterzeile auf
  async function showFilters() {
    await screen.findByText('Welcher Speicher?');
    const toggle = screen.getByRole('button', { name: 'Filter' });
    expect(toggle.getAttribute('aria-expanded')).toBe('false');
    await userEvent.click(toggle);
    expect(toggle.getAttribute('aria-expanded')).toBe('true');
  }

  it('filtert nach Tag', async () => {
    open();
    await showFilters();
    await userEvent.selectOptions(screen.getByLabelText('Tag'), 'speicher');
    expect(titles()).toEqual(['Welcher Speicher?']);
  });

  it('filtert nach betroffenem Ticket', async () => {
    open();
    await showFilters();
    await userEvent.selectOptions(screen.getByLabelText('Bezug'), 'ARC-0007');
    expect(titles()).toEqual(['Welcher Speicher?']);
  });

  it('filtert nach Herkunft', async () => {
    open();
    await showFilters();
    await userEvent.selectOptions(screen.getByLabelText('Herkunft'), 'human');
    expect(titles()).toEqual(['Wartende Frage']);
    await userEvent.selectOptions(screen.getByLabelText('Herkunft'), 'session');
    expect(titles()).toEqual(['Welcher Speicher?']);
  });

  it('filtert nach „wartet auf“ und „recherchiert“', async () => {
    open();
    await showFilters();
    await userEvent.click(screen.getByRole('button', { name: /wartet auf/ }));
    expect(titles()).toEqual(['Wartende Frage']);
    await userEvent.click(screen.getByRole('button', { name: /wartet auf/ }));
    await userEvent.click(screen.getByRole('button', { name: 'recherchiert' }));
    expect(titles()).toEqual(['Welcher Speicher?']);
  });

  it('filtert per Textsuche und setzt die Filter zurück', async () => {
    open();
    await screen.findByText('Welcher Speicher?');
    await userEvent.type(screen.getByLabelText('Fragen durchsuchen'), 'wartende');
    expect(titles()).toEqual(['Wartende Frage']);
    await userEvent.type(screen.getByLabelText('Fragen durchsuchen'), 'xyz');
    expect(screen.getByText('Keine Fragen mit diesen Filtern')).toBeTruthy();
    await userEvent.click(screen.getByRole('button', { name: 'Filter zurücksetzen' }));
    expect(titles()).toEqual(['Welcher Speicher?', 'Wartende Frage']);
    expect((screen.getByLabelText('Fragen durchsuchen') as HTMLInputElement).value).toBe('');
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

  it('legt eine Folgefrage vorbelegt an, die von der Frage abhängt', async () => {
    const createDbQuestion = vi.fn(async () => dbQuestionDetail(dbQuestion('ARC-F0006', 'x')));
    const onClose = vi.fn();
    const followUp = { questionId: 'ARC-F0001', title: 'Verschlüsseln?', description: 'Verschlüsseln? Auch im Backup.' };
    render(<ApiProvider api={fakeApi({ createDbQuestion })}><DbAskQuestionDialog projectKey="ARC" followUp={followUp} onClose={onClose} onCreated={vi.fn()} /></ApiProvider>);
    const dialog = screen.getByRole('dialog', { name: 'Folgefrage zu ARC-F0001' });
    expect(within(dialog).getByText(/Die neue Frage hängt von ARC-F0001 ab\./)).toBeTruthy();
    expect((screen.getByLabelText('Titel') as HTMLInputElement).value).toBe('Verschlüsseln?');
    expect((screen.getByLabelText('Beschreibung') as HTMLTextAreaElement).value).toBe('Verschlüsseln? Auch im Backup.');
    expect((screen.getByLabelText('Anlass') as HTMLInputElement).value).toBe('Folgefrage zu ARC-F0001');
    await userEvent.click(screen.getByRole('button', { name: 'Frage stellen' }));
    expect(createDbQuestion).toHaveBeenCalledWith('ARC', {
      title: 'Verschlüsseln?', description: 'Verschlüsseln? Auch im Backup.', occasion: 'Folgefrage zu ARC-F0001', tags: [], ticketIds: [], dependsOn: ['ARC-F0001'],
    });
  });

  it('schließt eine unveränderte Folgefrage ohne Nachfrage', async () => {
    const onClose = vi.fn();
    const followUp = { questionId: 'ARC-F0001', title: 'Verschlüsseln?', description: '' };
    render(<ApiProvider api={fakeApi()}><DbAskQuestionDialog projectKey="ARC" followUp={followUp} onClose={onClose} onCreated={vi.fn()} /></ApiProvider>);
    await userEvent.keyboard('{Escape}');
    expect(onClose).toHaveBeenCalled();
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
