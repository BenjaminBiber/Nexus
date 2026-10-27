// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter, Route, Routes, useLocation } from 'react-router-dom';
import { ApiProvider } from '../src/api/ApiContext.js';
import type { DbSearchHitDto, DbSearchResultDto } from '../src/api/dbTypes.js';
import { SearchDialog } from '../src/components/SearchDialog.js';
import { App } from '../src/App.js';
import { fakeApi } from './helpers/fakeApi.js';

afterEach(cleanup);

const project = { id: 1, key: 'ARC', name: 'Arches' };
const HITS: DbSearchHitDto[] = [
  { kind: 'ticket', id: 'ARC-0007', ticketId: 'ARC-0007', field: 'plan', title: 'Export bauen', project, board: { id: 4, name: 'Sprint' }, status: 'in_progress', snippet: '…nutzt den Speicher für…', matches: [[11, 19]], at: null },
  { kind: 'question', id: 'ARC-F0002', ticketId: null, field: 'title', title: 'Welcher Speicher?', project, board: null, status: 'open', snippet: 'Welcher Speicher?', matches: [[8, 16]], at: null },
  { kind: 'note', id: '12', ticketId: null, field: 'note', title: 'Speicher läuft voll', project, board: { id: 5, name: 'Incidents' }, status: 'open', snippet: 'Speicher läuft voll', matches: [[0, 8]], at: '2026-10-13T10:00:00.000Z' },
];
const result = (q: string, hits = HITS): DbSearchResultDto => ({ query: q, terms: [q], hits, truncated: false });

// Zeigt die aktuelle Adresse samt Navigationszustand
function Where() {
  const location = useLocation();
  return <p data-testid="where">{`${location.pathname} ${JSON.stringify(location.state)}`}</p>;
}

function renderDialog(searchDb = vi.fn(async (q: string) => result(q))) {
  const onClose = vi.fn();
  render(
    <ApiProvider api={fakeApi({ searchDb })}>
      <MemoryRouter initialEntries={['/p/p1/sessions']}>
        <SearchDialog pid="p1" onClose={onClose} />
        <Where />
      </MemoryRouter>
    </ApiProvider>,
  );
  return { searchDb, onClose };
}

describe('SearchDialog', () => {
  it('zeigt Treffer nach Art gruppiert mit Projekt, Board, Status und hervorgehobener Fundstelle', async () => {
    const { searchDb } = renderDialog();
    await userEvent.type(screen.getByRole('combobox', { name: 'Suchbegriff' }), 'speicher');
    const list = await screen.findByRole('listbox', { name: 'Treffer' });
    expect(searchDb).toHaveBeenLastCalledWith('speicher');
    expect(within(list).getAllByRole('group').map((g) => g.getAttribute('aria-label'))).toEqual(['Tickets', 'Fragen', 'Notizen']);
    const [ticket, question, note] = within(list).getAllByRole('option');
    expect(ticket.textContent).toContain('ARC · Sprint · In Arbeit · Plan');
    expect(ticket.querySelector('mark')?.textContent).toBe('Speicher');
    expect(question.textContent).toContain('ARC · offen · Titel');
    expect(note.textContent).toContain('Notiz 12');
    expect(ticket.getAttribute('aria-selected')).toBe('true');
  });

  it('wählt mit ↑/↓ und öffnet mit Enter das Ticket', async () => {
    const { onClose } = renderDialog();
    const input = screen.getByRole('combobox', { name: 'Suchbegriff' });
    await userEvent.type(input, 'speicher');
    await screen.findByRole('listbox', { name: 'Treffer' });
    await userEvent.keyboard('{ArrowDown}{ArrowUp}{Enter}');
    expect(onClose).toHaveBeenCalled();
    expect(screen.getByTestId('where').textContent).toBe('/p/p1/board/t/ARC-0007 null');
  });

  it('öffnet Fragen auf ihrer Seite und offene Notizen im Notizen-Dialog ihres Boards', async () => {
    renderDialog();
    await userEvent.type(screen.getByRole('combobox', { name: 'Suchbegriff' }), 'speicher');
    await userEvent.click((await screen.findAllByRole('option'))[1]);
    expect(screen.getByTestId('where').textContent).toBe('/p/p1/questions/ARC-F0002 null');
    await userEvent.click(screen.getAllByRole('option')[2]);
    expect(screen.getByTestId('where').textContent).toBe('/p/p1/board {"notesBoard":5}');
  });

  it('meldet „Keine Treffer“ und lässt ältere Antworten keine neueren überschreiben', async () => {
    let resolveFirst: (r: DbSearchResultDto) => void = () => undefined;
    const searchDb = vi.fn((q: string) => (q === 'sp'
      ? new Promise<DbSearchResultDto>((r) => { resolveFirst = r; })
      : Promise.resolve(result(q, []))));
    renderDialog(searchDb);
    const input = screen.getByRole('combobox', { name: 'Suchbegriff' });
    await userEvent.type(input, 'sp');
    await waitFor(() => expect(searchDb).toHaveBeenCalledWith('sp'));
    await userEvent.type(input, 'x');
    expect(await screen.findByText('Keine Treffer.')).toBeTruthy();
    resolveFirst(result('sp'));
    await new Promise((r) => setTimeout(r, 20));
    expect(screen.queryByRole('option')).toBeNull();
    expect(screen.getByText('Keine Treffer.')).toBeTruthy();
  });

  it('schließt mit Escape', async () => {
    const { onClose } = renderDialog();
    await userEvent.keyboard('{Escape}');
    expect(onClose).toHaveBeenCalled();
  });
});

describe('Suche im App-Rahmen', () => {
  function openApp() {
    render(
      <ApiProvider api={fakeApi()}>
        <MemoryRouter initialEntries={['/p/p1/sessions']}>
          <App />
        </MemoryRouter>
      </ApiProvider>,
    );
  }

  it('öffnet den Dialog über „Suchen“ in der Topbar und schließt ihn mit Escape', async () => {
    openApp();
    await userEvent.click(await screen.findByRole('button', { name: 'Suchen' }));
    expect(screen.getByRole('dialog', { name: 'Suche über alle Boards' })).toBeTruthy();
    expect(document.activeElement).toBe(screen.getByRole('combobox', { name: 'Suchbegriff' }));
    await userEvent.keyboard('{Escape}');
    expect(screen.queryByRole('dialog', { name: 'Suche über alle Boards' })).toBeNull();
  });

  it('öffnet eine Notiz im Notizen-Dialog ihres Boards, auch wenn ein anderes Board aktiv ist', async () => {
    const getDbNotes = vi.fn(async () => []);
    render(
      <ApiProvider api={fakeApi({ searchDb: async (q: string) => result(q), getDbNotes })}>
        <MemoryRouter initialEntries={['/p/p1/sessions']}>
          <App />
        </MemoryRouter>
      </ApiProvider>,
    );
    await userEvent.click(await screen.findByRole('button', { name: 'Suchen' }));
    await userEvent.type(screen.getByRole('combobox', { name: 'Suchbegriff' }), 'speicher');
    const list = await screen.findByRole('listbox', { name: 'Treffer' });
    await userEvent.click(within(list).getAllByRole('option')[2]);
    expect(await screen.findByRole('dialog', { name: 'Notizen' })).toBeTruthy();
    await waitFor(() => expect(getDbNotes).toHaveBeenCalledWith(5, expect.anything()));
  });

  it('öffnet den Dialog mit Strg+K und Cmd+K', async () => {
    openApp();
    await screen.findByRole('button', { name: 'Suchen' });
    await userEvent.keyboard('{Control>}k{/Control}');
    expect(screen.getByRole('dialog', { name: 'Suche über alle Boards' })).toBeTruthy();
    await userEvent.keyboard('{Escape}');
    await userEvent.keyboard('{Meta>}K{/Meta}');
    expect(screen.getByRole('dialog', { name: 'Suche über alle Boards' })).toBeTruthy();
  });
});
