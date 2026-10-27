// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter, Route, Routes, useLocation } from 'react-router-dom';
import { ApiProvider } from '../src/api/ApiContext.js';
import { ApiError } from '../src/api/ApiError.js';
import type { InboxListDto } from '../src/api/inboxTypes.js';
import { LiveProvider } from '../src/live/LiveProvider.js';
import { InboxPage } from '../src/pages/InboxPage.js';
import { FakeEventSource } from './helpers/FakeEventSource.js';
import { fakeApi, inboxItem, inboxList } from './helpers/fakeApi.js';

beforeEach(() => FakeEventSource.reset());
afterEach(cleanup);

const LIST = inboxList([
  inboxItem('ticket:ARC-0002', 'merge', 'Konflikt', { detail: 'Merge: a.ts' }),
  inboxItem('question:ARC-F0001', 'question', 'Welcher Speicher?', { waitingTickets: 2, detail: 'Planung' }),
  inboxItem('ticket:ARC-0001', 'accept', 'Fertig', { detail: 'Human Interaction: ansehen' }),
  inboxItem('note:7', 'note', 'Cache prüfen', { boardId: 2, boardName: 'Nächster' }),
]);

// Zeigt die aktuelle Adresse samt Suche (für Filter-Prüfungen)
function Where() {
  const location = useLocation();
  return <output data-testid="where">{`${location.pathname}${location.search}`}</output>;
}

function open(overrides: Parameters<typeof fakeApi>[0] = {}, path = '/p/p1/inbox') {
  const api = fakeApi({ getInbox: vi.fn(async () => LIST), openEvents: vi.fn(() => new FakeEventSource()), ...overrides });
  render(
    <ApiProvider api={api}>
      <LiveProvider pid="p1">
        <MemoryRouter initialEntries={[path]}>
          <Routes>
            <Route path="/p/:pid/inbox" element={<><InboxPage /><Where /></>} />
            <Route path="*" element={<Where />} />
          </Routes>
        </MemoryRouter>
      </LiveProvider>
    </ApiProvider>,
  );
  return api;
}

const rows = () => within(screen.getByRole('list', { name: 'Aufgaben' })).getAllByRole('listitem');

describe('Seite „Meine Aufgaben“', () => {
  it('zeigt die Einträge in der Reihenfolge des Servers mit Art, Board, Detail und Link', async () => {
    open();
    await screen.findByRole('list', { name: 'Aufgaben' });
    expect(rows().map((r) => r.querySelector('.inbox-badge')?.textContent)).toEqual(['Merge gescheitert', 'Frage', 'Abnahme', 'Notiz']);
    expect(screen.getByRole('link', { name: /Konflikt/ }).getAttribute('href')).toBe('/p/p1/board/t/ARC-0002');
    expect(screen.getByRole('link', { name: /Welcher Speicher/ }).getAttribute('href')).toBe('/p/p1/questions/ARC-F0001');
    expect(screen.getByRole('link', { name: /Cache prüfen/ }).getAttribute('href')).toBe('/p/p1/board');
    expect(rows()[1].textContent).toContain('2 Tickets warten');
    expect(rows()[3].textContent).toContain('ARC · Nächster');
    expect(screen.getByText('Human Interaction: ansehen')).toBeTruthy();
  });

  it('nimmt ab, mergt erneut und macht aus einer Notiz ein Ticket, danach lädt die Liste neu', async () => {
    const acceptDispatchTicket = vi.fn(async () => ({ result: { text: 'gemergt' } }));
    const retryDispatchMerge = vi.fn(async () => ({ result: { text: 'gemergt' } }));
    const takeDbNote = vi.fn(async () => ({}));
    const api = open({ acceptDispatchTicket, retryDispatchMerge, takeDbNote });
    await screen.findByRole('list', { name: 'Aufgaben' });
    await userEvent.click(screen.getByRole('button', { name: 'Abnehmen' }));
    await waitFor(() => expect(acceptDispatchTicket).toHaveBeenCalledWith('ARC-0001', 1));
    await userEvent.click(screen.getByRole('button', { name: 'Erneut mergen' }));
    await waitFor(() => expect(retryDispatchMerge).toHaveBeenCalledWith('ARC-0002', 1));
    await userEvent.click(screen.getByRole('button', { name: 'Ticket daraus' }));
    await waitFor(() => expect(takeDbNote).toHaveBeenCalledWith(7));
    await waitFor(() => expect(api.getInbox).toHaveBeenCalledTimes(4));
  });

  it('setzt den Board-Filter in die Adresse und lädt nur dieses Board', async () => {
    const api = open();
    const select = (await screen.findByLabelText('Board')) as HTMLSelectElement;
    await waitFor(() => expect(Array.from(select.options).map((o) => o.text)).toEqual(['Alle eigenen Boards', 'ARC · Sprint', 'ARC · Nächster', 'OTH · Fremd']));
    await userEvent.selectOptions(select, '2');
    await waitFor(() => expect(screen.getByTestId('where').textContent).toBe('/p/p1/inbox?board=2'));
    await waitFor(() => expect(api.getInbox).toHaveBeenLastCalledWith(2));
  });

  it('filtert nach Art über ?kind= und bietet dann kein Umsortieren an', async () => {
    open({}, '/p/p1/inbox?kind=note,accept');
    await screen.findByRole('list', { name: 'Aufgaben' });
    expect(rows().map((r) => r.dataset.key)).toEqual(['ticket:ARC-0001', 'note:7']);
    expect(rows().every((r) => r.getAttribute('draggable') === null)).toBe(true);
    expect(screen.getByText('Umsortieren geht nur ohne Filter.')).toBeTruthy();
    await userEvent.click(screen.getByRole('button', { name: /^Notiz/ }));
    expect(screen.getByTestId('where').textContent).toBe('/p/p1/inbox?kind=accept');
  });

  it('zeigt den Leerzustand, Fehler und den Hinweis auf eine fehlende Board-Datenbank', async () => {
    open({ getInbox: async () => inboxList([]) });
    expect(await screen.findByRole('heading', { name: 'Nichts wartet auf dich' })).toBeTruthy();
    cleanup();
    open({ getInbox: async () => { throw new Error('kaputt'); } });
    expect(await screen.findByRole('heading', { name: 'Aufgaben konnten nicht geladen werden' })).toBeTruthy();
    cleanup();
    open({ getInbox: async () => { throw new ApiError('board-config-missing', 'Keine Board-Datenbank eingerichtet.', 503); } });
    expect(await screen.findByRole('heading', { name: 'Board-Datenbank nicht verfügbar' })).toBeTruthy();
  });

  it('setzt die Reihenfolge zurück und lädt neu', async () => {
    const resetInboxOrder = vi.fn(async () => ({ keys: [] }));
    const api = open({ resetInboxOrder });
    await screen.findByRole('list', { name: 'Aufgaben' });
    await userEvent.click(screen.getByRole('button', { name: 'Reihenfolge zurücksetzen' }));
    await waitFor(() => expect(resetInboxOrder).toHaveBeenCalled());
    await waitFor(() => expect(api.getInbox).toHaveBeenCalledTimes(2));
  });
});

describe('Umsortieren in „Meine Aufgaben“', () => {
  it('verschiebt einen Eintrag per Ziehen nach unten und speichert alle Schlüssel', async () => {
    const saveInboxOrder = vi.fn(async (keys: string[]) => ({ keys }));
    open({ saveInboxOrder });
    await screen.findByRole('list', { name: 'Aufgaben' });
    expect(rows().every((r) => r.getAttribute('draggable') === 'true')).toBe(true);
    expect(screen.queryByRole('button', { name: /nach (oben|unten)/ })).toBeNull();
    fireEvent.dragStart(rows()[0]);
    fireEvent.drop(rows()[1]);
    expect(rows().map((r) => r.dataset.key)).toEqual(['question:ARC-F0001', 'ticket:ARC-0002', 'ticket:ARC-0001', 'note:7']);
    await waitFor(() => expect(saveInboxOrder).toHaveBeenCalledWith(['question:ARC-F0001', 'ticket:ARC-0002', 'ticket:ARC-0001', 'note:7']));
  });

  it('stellt bei einem Fehler die alte Reihenfolge wieder her', async () => {
    const saveInboxOrder = vi.fn(async () => { throw new Error('Platte voll'); });
    open({ saveInboxOrder });
    await screen.findByRole('list', { name: 'Aufgaben' });
    fireEvent.dragStart(rows()[2]);
    fireEvent.drop(rows()[1]);
    expect(await screen.findByText('Reihenfolge konnte nicht gespeichert werden: Platte voll')).toBeTruthy();
    expect(rows().map((r) => r.dataset.key)).toEqual(['ticket:ARC-0002', 'question:ARC-F0001', 'ticket:ARC-0001', 'note:7']);
  });

  it('verschiebt per Ziehen und Ablegen', async () => {
    const saveInboxOrder = vi.fn(async (keys: string[]) => ({ keys }));
    open({ saveInboxOrder });
    await screen.findByRole('list', { name: 'Aufgaben' });
    fireEvent.dragStart(rows()[3]);
    fireEvent.dragOver(rows()[0]);
    fireEvent.drop(rows()[0]);
    await waitFor(() => expect(saveInboxOrder).toHaveBeenCalledWith(['note:7', 'ticket:ARC-0002', 'question:ARC-F0001', 'ticket:ARC-0001']));
  });

  it('liest beim Laden die Liste so, wie der Server sie liefert', async () => {
    const reordered: InboxListDto = { ...LIST, items: [...LIST.items].reverse() };
    open({ getInbox: async () => reordered });
    await screen.findByRole('list', { name: 'Aufgaben' });
    expect(rows().map((r) => r.dataset.key)).toEqual(['note:7', 'ticket:ARC-0001', 'question:ARC-F0001', 'ticket:ARC-0002']);
  });
});
