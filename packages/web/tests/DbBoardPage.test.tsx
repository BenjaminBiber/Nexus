// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter } from 'react-router-dom';
import { App } from '../src/App.js';
import { ApiProvider } from '../src/api/ApiContext.js';
import { ApiError } from '../src/api/ApiError.js';
import { DbBoardPage } from '../src/pages/DbBoardPage.js';
import { FakeEventSource } from './helpers/FakeEventSource.js';
import { dbActive, dbBoards, dbCard, dbDetail, dbView, fakeApi } from './helpers/fakeApi.js';
import { boardPickerTrigger, pickBoard, pickProject } from './helpers/boardPicker.js';
import { renderWithApp } from './helpers/renderWithApp.js';

beforeEach(() => FakeEventSource.reset());
afterEach(cleanup);

const CARDS = [
  dbCard('ARC-0001', 'Login bauen', 'backlog', { priority: 'urgent', labels: ['ui'], assignee: 'claude', needsPlanning: true, commentCount: 2, hasPlan: true }),
  dbCard('ARC-0002', 'Token prüfen', 'blocked', { blockedReason: 'Rückfrage' }),
  dbCard('ARC-0003', 'Wartet hier', 'backlog', { column: 'waiting', waiting: true, dependsOpen: ['ARC-0001'] }),
  dbCard('ARC-0004', 'Server', 'blocked', { blockedReason: 'DB fehlt' }),
];

// Rendert die ganze App (mit Seitenleiste und deren Board-Projekt-Auswahl) unter einer Adresse
function openApp(overrides: Parameters<typeof fakeApi>[0] = {}, path = '/p/p1/board') {
  const api = fakeApi({ getDbBoardView: vi.fn(async () => dbView(CARDS)), ...overrides });
  render(
    <ApiProvider api={api}>
      <MemoryRouter initialEntries={[path]}>
        <App />
      </MemoryRouter>
    </ApiProvider>,
  );
  return api;
}

// Rendert die Seite unter /p/p1/board mit austauschbaren Aufrufen
function open(overrides: Parameters<typeof fakeApi>[0] = {}, path = '/p/p1/board') {
  const api = fakeApi({ getDbBoardView: vi.fn(async () => dbView(CARDS)), ...overrides });
  renderWithApp(<DbBoardPage />, { api, path, route: '/p/:pid/board/*' });
  return api;
}

describe('DbBoardPage', () => {
  it('blendet die Spalte „Nicht mehr nötig“ nach der Darstellungseinstellung aus', async () => {
    open({ getUserSettings: async () => ({ notifications: 'off', heroImages: false, theme: 'system', claudeUsage: false, hideNotNeeded: true }) });
    await screen.findByText('Login bauen');
    await waitFor(() => expect(screen.queryByRole('heading', { level: 2, name: 'Nicht mehr nötig' })).toBeNull());
    expect(screen.getByRole('heading', { level: 2, name: 'Erledigt' })).toBeTruthy();
    // Das Raster verteilt die Breite auf die übrigen sieben Spalten
    expect((document.querySelector('.db-columns') as HTMLElement).style.getPropertyValue('--db-column-count')).toBe('7');
  });

  it('blendet die Spalte „In Planung“ nach der Darstellungseinstellung aus', async () => {
    open({ getUserSettings: async () => ({ notifications: 'off', heroImages: false, theme: 'system', claudeUsage: false, compactCards: false, hideNotNeeded: false, hideInPlanning: true }) });
    await screen.findByText('Login bauen');
    await waitFor(() => expect(screen.queryByRole('heading', { level: 2, name: 'In Planung' })).toBeNull());
    expect(screen.getByRole('heading', { level: 2, name: 'Nicht mehr nötig' })).toBeTruthy();
    expect((document.querySelector('.db-columns') as HTMLElement).style.getPropertyValue('--db-column-count')).toBe('7');
  });

  it('zeigt die Spalten in fester Reihenfolge und Karten mit beschrifteten Kennzeichen', async () => {
    open();
    await screen.findByText('Login bauen');
    expect(screen.getAllByRole('heading', { level: 2 }).map((h) => h.textContent)).toEqual(
      ['Backlog', 'Wartet', 'In Planung', 'Geplant', 'In Arbeit', 'Blockiert', 'Erledigt', 'Nicht mehr nötig'],
    );
    const card = screen.getByRole('button', { name: /^ARC-0001/ });
    for (const text of ['dringend', '@claude', 'Planung', 'Plan', '2 Kommentare', 'ui']) expect(within(card).getByText(text), text).toBeTruthy();
    expect(screen.getByText('Rückfrage').getAttribute('title')).toBe('Rückfrage');
    expect(screen.getByText('Blockiert', { selector: '.badge' }).getAttribute('title')).toBe('DB fehlt');
    expect(within(screen.getByRole('region', { name: 'Wartet' })).getByText('wartet auf ARC-0001')).toBeTruthy();
  });

  it('zeigt die Tags ohne die kompakte Ansicht direkt auf der Karte', async () => {
    open();
    await screen.findByText('Login bauen');
    expect(screen.getByText('ui')).toBeTruthy();
    expect(screen.queryByRole('button', { name: /^Tags/ })).toBeNull();
  });

  it('klappt die Tags in der kompakten Ansicht hinter einen Umschalter', async () => {
    open({ getUserSettings: async () => ({ notifications: 'off', heroImages: false, theme: 'system', claudeUsage: false, compactCards: true }) });
    await screen.findByText('Login bauen');
    const toggle = await screen.findByRole('button', { name: 'Tags (1)' });
    expect(screen.queryByText('ui')).toBeNull();
    expect(toggle.getAttribute('aria-expanded')).toBe('false');
    await userEvent.click(toggle);
    expect(screen.getByText('ui')).toBeTruthy();
    expect(screen.getByRole('button', { name: 'Tags (1)' }).getAttribute('aria-expanded')).toBe('true');
    await userEvent.click(toggle);
    expect(screen.queryByText('ui')).toBeNull();
  });

  it('filtert mit der Board-Suche nur die Karten des aktuellen Boards; „Suchen“ in der Seitenleiste öffnet die globale Suche', async () => {
    openApp();
    await screen.findByText('Login bauen');
    await userEvent.type(screen.getByRole('searchbox', { name: 'Suche im Board' }), 'token');
    expect(screen.getByText('Token prüfen')).toBeTruthy();
    expect(screen.queryByText('Login bauen')).toBeNull();
    await userEvent.click(screen.getByRole('button', { name: /^Suchen/ }));
    expect(screen.getByRole('dialog', { name: 'Suche über alle Boards' })).toBeTruthy();
  });

  it('wechselt das Board über die Auswahl und lädt dessen Ansicht', async () => {
    const activateDbBoard = vi.fn(async (id: number) => dbActive(id));
    const getDbBoardView = vi.fn(async (id: number) => dbView(id === 2 ? [dbCard('ARC-0009', 'Anderes Board', 'planned', { boardId: 2 })] : CARDS, id));
    openApp({ activateDbBoard, getDbBoardView });
    await screen.findByText('Login bauen');
    expect(screen.queryByRole('combobox', { name: 'Board' })).toBeNull();
    expect(boardPickerTrigger().getAttribute('aria-label')).toBe('Board wählen: Arches › Sprint');
    await userEvent.click(boardPickerTrigger());
    const boardList = screen.getByRole('list', { name: 'Boards in ARC' });
    expect(within(boardList).getAllByRole('button').map((b) => b.querySelector('.bp-item-name')?.textContent)).toEqual(['Sprint', 'Nächster']);
    await userEvent.click(within(boardList).getByRole('button', { name: /^Nächster/ }));
    expect(activateDbBoard).toHaveBeenCalledWith(2);
    expect(await screen.findByText('Anderes Board')).toBeTruthy();
    expect(screen.queryByRole('dialog', { name: 'Board wählen' })).toBeNull();
  });

  it('wählt beim Klick auf ein Projekt dessen erstes Board und zeigt im Feld kein Kürzel', async () => {
    const activateDbBoard = vi.fn(async (id: number) => dbActive(id));
    openApp({ activateDbBoard, getDbBoards: async () => [dbBoards[2], dbBoards[0], dbBoards[1]] });
    await screen.findByText('Login bauen');
    expect(boardPickerTrigger().textContent).toBe('ArchesSprint');
    await pickProject('OTH');
    expect(activateDbBoard).toHaveBeenCalledWith(3);
    activateDbBoard.mockClear();
    await pickProject('ARC');
    expect(activateDbBoard).toHaveBeenCalledWith(1);
    expect(screen.queryByRole('dialog', { name: 'Board wählen' })).toBeNull();
  });

  it('wählt in der Seitenleiste ein Board eines anderen Projekts', async () => {
    const activateDbBoard = vi.fn(async (id: number) => dbActive(id));
    openApp({ activateDbBoard });
    await screen.findByText('Login bauen');
    await pickBoard('Fremd', 'OTH');
    expect(activateDbBoard).toHaveBeenCalledWith(3);
  });

  it('filtert die Board-Auswahl über die Suche und wählt mit Enter das erste Treffer-Board', async () => {
    const activateDbBoard = vi.fn(async (id: number) => dbActive(id));
    openApp({ activateDbBoard });
    await screen.findByText('Login bauen');
    await userEvent.click(boardPickerTrigger());
    const search = screen.getByRole('searchbox', { name: 'Projekt oder Board suchen' });
    expect(document.activeElement).toBe(search);
    await userEvent.type(search, 'fremd');
    const dialog = screen.getByRole('dialog', { name: 'Board wählen' });
    expect(within(within(dialog).getByRole('list', { name: 'Projekte' })).getAllByRole('button').map((b) => b.querySelector('.bp-key')?.textContent)).toEqual(['OTH']);
    await userEvent.keyboard('{Enter}');
    expect(activateDbBoard).toHaveBeenCalledWith(3);
  });

  it('schließt die Board-Auswahl mit Esc und gibt den Fokus an den Auslöser zurück', async () => {
    openApp();
    await screen.findByText('Login bauen');
    await userEvent.click(boardPickerTrigger());
    expect(screen.getByRole('dialog', { name: 'Board wählen' })).toBeTruthy();
    await userEvent.keyboard('{Escape}');
    expect(screen.queryByRole('dialog', { name: 'Board wählen' })).toBeNull();
    expect(document.activeElement).toBe(boardPickerTrigger());
  });

  it('meldet den Wechsel des aktiven Boards (für den Zähler am Reiter „Fragen“)', async () => {
    const listener = vi.fn();
    window.addEventListener('harness:db-active-changed', listener);
    try {
      openApp({ activateDbBoard: vi.fn(async (id: number) => dbActive(id)) });
      await screen.findByText('Login bauen');
      await pickBoard('Fremd', 'OTH');
      await waitFor(() => expect(listener).toHaveBeenCalledTimes(1));
    } finally {
      window.removeEventListener('harness:db-active-changed', listener);
    }
  });

  it('verschiebt eine Karte beim Ablegen sofort und nimmt sie bei Fehler zurück', async () => {
    let fail: (error: Error) => void = () => undefined;
    const moveDbTicket = vi.fn(() => new Promise((_resolve, reject) => { fail = reject; }));
    open({ moveDbTicket });
    await screen.findByText('Login bauen');
    fireEvent.drop(screen.getByRole('region', { name: 'In Arbeit' }), { dataTransfer: { getData: () => 'ARC-0001' } });
    expect(within(screen.getByRole('region', { name: 'In Arbeit' })).getByText('Login bauen')).toBeTruthy();
    expect(moveDbTicket).toHaveBeenCalledWith('ARC-0001', { status: 'in_progress', rev: 1 });
    await act(async () => fail(new ApiError('dependency-open', 'ARC-0001 wartet auf ARC-0000.', 422)));
    expect((await screen.findByRole('alert')).textContent).toContain('wartet auf ARC-0000');
    expect(within(screen.getByRole('region', { name: 'Backlog' })).getByText('Login bauen')).toBeTruthy();
  });

  it('lädt nach einem Konflikt (409) die Ansicht neu und nimmt die Karte zurück', async () => {
    const getDbBoardView = vi.fn(async () => dbView(CARDS));
    const moveDbTicket = vi.fn(async () => { throw new ApiError('conflict', 'Ticket wurde geändert.', 409); });
    open({ getDbBoardView, moveDbTicket });
    await screen.findByText('Login bauen');
    fireEvent.drop(screen.getByRole('region', { name: 'In Arbeit' }), { dataTransfer: { getData: () => 'ARC-0001' } });
    expect((await screen.findByRole('alert')).textContent).toContain('geändert');
    expect(within(screen.getByRole('region', { name: 'Backlog' })).getByText('Login bauen')).toBeTruthy();
    await waitFor(() => expect(getDbBoardView).toHaveBeenCalledTimes(2));
  });

  it('ignoriert beim wartenden Ticket die Wahl des eigenen Status', async () => {
    const moveDbTicket = vi.fn();
    open({ moveDbTicket });
    await screen.findByText('Wartet hier');
    const select = screen.getByLabelText('Status von ARC-0003 ändern') as HTMLSelectElement;
    expect(select.value).toBe('backlog');
    await userEvent.selectOptions(select, 'backlog');
    expect(moveDbTicket).not.toHaveBeenCalled();
  });

  it('lädt nach erfolgreichem Verschieben neu und bietet das Statusmenü als Tastatur-Alternative', async () => {
    const getDbBoardView = vi.fn(async () => dbView(CARDS));
    const moveDbTicket = vi.fn(async () => CARDS[0]);
    open({ getDbBoardView, moveDbTicket });
    await screen.findByText('Login bauen');
    await userEvent.selectOptions(screen.getByLabelText('Status von ARC-0001 ändern'), 'planned');
    expect(moveDbTicket).toHaveBeenCalledWith('ARC-0001', { status: 'planned', rev: 1 });
    await waitFor(() => expect(getDbBoardView).toHaveBeenCalledTimes(2));
  });

  it('nimmt in „Wartet“ nichts an und erklärt warum', async () => {
    const moveDbTicket = vi.fn();
    open({ moveDbTicket });
    await screen.findByText('Login bauen');
    fireEvent.drop(screen.getByRole('region', { name: 'Wartet' }), { dataTransfer: { getData: () => 'ARC-0001' } });
    expect(await screen.findByText(/nimmt keine Karten an/)).toBeTruthy();
    expect(moveDbTicket).not.toHaveBeenCalled();
  });

  it('zeigt die Zahl offener Notizen', async () => {
    open({ getDbNotes: async () => [{ id: 1 }, { id: 2 }] });
    expect(await screen.findByRole('button', { name: 'Notizen (2)' })).toBeTruthy();
  });

  it('zeigt ohne aktives Board einen Hinweis', async () => {
    open({ getDbActive: async () => dbActive(null) });
    expect(await screen.findByText('Kein Board gewählt')).toBeTruthy();
  });

  it('zeigt bei nicht verfügbarer Board-Datenbank den Hinweis mit Link zu den Einstellungen', async () => {
    open({ getDbActive: async () => { throw new ApiError('board-config-missing', 'board.yaml fehlt', 503); } });
    const alert = await screen.findByRole('alert');
    expect(alert.textContent).toContain('board.yaml fehlt');
    expect(within(alert).getByRole('link', { name: 'Einstellungen → Board-Datenbank' }).getAttribute('href')).toBe('/p/p1/settings#board-db');
  });

  it('beobachtet das aktive Board und lädt bei einer Live-Meldung neu', async () => {
    const getDbBoardView = vi.fn(async () => dbView(CARDS));
    const openEvents = vi.fn((_pid: string, _board?: number | null, _project?: number | null) => new FakeEventSource());
    render(
      <ApiProvider api={fakeApi({ getDbBoardView, openEvents })}>
        <MemoryRouter initialEntries={['/p/p1/board']}>
          <App />
        </MemoryRouter>
      </ApiProvider>,
    );
    await screen.findByText('Login bauen');
    // Board 1 und (über den Reiter „Fragen“) dessen Projekt 1
    await waitFor(() => expect(openEvents).toHaveBeenLastCalledWith('p1', 1, 1));
    act(() => FakeEventSource.current().emit({ type: 'dbboard', boardId: 1 }));
    await waitFor(() => expect(getDbBoardView).toHaveBeenCalledTimes(2));
  });

});

describe('DbBoardPage – Abnahme beim Verschieben nach „Erledigt“', () => {
  const QUESTION = 'Woran sieht man, dass es fertig ist?';

  it('fragt beim Ablegen auf „Erledigt“ ohne Abnahme nach und verschiebt mit Abnahme', async () => {
    const moveDbTicket = vi.fn(() => new Promise<never>(() => undefined));
    open({ moveDbTicket });
    await screen.findByText('Login bauen');
    fireEvent.drop(screen.getByRole('region', { name: 'Erledigt' }), { dataTransfer: { getData: () => 'ARC-0001' } });
    const dialog = await screen.findByRole('dialog', { name: QUESTION });
    expect(moveDbTicket).not.toHaveBeenCalled();
    expect((within(dialog).getByRole('button', { name: 'Verschieben' }) as HTMLButtonElement).disabled).toBe(true);
    await userEvent.type(within(dialog).getByLabelText(QUESTION), 'Login klappt');
    await userEvent.click(within(dialog).getByRole('button', { name: 'Verschieben' }));
    expect(moveDbTicket).toHaveBeenCalledWith('ARC-0001', { status: 'done', rev: 1, acceptance: 'Login klappt' });
    expect(screen.queryByRole('dialog')).toBeNull();
    expect(within(screen.getByRole('region', { name: 'Erledigt' })).getByText('Login bauen')).toBeTruthy();
  });

  it('schließt den Dialog mit Escape auch ohne Fokus darin und gibt den Fokus zurück', async () => {
    const moveDbTicket = vi.fn();
    open({ moveDbTicket });
    await screen.findByText('Login bauen');
    const select = screen.getByLabelText('Status von ARC-0001 ändern');
    select.focus();
    await userEvent.selectOptions(select, 'done');
    await screen.findByRole('dialog', { name: QUESTION });
    (document.activeElement as HTMLElement).blur();
    fireEvent.keyDown(screen.getByRole('dialog'), { key: 'Escape' });
    expect(screen.queryByRole('dialog')).toBeNull();
    expect(moveDbTicket).not.toHaveBeenCalled();
    expect(document.activeElement).toBe(select);
  });

  it('lässt beim Abbrechen alles unverändert', async () => {
    const moveDbTicket = vi.fn();
    open({ moveDbTicket });
    await screen.findByText('Login bauen');
    fireEvent.drop(screen.getByRole('region', { name: 'Erledigt' }), { dataTransfer: { getData: () => 'ARC-0001' } });
    const dialog = await screen.findByRole('dialog', { name: QUESTION });
    await userEvent.click(within(dialog).getByRole('button', { name: 'Abbrechen' }));
    expect(screen.queryByRole('dialog')).toBeNull();
    expect(moveDbTicket).not.toHaveBeenCalled();
    expect(within(screen.getByRole('region', { name: 'Backlog' })).getByText('Login bauen')).toBeTruthy();
    expect((screen.getByLabelText('Status von ARC-0001 ändern') as HTMLSelectElement).value).toBe('backlog');
  });

  it('nutzt denselben Dialog im Statusmenü', async () => {
    const moveDbTicket = vi.fn(async () => CARDS[0]);
    open({ moveDbTicket });
    await screen.findByText('Login bauen');
    await userEvent.selectOptions(screen.getByLabelText('Status von ARC-0001 ändern'), 'done');
    const dialog = await screen.findByRole('dialog', { name: QUESTION });
    expect(moveDbTicket).not.toHaveBeenCalled();
    await userEvent.type(within(dialog).getByLabelText(QUESTION), 'Test grün');
    await userEvent.click(within(dialog).getByRole('button', { name: 'Verschieben' }));
    expect(moveDbTicket).toHaveBeenCalledWith('ARC-0001', { status: 'done', rev: 1, acceptance: 'Test grün' });
  });

  it('verschiebt mit vorhandener Abnahme ohne Nachfrage', async () => {
    const moveDbTicket = vi.fn(async () => CARDS[0]);
    const cards = [dbCard('ARC-0005', 'Fertig machen', 'in_progress', { hasAcceptance: true })];
    open({ moveDbTicket, getDbBoardView: vi.fn(async () => dbView(cards)) });
    await screen.findByText('Fertig machen');
    fireEvent.drop(screen.getByRole('region', { name: 'Erledigt' }), { dataTransfer: { getData: () => 'ARC-0005' } });
    expect(moveDbTicket).toHaveBeenCalledWith('ARC-0005', { status: 'done', rev: 1 });
    expect(screen.queryByRole('dialog')).toBeNull();
  });

  it('fragt nach, wenn der Server die Abnahme vermisst', async () => {
    const moveDbTicket = vi.fn()
      .mockRejectedValueOnce(new ApiError('acceptance-missing', 'ARC-0005 hat keine Abnahme.', 422))
      .mockResolvedValueOnce(CARDS[0]);
    const cards = [dbCard('ARC-0005', 'Fertig machen', 'in_progress', { hasAcceptance: true })];
    open({ moveDbTicket, getDbBoardView: vi.fn(async () => dbView(cards)) });
    await screen.findByText('Fertig machen');
    fireEvent.drop(screen.getByRole('region', { name: 'Erledigt' }), { dataTransfer: { getData: () => 'ARC-0005' } });
    const dialog = await screen.findByRole('dialog', { name: QUESTION });
    expect(screen.queryByRole('alert')).toBeNull();
    expect(within(screen.getByRole('region', { name: 'In Arbeit' })).getByText('Fertig machen')).toBeTruthy();
    await userEvent.type(within(dialog).getByLabelText(QUESTION), 'Neu');
    await userEvent.click(within(dialog).getByRole('button', { name: 'Verschieben' }));
    expect(moveDbTicket).toHaveBeenLastCalledWith('ARC-0005', { status: 'done', rev: 1, acceptance: 'Neu' });
  });
});

describe('DbBoardPage – ungespeicherte Eingaben im Ticket-Panel', () => {
  const detailOf = async (id: string) => dbDetail(CARDS.find((c) => c.id === id)!);

  it('fragt vor dem Öffnen eines anderen Tickets nach und wechselt erst nach „Verwerfen“', async () => {
    open({ getDbTicket: vi.fn(detailOf) }, '/p/p1/board/t/ARC-0001');
    await userEvent.click(await screen.findByRole('button', { name: 'Bearbeiten' }));
    await userEvent.type(screen.getByRole('textbox', { name: 'Beschreibung' }), 'Entwurf');
    await userEvent.click(screen.getByRole('button', { name: /^ARC-0002/ }));
    expect(await screen.findByRole('alertdialog', { name: 'Ungespeicherte Änderungen' })).toBeTruthy();
    expect(screen.getByRole('dialog', { name: 'Ticket ARC-0001' })).toBeTruthy();
    await userEvent.click(screen.getByRole('button', { name: 'Verwerfen' }));
    expect(await screen.findByRole('dialog', { name: 'Ticket ARC-0002' })).toBeTruthy();
  });

  it('fragt vor dem Board-Wechsel nach', async () => {
    const activateDbBoard = vi.fn(async (id: number) => dbActive(id));
    openApp({ getDbTicket: vi.fn(detailOf), activateDbBoard }, '/p/p1/board/t/ARC-0001');
    await userEvent.click(await screen.findByRole('button', { name: 'Bearbeiten' }));
    await userEvent.type(screen.getByRole('textbox', { name: 'Beschreibung' }), 'Entwurf');
    await pickBoard('Nächster');
    expect(await screen.findByRole('alertdialog', { name: 'Ungespeicherte Änderungen' })).toBeTruthy();
    expect(activateDbBoard).not.toHaveBeenCalled();
    await userEvent.click(screen.getByRole('button', { name: 'Verwerfen' }));
    await waitFor(() => expect(activateDbBoard).toHaveBeenCalledWith(2));
    expect(screen.queryByRole('alertdialog', { name: 'Ungespeicherte Änderungen' })).toBeNull();
    expect(screen.queryByRole('textbox', { name: 'Beschreibung' })).toBeNull();
  });
});

describe('DbBoardPage – Ticket-Panel', () => {
  const detailOf = async (id: string) => dbDetail(CARDS.find((c) => c.id === id)!);

  it('holt beim Öffnen per Tastatur den Fokus ins Panel und gibt ihn nach Esc an die Karte zurück', async () => {
    open({ getDbTicket: vi.fn(detailOf) });
    const card = await screen.findByRole('button', { name: /^ARC-0001/ });
    card.focus();
    await userEvent.keyboard('{Enter}');
    const panel = await screen.findByRole('dialog', { name: 'Ticket ARC-0001' });
    await waitFor(() => expect(panel.contains(document.activeElement)).toBe(true));
    await userEvent.keyboard('{Escape}');
    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Ticket ARC-0001' })).toBeNull());
    expect(document.activeElement).toBe(screen.getByRole('button', { name: /^ARC-0001/ }));
  });

  it('schließt das Panel nach einem Board-Wechsel', async () => {
    const activateDbBoard = vi.fn(async (id: number) => dbActive(id));
    openApp({ getDbTicket: vi.fn(detailOf), activateDbBoard }, '/p/p1/board/t/ARC-0001');
    await screen.findByRole('dialog', { name: 'Ticket ARC-0001' });
    await pickBoard('Nächster');
    await waitFor(() => expect(activateDbBoard).toHaveBeenCalledWith(2));
    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Ticket ARC-0001' })).toBeNull());
  });

  it('schließt das Panel nach einem Projekt-Wechsel', async () => {
    const activateDbBoard = vi.fn(async (id: number) => dbActive(id));
    openApp({ getDbTicket: vi.fn(detailOf), activateDbBoard }, '/p/p1/board/t/ARC-0001');
    await screen.findByRole('dialog', { name: 'Ticket ARC-0001' });
    await pickBoard('Fremd', 'OTH');
    await waitFor(() => expect(activateDbBoard).toHaveBeenCalledWith(3));
    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Ticket ARC-0001' })).toBeNull());
  });
});

describe('DbBoardPage – Klick aufs Board schließt das Ticket-Panel', () => {
  const detailOf = async (id: string) => dbDetail(CARDS.find((c) => c.id === id)!);
  const emptyArea = () => within(screen.getByRole('region', { name: 'Geplant' })).getByText('Keine Tickets');
  const panelName = 'Ticket ARC-0001';

  it('schließt bei Klick auf einen leeren Spaltenbereich', async () => {
    open({ getDbTicket: vi.fn(detailOf) }, '/p/p1/board/t/ARC-0001');
    await screen.findByRole('dialog', { name: panelName });
    await userEvent.click(emptyArea());
    await waitFor(() => expect(screen.queryByRole('dialog', { name: panelName })).toBeNull());
  });

  it('schließt bei Klick auf einen Spaltenkopf', async () => {
    open({ getDbTicket: vi.fn(detailOf) }, '/p/p1/board/t/ARC-0001');
    await screen.findByRole('dialog', { name: panelName });
    await userEvent.click(screen.getByRole('heading', { name: 'Geplant' }));
    await waitFor(() => expect(screen.queryByRole('dialog', { name: panelName })).toBeNull());
  });

  it('öffnet bei Klick auf eine andere Karte diese Karte', async () => {
    open({ getDbTicket: vi.fn(detailOf) }, '/p/p1/board/t/ARC-0001');
    await screen.findByRole('dialog', { name: panelName });
    await userEvent.click(screen.getByRole('button', { name: /^ARC-0002/ }));
    expect(await screen.findByRole('dialog', { name: 'Ticket ARC-0002' })).toBeTruthy();
  });

  it('schließt nicht bei Klick im Panel', async () => {
    open({ getDbTicket: vi.fn(detailOf) }, '/p/p1/board/t/ARC-0001');
    const panel = await screen.findByRole('dialog', { name: panelName });
    await userEvent.click(await within(panel).findByRole('textbox', { name: 'Titel' }));
    expect(screen.getByRole('dialog', { name: panelName })).toBeTruthy();
  });

  it('schließt nicht bei Klick auf Kopfsteuerelemente', async () => {
    openApp({ getDbTicket: vi.fn(detailOf) }, '/p/p1/board/t/ARC-0001');
    await screen.findByRole('dialog', { name: panelName });
    await userEvent.click(boardPickerTrigger());
    expect(screen.getByRole('dialog', { name: panelName })).toBeTruthy();
  });

  it('fragt bei ungespeicherter Eingabe erst nach', async () => {
    open({ getDbTicket: vi.fn(detailOf) }, '/p/p1/board/t/ARC-0001');
    await userEvent.click(await screen.findByRole('button', { name: 'Bearbeiten' }));
    await userEvent.type(screen.getByRole('textbox', { name: 'Beschreibung' }), 'Entwurf');
    await userEvent.click(emptyArea());
    expect(await screen.findByRole('alertdialog', { name: 'Ungespeicherte Änderungen' })).toBeTruthy();
    expect(screen.getByRole('dialog', { name: panelName })).toBeTruthy();
    await userEvent.click(screen.getByRole('button', { name: 'Verwerfen' }));
    await waitFor(() => expect(screen.queryByRole('dialog', { name: panelName })).toBeNull());
  });

  it('schließt nicht, wenn zuvor eine Karte gezogen wurde', async () => {
    open({ getDbTicket: vi.fn(detailOf) }, '/p/p1/board/t/ARC-0001');
    await screen.findByRole('dialog', { name: panelName });
    fireEvent.dragStart(screen.getByRole('button', { name: /^ARC-0002/ }).closest('article')!, { dataTransfer: { setData: () => undefined } });
    await userEvent.click(emptyArea());
    expect(screen.getByRole('dialog', { name: panelName })).toBeTruthy();
  });

  it('schluckt keinen späteren Klick, wenn das Ziehen ohne Klick endete', async () => {
    open({ getDbTicket: vi.fn(detailOf) }, '/p/p1/board/t/ARC-0001');
    await screen.findByRole('dialog', { name: panelName });
    const card = screen.getByRole('button', { name: /^ARC-0002/ }).closest('article')!;
    fireEvent.dragStart(card, { dataTransfer: { setData: () => undefined } });
    fireEvent.dragEnd(card);
    fireEvent.click(emptyArea());
    await waitFor(() => expect(screen.queryByRole('dialog', { name: panelName })).toBeNull());
  });
});

describe('DbBoardPage – Hinweise verschwinden automatisch', () => {
  afterEach(() => vi.useRealTimers());
  const advance = (ms: number) => act(async () => { vi.advanceTimersByTime(ms); });

  it('blendet Info-Hinweise nach 4 s aus und startet bei neuer Meldung neu', async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    open();
    await screen.findByText('Login bauen');
    const drop = () => fireEvent.drop(screen.getByRole('region', { name: 'Wartet' }), { dataTransfer: { getData: () => 'ARC-0001' } });
    drop();
    expect(await screen.findByRole('status')).toBeTruthy();
    await advance(3000);
    drop();
    await advance(3000);
    expect(screen.queryByRole('status')).not.toBeNull();
    await advance(1100);
    expect(screen.queryByRole('status')).toBeNull();
  });

  it('lässt Fehlermeldungen stehen', async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    const moveDbTicket = vi.fn(async () => { throw new ApiError('dependency-open', 'ARC-0001 wartet auf ARC-0000.', 422); });
    open({ moveDbTicket });
    await screen.findByText('Login bauen');
    fireEvent.drop(screen.getByRole('region', { name: 'In Arbeit' }), { dataTransfer: { getData: () => 'ARC-0001' } });
    expect(await screen.findByRole('alert')).toBeTruthy();
    await advance(10000);
    expect(screen.getByRole('alert')).toBeTruthy();
  });
});

describe('DbBoardPage – Knopf „Ticket hinzufügen“', () => {
  it('hat die Einzelknöpfe nicht mehr in der Kopfzeile und klappt ein Menü auf', async () => {
    open();
    await screen.findByText('Login bauen');
    expect(screen.queryByRole('button', { name: '+ Ticket' })).toBeNull();
    expect(screen.queryByRole('button', { name: 'Von anderem Board holen' })).toBeNull();
    const fab = screen.getByRole('button', { name: 'Ticket hinzufügen' });
    expect(fab.getAttribute('aria-expanded')).toBe('false');
    await userEvent.click(fab);
    expect(fab.getAttribute('aria-expanded')).toBe('true');
    expect(screen.getAllByRole('menuitem').map((m) => m.textContent)).toEqual(['Neues Ticket', 'Von anderem Board holen']);
    await userEvent.keyboard('{Escape}');
    expect(screen.queryByRole('menu')).toBeNull();
    expect(document.activeElement).toBe(fab);
    await userEvent.click(fab);
    await userEvent.click(document.body);
    expect(screen.queryByRole('menu')).toBeNull();
  });
});

describe('DbBoardPage – Tickets von anderem Board holen', () => {
  afterEach(() => vi.useRealTimers());
  // Der Eintrag steckt im Menü des Knopfes „Ticket hinzufügen“
  const button = async () => {
    await userEvent.click(screen.getByRole('button', { name: 'Ticket hinzufügen' }));
    return screen.getByRole('menuitem', { name: 'Von anderem Board holen' }) as HTMLButtonElement;
  };

  it('ist gesperrt ohne aktives Board und ohne weiteres Board im Projekt', async () => {
    open({ getDbBoards: async () => [dbBoards[0], dbBoards[2]] });
    await screen.findByText('Login bauen');
    expect((await button()).disabled).toBe(true);
    cleanup();
    open({ getDbActive: async () => dbActive(null) });
    await screen.findByText('Kein Board gewählt');
    expect((screen.getByRole('button', { name: 'Ticket hinzufügen' }) as HTMLButtonElement).disabled).toBe(true); // ganzer Knopf gesperrt
    cleanup();
    open();
    await screen.findByText('Login bauen');
    expect((await button()).disabled).toBe(false);
  });

  it('holt Tickets, lädt das Board neu und blendet den Hinweis nach 4 s aus', async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    const source = [dbCard('ARC-0011', 'Fremdes A', 'backlog', { boardId: 2 }), dbCard('ARC-0012', 'Fremdes B', 'planned', { boardId: 2 })];
    const getDbBoardView = vi.fn(async (id: number) => dbView(id === 2 ? source : CARDS, id));
    const importDbTickets = vi.fn(async () => ({ moved: source }));
    open({ getDbBoardView, importDbTickets });
    await screen.findByText('Login bauen');
    await userEvent.click(await button());
    const dialog = await screen.findByRole('dialog', { name: 'Von anderem Board holen' });
    expect(Array.from((within(dialog).getByLabelText('Quell-Board') as HTMLSelectElement).options).map((o) => o.text)).toEqual(['Nächster (Florian)']);
    await userEvent.click(await within(dialog).findByRole('checkbox', { name: /^ARC-0011/ }));
    await userEvent.click(within(dialog).getByRole('checkbox', { name: /^ARC-0012/ }));
    await userEvent.click(within(dialog).getByRole('button', { name: 'Importieren (2)' }));
    expect(importDbTickets).toHaveBeenCalledWith(1, ['ARC-0011', 'ARC-0012']);
    expect((await screen.findByRole('status')).textContent).toBe('2 Tickets geholt.');
    expect(screen.queryByRole('dialog', { name: 'Von anderem Board holen' })).toBeNull();
    expect(getDbBoardView.mock.calls.filter(([id]) => id === 1).length).toBe(2);
    await act(async () => { vi.advanceTimersByTime(4100); });
    expect(screen.queryByRole('status')).toBeNull();
  });
});

describe('DbBoardPage – Karten mit offener Frage', () => {
  const BLOCKED = 'Wartet auf Frage ARC-F0001, ARC-F0002 – erst entscheiden.';
  const cards = [
    dbCard('ARC-0001', 'Login bauen', 'backlog', { waitingOnQuestions: [{ id: 'ARC-F0001', title: 'Speicher?' }, { id: 'ARC-F0002', title: 'Port?' }] }),
  ];
  const openBlocked = (moveDbTicket = vi.fn(async () => cards[0])) => {
    open({ getDbBoardView: vi.fn(async () => dbView(cards)), moveDbTicket });
    return moveDbTicket;
  };

  it('verschiebt beim Ablegen auf „In Arbeit“ nicht und nennt die Fragen ohne CLI-Hinweis', async () => {
    const moveDbTicket = openBlocked();
    await screen.findByText('Login bauen');
    fireEvent.drop(screen.getByRole('region', { name: 'In Arbeit' }), { dataTransfer: { getData: () => 'ARC-0001' } });
    const alert = await screen.findByRole('alert');
    expect(alert.textContent).toBe(BLOCKED);
    expect(alert.textContent).not.toContain('--force');
    expect(moveDbTicket).not.toHaveBeenCalled();
    expect(within(screen.getByRole('region', { name: 'Backlog' })).getByText('Login bauen')).toBeTruthy();
  });

  it('öffnet für „Erledigt“ keinen Abnahme-Dialog, auch nicht über das Statusmenü', async () => {
    const moveDbTicket = openBlocked();
    await screen.findByText('Login bauen');
    fireEvent.drop(screen.getByRole('region', { name: 'Erledigt' }), { dataTransfer: { getData: () => 'ARC-0001' } });
    expect((await screen.findByRole('alert')).textContent).toBe(BLOCKED);
    await userEvent.selectOptions(screen.getByLabelText('Status von ARC-0001 ändern'), 'done');
    expect(screen.queryByRole('dialog')).toBeNull();
    await userEvent.selectOptions(screen.getByLabelText('Status von ARC-0001 ändern'), 'in_progress');
    expect(screen.getByRole('alert').textContent).toBe(BLOCKED);
    expect(moveDbTicket).not.toHaveBeenCalled();
    expect((screen.getByLabelText('Status von ARC-0001 ändern') as HTMLSelectElement).value).toBe('backlog');
  });

  it('erlaubt andere Spalten', async () => {
    const moveDbTicket = openBlocked();
    await screen.findByText('Login bauen');
    await userEvent.selectOptions(screen.getByLabelText('Status von ARC-0001 ändern'), 'planned');
    expect(moveDbTicket).toHaveBeenCalledWith('ARC-0001', { status: 'planned', rev: 1 });
    expect(screen.queryByRole('alert')).toBeNull();
  });
});

describe('DbBoardPage – Abhängigkeitsgraph und Fortschritt je Spec', () => {
  const detailOf = async (id: string) => dbDetail(CARDS.find((c) => c.id === id)!);
  const graph = {
    scope: { kind: 'board' as const, boardId: 1 },
    nodes: [
      { id: 'ARC-0001', title: 'Login bauen', status: 'backlog' as const, column: 'backlog' as const, boardId: 1, boardName: 'Sprint', spec: null, external: false },
      { id: 'ARC-0003', title: 'Wartet hier', status: 'backlog' as const, column: 'waiting' as const, boardId: 1, boardName: 'Sprint', spec: null, external: false },
    ],
    edges: [{ from: 'ARC-0001', to: 'ARC-0003' }],
    criticalPath: ['ARC-0001', 'ARC-0003'],
    cycleEdges: [],
    hiddenWithoutEdges: 2,
    hiddenDone: 0,
  };

  it('öffnet über „Abhängigkeiten“ den Board-Graphen; ein Klick auf einen Knoten öffnet das Ticket und schließt den Dialog', async () => {
    const getDbGraph = vi.fn(async () => graph);
    open({ getDbGraph, getDbTicket: vi.fn(detailOf) });
    await screen.findByText('Login bauen');
    await userEvent.click(screen.getByRole('button', { name: 'Abhängigkeiten' }));
    await screen.findByRole('dialog', { name: 'Abhängigkeiten' });
    expect(getDbGraph).toHaveBeenCalledWith({ kind: 'board', boardId: 1 }, false);
    const node = await waitFor(() => {
      const found = document.querySelector<SVGGElement>(".dep-graph-node[data-id='ARC-0003']");
      expect(found).toBeTruthy();
      return found!;
    });
    await userEvent.click(node);
    expect(await screen.findByRole('dialog', { name: 'Ticket ARC-0003' })).toBeTruthy();
    expect(screen.queryByRole('dialog', { name: 'Abhängigkeiten' })).toBeNull();
  });

  it('zeigt im Specs-Dialog je Spec einen Fortschrittsbalken; ein Klick öffnet den Graphen der Spec', async () => {
    const getDbGraph = vi.fn(async () => graph);
    const getDbSpecProgress = vi.fn(async () => [
      { spec: '001-login', total: 7, done: 3, blocked: 1, waiting: 2, inProgress: 0, open: 1, notNeeded: 0 },
    ]);
    open({ getDbGraph, getDbSpecProgress });
    await userEvent.click(await screen.findByRole('button', { name: 'Specs' }));
    expect(await screen.findByText('3 von 7 erledigt, 1 blockiert, 2 wartet, 0 in Arbeit, 1 offen')).toBeTruthy();
    expect(getDbSpecProgress).toHaveBeenCalledWith(1);
    await userEvent.click(screen.getByRole('button', { name: '001-login' }));
    await screen.findByRole('dialog', { name: 'Abhängigkeiten' });
    expect(getDbGraph).toHaveBeenCalledWith({ kind: 'spec', project: 'ARC', spec: '001-login' }, true);
    expect(screen.getByRole('button', { name: 'Spec 001-login' }).getAttribute('aria-pressed')).toBe('true');
  });
});
