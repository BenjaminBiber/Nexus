// @vitest-environment jsdom
import { StrictMode } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { ApiProvider } from '../src/api/ApiContext.js';
import { ApiError } from '../src/api/ApiError.js';
import type { DbNewTicketInput, DbNoteDto } from '../src/api/dbTypes.js';
import { DbAcceptanceDialog } from '../src/components/db/DbAcceptanceDialog.js';
import { DbModal } from '../src/components/db/DbModal.js';
import { DbNewTicketDialog } from '../src/components/db/DbNewTicketDialog.js';
import { DbNotesDialog } from '../src/components/db/DbNotesDialog.js';
import { DbBoardPage } from '../src/pages/DbBoardPage.js';
import { dbCard, dbDetail, dbView, fakeApi } from './helpers/fakeApi.js';
import { renderWithApp } from './helpers/renderWithApp.js';

afterEach(cleanup);

// Ältere Notiz ohne Titel: die erste Zeile des Inhalts dient als Titel
const NOTE: DbNoteDto = {
  id: 1, boardId: 1, authorId: 'u1', authorName: 'Florian', title: '', body: 'Cache prüfen\nDetails', source: 'ARC-0001', labels: ['perf'],
  seenCount: 1, seenIn: [], state: 'open', ticketId: null, reason: null, createdAt: '2026-10-13T10:00:00.000Z',
};

describe('DbNewTicketDialog', () => {
  it('legt ein Ticket mit allen Feldern an', async () => {
    const createDbTicket = vi.fn(async (_board: number, input: DbNewTicketInput) => dbCard('ARC-0005', input.title));
    const onCreated = vi.fn();
    render(<ApiProvider api={fakeApi({ createDbTicket })}><DbNewTicketDialog boardId={1} onClose={vi.fn()} onCreated={onCreated} /></ApiProvider>);
    await userEvent.type(screen.getByLabelText('Titel'), 'Export');
    await userEvent.type(screen.getByLabelText('Beschreibung'), 'CSV');
    await userEvent.selectOptions(screen.getByLabelText('Status'), 'planned');
    await userEvent.selectOptions(screen.getByLabelText('Priorität'), 'high');
    await userEvent.type(screen.getByLabelText('Labels'), 'api, csv');
    await userEvent.click(screen.getByLabelText('Braucht Planung'));
    await userEvent.click(screen.getByRole('button', { name: 'Anlegen' }));
    expect(createDbTicket).toHaveBeenCalledWith(1, { title: 'Export', description: 'CSV', status: 'planned', priority: 'high', labels: ['api', 'csv'], needsPlanning: false });
    await waitFor(() => expect(onCreated).toHaveBeenCalledWith(expect.objectContaining({ id: 'ARC-0005' })));
  });

  it('verlangt einen Titel und zeigt Fehler des Servers', async () => {
    const createDbTicket = vi.fn(async () => { throw new ApiError('dependency-unknown', 'Voraussetzung ARC-0042 gibt es nicht.', 422); });
    render(<ApiProvider api={fakeApi({ createDbTicket })}><DbNewTicketDialog boardId={1} onClose={vi.fn()} onCreated={vi.fn()} /></ApiProvider>);
    await userEvent.click(screen.getByRole('button', { name: 'Anlegen' }));
    expect((await screen.findByRole('alert')).textContent).toContain('Titel');
    expect(createDbTicket).not.toHaveBeenCalled();
    await userEvent.type(screen.getByLabelText('Titel'), 'A');
    await userEvent.click(screen.getByRole('button', { name: 'Anlegen' }));
    expect((await screen.findByRole('alert')).textContent).toContain('ARC-0042');
  });

  it('fragt beim Abbrechen nach, wenn schon etwas eingetragen ist', async () => {
    const onClose = vi.fn();
    render(<ApiProvider api={fakeApi()}><DbNewTicketDialog boardId={1} onClose={onClose} onCreated={vi.fn()} /></ApiProvider>);
    await userEvent.type(screen.getByLabelText('Titel'), 'Halb');
    await userEvent.click(screen.getByRole('button', { name: 'Abbrechen' }));
    expect(screen.getByRole('alertdialog', { name: 'Eingaben verwerfen' })).toBeTruthy();
    expect(onClose).not.toHaveBeenCalled();
    await userEvent.click(screen.getByRole('button', { name: 'Verwerfen' }));
    expect(onClose).toHaveBeenCalled();
  });
});

describe('DbNotesDialog', () => {
  it('lädt nach Filter, legt Notizen an und macht ein Ticket daraus', async () => {
    const getDbNotes = vi.fn(async (_board: number, filter: string) => (filter === 'open' ? [NOTE] : []));
    const addDbNote = vi.fn(async () => NOTE);
    const takeDbNote = vi.fn(async () => ({ note: { ...NOTE, state: 'taken', ticketId: 'ARC-0007' }, ticket: dbCard('ARC-0007', 'Cache prüfen') }));
    const onChanged = vi.fn();
    render(<ApiProvider api={fakeApi({ getDbNotes, addDbNote, takeDbNote })}><DbNotesDialog boardId={1} onClose={vi.fn()} onChanged={onChanged} onOpenTicket={vi.fn()} /></ApiProvider>);
    expect(await screen.findByText(/Cache prüfen/)).toBeTruthy();
    await userEvent.click(screen.getByRole('button', { name: 'Übernommen' }));
    expect(getDbNotes).toHaveBeenLastCalledWith(1, 'taken');
    expect(await screen.findByText('Nichts mit diesem Stand.')).toBeTruthy();
    await userEvent.click(screen.getByRole('button', { name: 'Offen' }));
    await userEvent.type(screen.getByLabelText('Neue Notiz'), 'Logs aufräumen');
    await userEvent.type(screen.getByLabelText('Details'), 'Server-Log wächst ohne Rotation');
    await userEvent.type(screen.getByLabelText('Labels der Notiz'), 'ops');
    await userEvent.click(screen.getByRole('button', { name: 'Notiz anlegen' }));
    expect(addDbNote).toHaveBeenCalledWith(1, { title: 'Logs aufräumen', body: 'Server-Log wächst ohne Rotation', labels: ['ops'], force: false });
    await userEvent.click(await screen.findByRole('button', { name: 'Ticket daraus machen' }));
    expect((screen.getByLabelText('Titel des Tickets') as HTMLInputElement).value).toBe('Cache prüfen');
    await userEvent.click(screen.getByRole('button', { name: 'Ticket anlegen' }));
    expect(takeDbNote).toHaveBeenCalledWith(1, { title: 'Cache prüfen' });
    await waitFor(() => expect(onChanged).toHaveBeenCalledTimes(2));
  });

  it('verwirft nur mit Grund', async () => {
    const dropDbNote = vi.fn(async () => ({ ...NOTE, state: 'dropped', reason: 'doppelt' }));
    render(<ApiProvider api={fakeApi({ getDbNotes: async () => [NOTE], dropDbNote })}><DbNotesDialog boardId={1} onClose={vi.fn()} onChanged={vi.fn()} onOpenTicket={vi.fn()} /></ApiProvider>);
    await userEvent.click(await screen.findByRole('button', { name: 'Verwerfen' }));
    await userEvent.click(screen.getByRole('button', { name: 'Verwerfen bestätigen' }));
    expect((await screen.findByRole('alert')).textContent).toContain('Ohne Grund');
    expect(dropDbNote).not.toHaveBeenCalled();
    await userEvent.type(screen.getByLabelText('Grund'), 'doppelt');
    await userEvent.click(screen.getByRole('button', { name: 'Verwerfen bestätigen' }));
    expect(dropDbNote).toHaveBeenCalledWith(1, 'doppelt');
  });

  it('öffnet das übernommene Ticket und die Herkunft', async () => {
    const onOpenTicket = vi.fn();
    const taken = { ...NOTE, state: 'taken' as const, ticketId: 'ARC-0007' };
    render(<ApiProvider api={fakeApi({ getDbNotes: async () => [taken] })}><DbNotesDialog boardId={1} onClose={vi.fn()} onChanged={vi.fn()} onOpenTicket={onOpenTicket} /></ApiProvider>);
    await userEvent.click(await screen.findByRole('button', { name: 'ARC-0007' }));
    expect(onOpenTicket).toHaveBeenCalledWith('ARC-0007');
    await userEvent.click(screen.getByRole('button', { name: 'aus ARC-0001' }));
    expect(onOpenTicket).toHaveBeenLastCalledWith('ARC-0001');
  });
});

describe('Dialoge auf der Board-Seite', () => {
  it('öffnet „Neues Ticket“, meldet das angelegte Ticket und lädt neu', async () => {
    const getDbBoardView = vi.fn(async () => dbView([dbCard('ARC-0001', 'Login bauen')]));
    const createDbTicket = vi.fn(async () => dbCard('ARC-0005', 'Export'));
    renderWithApp(<DbBoardPage />, { api: fakeApi({ getDbBoardView, createDbTicket }), path: '/p/p1/board', route: '/p/:pid/board/*' });
    await screen.findByText('Login bauen');
    await userEvent.click(screen.getByRole('button', { name: 'Ticket hinzufügen' }));
    await userEvent.click(screen.getByRole('menuitem', { name: 'Neues Ticket' }));
    await userEvent.type(screen.getByLabelText('Titel'), 'Export');
    await userEvent.click(screen.getByRole('button', { name: 'Anlegen' }));
    expect((await screen.findByRole('status')).textContent).toBe('ARC-0005 angelegt.');
    expect(screen.queryByRole('dialog', { name: 'Neues Ticket' })).toBeNull();
    await waitFor(() => expect(getDbBoardView).toHaveBeenCalledTimes(2));
  });

  it('öffnet die Notizen und aus einer Notiz das Ticket-Panel', async () => {
    const taken = { ...NOTE, state: 'taken' as const, ticketId: 'ARC-0001' };
    renderWithApp(<DbBoardPage />, {
      api: fakeApi({ getDbBoardView: async () => dbView([dbCard('ARC-0001', 'Login bauen')]), getDbNotes: async () => [taken], getDbTicket: async () => dbDetail(dbCard('ARC-0001', 'Login bauen')) }),
      path: '/p/p1/board', route: '/p/:pid/board/*',
    });
    await userEvent.click(await screen.findByRole('button', { name: 'Notizen (1)' }));
    await userEvent.click(await screen.findByRole('button', { name: 'ARC-0001' }));
    expect(screen.queryByRole('dialog', { name: 'Notizen' })).toBeNull();
    expect(await screen.findByRole('dialog', { name: 'Ticket ARC-0001' })).toBeTruthy();
  });
});

describe('Dialoge: Abnahme, Doppelklick, Esc und Fokus', () => {
  it('verlangt bei „Erledigt“ eine Abnahme und schickt sie mit', async () => {
    const createDbTicket = vi.fn(async (_board: number, input: DbNewTicketInput) => dbCard('ARC-0006', input.title, 'done'));
    render(<ApiProvider api={fakeApi({ createDbTicket })}><DbNewTicketDialog boardId={1} onClose={vi.fn()} onCreated={vi.fn()} /></ApiProvider>);
    expect(screen.queryByLabelText('Abnahme')).toBeNull();
    await userEvent.type(screen.getByLabelText('Titel'), 'Alt');
    await userEvent.selectOptions(screen.getByLabelText('Status'), 'done');
    await userEvent.click(screen.getByRole('button', { name: 'Anlegen' }));
    expect((await screen.findByRole('alert')).textContent).toContain('Abnahme');
    expect(createDbTicket).not.toHaveBeenCalled();
    await userEvent.type(screen.getByLabelText('Abnahme'), 'läuft produktiv');
    await userEvent.click(screen.getByRole('button', { name: 'Anlegen' }));
    expect(createDbTicket).toHaveBeenCalledWith(1, expect.objectContaining({ status: 'done', acceptance: 'läuft produktiv' }));
  });

  it('sperrt die Knöpfe während des Anlegens (kein Doppelklick)', async () => {
    let resolve: (t: ReturnType<typeof dbCard>) => void = () => {};
    const createDbTicket = vi.fn(() => new Promise((r) => { resolve = r; }));
    render(<ApiProvider api={fakeApi({ createDbTicket })}><DbNewTicketDialog boardId={1} onClose={vi.fn()} onCreated={vi.fn()} /></ApiProvider>);
    await userEvent.type(screen.getByLabelText('Titel'), 'A');
    await userEvent.click(screen.getByRole('button', { name: 'Anlegen' }));
    expect((screen.getByRole('button', { name: 'Anlegen' }) as HTMLButtonElement).disabled).toBe(true);
    expect((screen.getByRole('button', { name: 'Abbrechen' }) as HTMLButtonElement).disabled).toBe(true);
    await userEvent.click(screen.getByRole('button', { name: 'Anlegen' }));
    expect(createDbTicket).toHaveBeenCalledTimes(1);
    resolve(dbCard('ARC-0009', 'A'));
  });

  it('Esc schließt den leeren Dialog, mit Eingaben fragt er nach', async () => {
    const onClose = vi.fn();
    render(<ApiProvider api={fakeApi()}><DbNewTicketDialog boardId={1} onClose={onClose} onCreated={vi.fn()} /></ApiProvider>);
    await userEvent.click(screen.getByLabelText('Titel'));
    await userEvent.keyboard('{Escape}');
    expect(onClose).toHaveBeenCalledTimes(1);
    await userEvent.type(screen.getByLabelText('Titel'), 'x');
    await userEvent.keyboard('{Escape}');
    expect(screen.getByRole('alertdialog', { name: 'Eingaben verwerfen' })).toBeTruthy();
    expect(onClose).toHaveBeenCalledTimes(1);
    await userEvent.click(screen.getByRole('button', { name: 'Weiter bearbeiten' }));
    expect(screen.queryByRole('alertdialog')).toBeNull();
  });

  it('Notizen: ungespeicherter Entwurf fragt vor dem Schließen nach', async () => {
    const onClose = vi.fn();
    render(<ApiProvider api={fakeApi()}><DbNotesDialog boardId={1} onClose={onClose} onChanged={vi.fn()} onOpenTicket={vi.fn()} /></ApiProvider>);
    await userEvent.type(screen.getByLabelText('Neue Notiz'), 'halb');
    await userEvent.click(screen.getByRole('button', { name: 'Notizen schließen' }));
    expect(onClose).not.toHaveBeenCalled();
    await userEvent.click(screen.getByRole('button', { name: 'Verwerfen' }));
    expect(onClose).toHaveBeenCalled();
  });

  it('Notizen: Klick auf den Hintergrund schließt, mit Entwurf erst nach Rückfrage; Klick im Dialog nicht', async () => {
    const onClose = vi.fn();
    render(<ApiProvider api={fakeApi()}><DbNotesDialog boardId={1} onClose={onClose} onChanged={vi.fn()} onOpenTicket={vi.fn()} /></ApiProvider>);
    const dialog = screen.getByRole('dialog', { name: 'Notizen' });
    await userEvent.click(dialog);
    expect(onClose).not.toHaveBeenCalled();
    await userEvent.type(screen.getByLabelText('Neue Notiz'), 'halb');
    await userEvent.click(dialog.parentElement!);
    expect(onClose).not.toHaveBeenCalled();
    await userEvent.click(screen.getByRole('button', { name: 'Verwerfen' }));
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('Notizen: Klick auf den leeren Hintergrund schließt sofort', async () => {
    const onClose = vi.fn();
    render(<ApiProvider api={fakeApi()}><DbNotesDialog boardId={1} onClose={onClose} onChanged={vi.fn()} onOpenTicket={vi.fn()} /></ApiProvider>);
    await userEvent.click(screen.getByRole('dialog', { name: 'Notizen' }).parentElement!);
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('Esc schließt die Notizen, Fokus zurück auf den Knopf, Zähler folgt der Übernahme', async () => {
    let open = [NOTE];
    const takeDbNote = vi.fn(async () => {
      open = [];
      return { note: { ...NOTE, state: 'taken', ticketId: 'ARC-0007' }, ticket: dbCard('ARC-0007', 'Cache prüfen') };
    });
    renderWithApp(<DbBoardPage />, {
      api: fakeApi({ getDbBoardView: async () => dbView([dbCard('ARC-0001', 'Login bauen')]), getDbNotes: async () => open, takeDbNote }),
      path: '/p/p1/board', route: '/p/:pid/board/*',
    });
    const trigger = await screen.findByRole('button', { name: 'Notizen (1)' });
    await userEvent.click(trigger);
    await userEvent.click(await screen.findByRole('button', { name: 'Ticket daraus machen' }));
    await userEvent.click(screen.getByRole('button', { name: 'Ticket anlegen' }));
    expect(await screen.findByRole('button', { name: 'Notizen' })).toBeTruthy();
    await userEvent.keyboard('{Escape}');
    expect(screen.queryByRole('dialog', { name: 'Notizen' })).toBeNull();
    expect(document.activeElement).toBe(screen.getByRole('button', { name: 'Notizen' }));
  });

  it('„Neues Ticket“: Abbrechen gibt den Fokus an den Knopf zurück', async () => {
    renderWithApp(<DbBoardPage />, { api: fakeApi({ getDbBoardView: async () => dbView([dbCard('ARC-0001', 'Login bauen')]) }), path: '/p/p1/board', route: '/p/:pid/board/*' });
    await screen.findByText('Login bauen');
    const trigger = screen.getByRole('button', { name: 'Ticket hinzufügen' });
    await userEvent.click(trigger);
    await userEvent.click(screen.getByRole('menuitem', { name: 'Neues Ticket' }));
    expect(screen.getByRole('dialog', { name: 'Neues Ticket' })).toBeTruthy();
    await userEvent.click(screen.getByRole('button', { name: 'Abbrechen' }));
    expect(screen.queryByRole('dialog', { name: 'Neues Ticket' })).toBeNull();
    expect(document.activeElement).toBe(trigger);
  });
});

describe('Dialog-Hülle: Fokusfalle, Esc und StrictMode', () => {
  it('hält Tab und Umschalt+Tab im Dialog', async () => {
    render(
      <ApiProvider api={fakeApi()}>
        <button type="button">davor</button>
        <DbNewTicketDialog boardId={1} onClose={vi.fn()} onCreated={vi.fn()} />
        <button type="button">danach</button>
      </ApiProvider>,
    );
    const title = screen.getByLabelText('Titel');
    expect(document.activeElement).toBe(title);
    const cancel = screen.getByRole('button', { name: 'Abbrechen' });
    cancel.focus();
    await userEvent.tab();
    expect(document.activeElement).toBe(title);
    await userEvent.tab({ shift: true });
    expect(document.activeElement).toBe(cancel);
  });

  it('Esc greift auch während und nach einem gescheiterten Senden', async () => {
    let fail: (error: Error) => void = () => undefined;
    const createDbTicket = vi.fn(() => new Promise((_resolve, reject) => { fail = reject; }));
    const onClose = vi.fn();
    render(<ApiProvider api={fakeApi({ createDbTicket })}><DbNewTicketDialog boardId={1} onClose={onClose} onCreated={vi.fn()} /></ApiProvider>);
    const dialog = screen.getByRole('dialog', { name: 'Neues Ticket' });
    await userEvent.type(screen.getByLabelText('Titel'), 'A');
    await userEvent.click(screen.getByRole('button', { name: 'Anlegen' }));
    // Der gedrückte Knopf ist gesperrt; der Fokus bleibt auf etwas Bedienbarem im Dialog
    const during = document.activeElement as HTMLButtonElement;
    expect(dialog.contains(during)).toBe(true);
    expect(during.disabled).not.toBe(true);
    fail(new ApiError('conflict', 'Geht gerade nicht.', 409));
    expect((await screen.findByRole('alert')).textContent).toBe('Geht gerade nicht.');
    expect(dialog.contains(document.activeElement)).toBe(true);
    await userEvent.keyboard('{Escape}');
    expect(screen.getByRole('alertdialog', { name: 'Eingaben verwerfen' })).toBeTruthy();
    await userEvent.keyboard('{Escape}');
    expect(screen.queryByRole('alertdialog')).toBeNull();
    expect(onClose).not.toHaveBeenCalled();
  });

  it('behält unter StrictMode den Fokus im Dialog und gibt ihn erst beim Schließen zurück', () => {
    const trigger = document.createElement('button');
    document.body.appendChild(trigger);
    trigger.focus();
    const { unmount } = render(
      <StrictMode>
        <ApiProvider api={fakeApi()}>
          <DbNewTicketDialog boardId={1} onClose={vi.fn()} onCreated={vi.fn()} />
        </ApiProvider>
      </StrictMode>,
    );
    expect(document.activeElement).toBe(screen.getByLabelText('Titel'));
    unmount();
    expect(document.activeElement).toBe(trigger);
    trigger.remove();
  });

  it('StrictMode: Notizen und Abnahme stehlen den Fokus nicht', () => {
    const trigger = document.createElement('button');
    document.body.appendChild(trigger);
    trigger.focus();
    const { unmount } = render(
      <StrictMode>
        <ApiProvider api={fakeApi()}>
          <DbNotesDialog boardId={1} onClose={vi.fn()} onChanged={vi.fn()} onOpenTicket={vi.fn()} />
        </ApiProvider>
      </StrictMode>,
    );
    expect(document.activeElement).toBe(screen.getByRole('button', { name: 'Notizen schließen' }));
    unmount();
    expect(document.activeElement).toBe(trigger);
    render(<StrictMode><DbAcceptanceDialog ticketId="ARC-0001" onConfirm={vi.fn()} onCancel={vi.fn()} /></StrictMode>);
    expect(document.activeElement).toBe(screen.getByRole('textbox', { name: 'Woran sieht man, dass es fertig ist?' }));
    trigger.remove();
  });
});

describe('DbNotesDialog – Filter, Esc und Senden', () => {
  const DROPPED: DbNoteDto = { ...NOTE, id: 2, body: 'Alter Plan', state: 'dropped', reason: 'doppelt', source: null };

  it('zeigt verworfene Notizen mit Grund und unter „Alle“ alles', async () => {
    const getDbNotes = vi.fn(async (_board: number, filter: string) => (filter === 'open' ? [NOTE] : filter === 'dropped' ? [DROPPED] : [NOTE, DROPPED]));
    render(<ApiProvider api={fakeApi({ getDbNotes })}><DbNotesDialog boardId={1} onClose={vi.fn()} onChanged={vi.fn()} onOpenTicket={vi.fn()} /></ApiProvider>);
    await screen.findByText(/Cache prüfen/);
    await userEvent.click(screen.getByRole('button', { name: 'Verworfen' }));
    expect(getDbNotes).toHaveBeenLastCalledWith(1, 'dropped');
    expect(await screen.findByText('verworfen: doppelt')).toBeTruthy();
    expect(screen.queryByText(/Cache prüfen/)).toBeNull();
    expect(screen.getByRole('button', { name: 'Verworfen' }).getAttribute('aria-pressed')).toBe('true');
    await userEvent.click(screen.getByRole('button', { name: 'Alle' }));
    expect(getDbNotes).toHaveBeenLastCalledWith(1, 'all');
    expect(await screen.findByText(/Cache prüfen/)).toBeTruthy();
    expect(screen.getByText('verworfen: doppelt')).toBeTruthy();
    expect(screen.getByRole('button', { name: 'Alle' }).getAttribute('aria-pressed')).toBe('true');
  });

  it('behält beim Filterwechsel die bisherige Liste, bis die neue da ist', async () => {
    let deliver: (notes: DbNoteDto[]) => void = () => undefined;
    const getDbNotes = vi.fn((_board: number, filter: string) => (filter === 'open' ? Promise.resolve([NOTE]) : new Promise<DbNoteDto[]>((r) => { deliver = r; })));
    render(<ApiProvider api={fakeApi({ getDbNotes })}><DbNotesDialog boardId={1} onClose={vi.fn()} onChanged={vi.fn()} onOpenTicket={vi.fn()} /></ApiProvider>);
    await screen.findByText(/Cache prüfen/);
    await userEvent.click(screen.getByRole('button', { name: 'Übernommen' }));
    expect(screen.getByText(/Cache prüfen/)).toBeTruthy();
    deliver([]);
    expect(await screen.findByText('Nichts mit diesem Stand.')).toBeTruthy();
    expect(screen.queryByText(/Cache prüfen/)).toBeNull();
  });

  it('sendet eine Notiz nur einmal und lässt den Fokus im Textfeld', async () => {
    let finish: (note: DbNoteDto) => void = () => undefined;
    const addDbNote = vi.fn(() => new Promise<DbNoteDto>((r) => { finish = r; }));
    render(<ApiProvider api={fakeApi({ addDbNote })}><DbNotesDialog boardId={1} onClose={vi.fn()} onChanged={vi.fn()} onOpenTicket={vi.fn()} /></ApiProvider>);
    await userEvent.type(screen.getByLabelText('Neue Notiz'), 'Logs');
    const button = screen.getByRole('button', { name: 'Notiz anlegen' });
    await userEvent.click(button);
    expect((button as HTMLButtonElement).disabled).toBe(true);
    await userEvent.click(button);
    expect(addDbNote).toHaveBeenCalledTimes(1);
    finish(NOTE);
    await waitFor(() => expect((screen.getByLabelText('Neue Notiz') as HTMLTextAreaElement).value).toBe(''));
    expect(document.activeElement).toBe(screen.getByLabelText('Neue Notiz'));
  });

  it('Esc schließt erst das offene Formular, ein geänderter Ticket-Titel gilt als Eingabe', async () => {
    const onClose = vi.fn();
    render(<ApiProvider api={fakeApi({ getDbNotes: async () => [NOTE] })}><DbNotesDialog boardId={1} onClose={onClose} onChanged={vi.fn()} onOpenTicket={vi.fn()} /></ApiProvider>);
    await userEvent.click(await screen.findByRole('button', { name: 'Ticket daraus machen' }));
    await userEvent.keyboard('{Escape}');
    expect(screen.queryByLabelText('Titel des Tickets')).toBeNull();
    expect(screen.getByRole('dialog', { name: 'Notizen' })).toBeTruthy();
    expect(onClose).not.toHaveBeenCalled();
    await userEvent.click(screen.getByRole('button', { name: 'Verwerfen' }));
    await userEvent.keyboard('{Escape}');
    expect(screen.queryByLabelText('Grund')).toBeNull();
    expect(onClose).not.toHaveBeenCalled();
    await userEvent.click(screen.getByRole('button', { name: 'Ticket daraus machen' }));
    await userEvent.type(screen.getByLabelText('Titel des Tickets'), ' neu');
    await userEvent.click(screen.getByRole('button', { name: 'Notizen schließen' }));
    expect(screen.getByRole('alertdialog', { name: 'Eingaben verwerfen' })).toBeTruthy();
    expect(onClose).not.toHaveBeenCalled();
    // Esc: Rückfrage zu, dann Formular zu, dann Dialog zu
    await userEvent.keyboard('{Escape}');
    expect(screen.queryByRole('alertdialog')).toBeNull();
    await userEvent.keyboard('{Escape}');
    expect(screen.queryByLabelText('Titel des Tickets')).toBeNull();
    await userEvent.keyboard('{Escape}');
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('zeigt Titel, Details und wie oft eine Notiz aufgefallen ist', async () => {
    const titled: DbNoteDto = { ...NOTE, title: 'Cache läuft nie ab', body: 'BoardViewService ohne TTL', seenCount: 3, seenIn: ['ARC-0004'] };
    render(<ApiProvider api={fakeApi({ getDbNotes: async () => [titled] })}><DbNotesDialog boardId={1} onClose={vi.fn()} onChanged={vi.fn()} onOpenTicket={vi.fn()} /></ApiProvider>);
    expect(await screen.findByText('Cache läuft nie ab')).toBeTruthy();
    expect(screen.getByText('BoardViewService ohne TTL')).toBeTruthy();
    expect(screen.getByText('3× aufgefallen').getAttribute('title')).toBe('auch bei ARC-0004');
    await userEvent.click(screen.getByRole('button', { name: 'Ticket daraus machen' }));
    expect((screen.getByLabelText('Titel des Tickets') as HTMLInputElement).value).toBe('Cache läuft nie ab');
  });

  it('bietet bei einem schon notierten Befund Hochzählen oder Trotzdem-Anlegen an', async () => {
    const similar = { notes: [{ id: 1, title: 'Cache prüfen', state: 'open', ticketId: null }, { id: 4, title: 'Cache leert sich', state: 'taken', ticketId: 'ARC-0009' }] };
    const addDbNote = vi.fn(async (_board: number, input: { force?: boolean }) => {
      if (!input.force) throw new ApiError('note-similar', 'Diesen Befund gibt es vermutlich schon.', 422, undefined, similar);
      return NOTE;
    });
    const seenDbNote = vi.fn(async () => ({ ...NOTE, seenCount: 2 }));
    const onOpenTicket = vi.fn();
    render(<ApiProvider api={fakeApi({ addDbNote, seenDbNote })}><DbNotesDialog boardId={1} onClose={vi.fn()} onChanged={vi.fn()} onOpenTicket={onOpenTicket} /></ApiProvider>);
    await userEvent.type(screen.getByLabelText('Neue Notiz'), 'Cache prüfen');
    await userEvent.click(screen.getByRole('button', { name: 'Notiz anlegen' }));
    const alert = await screen.findByRole('alert');
    expect(alert.textContent).toContain('Diesen Befund gibt es vermutlich schon');
    expect(within(alert).getByRole('button', { name: 'schon als ARC-0009' })).toBeTruthy();
    await userEvent.click(within(alert).getByRole('button', { name: 'Trotzdem anlegen' }));
    expect(addDbNote).toHaveBeenLastCalledWith(1, { title: 'Cache prüfen', body: '', labels: [], force: true });
    await waitFor(() => expect((screen.getByLabelText('Neue Notiz') as HTMLInputElement).value).toBe(''));

    await userEvent.type(screen.getByLabelText('Neue Notiz'), 'Cache prüfen');
    await userEvent.click(screen.getByRole('button', { name: 'Notiz anlegen' }));
    await userEvent.click(await screen.findByRole('button', { name: 'Ist derselbe – hochzählen' }));
    expect(seenDbNote).toHaveBeenCalledWith(1);
    await waitFor(() => expect(screen.queryByRole('alert')).toBeNull());
    expect(addDbNote).toHaveBeenCalledTimes(3);
  });

  it('zeigt höchstens eine Fehlermeldung zugleich', async () => {
    const getDbNotes = vi.fn(async (_board: number, filter: string) => {
      if (filter === 'open') return [];
      throw new ApiError('db', 'Laden gescheitert.', 500);
    });
    const addDbNote = vi.fn(async () => { throw new ApiError('db', 'Anlegen gescheitert.', 500); });
    render(<ApiProvider api={fakeApi({ getDbNotes, addDbNote })}><DbNotesDialog boardId={1} onClose={vi.fn()} onChanged={vi.fn()} onOpenTicket={vi.fn()} /></ApiProvider>);
    await userEvent.click(screen.getByRole('button', { name: 'Übernommen' }));
    expect((await screen.findByRole('alert')).textContent).toBe('Laden gescheitert.');
    await userEvent.type(screen.getByLabelText('Neue Notiz'), 'x');
    await userEvent.click(screen.getByRole('button', { name: 'Notiz anlegen' }));
    await waitFor(() => expect(screen.getByRole('alert').textContent).toContain('Anlegen gescheitert.'));
    expect(screen.getAllByRole('alert')).toHaveLength(1);
    expect(within(screen.getByRole('alert')).getByText(/Laden gescheitert/)).toBeTruthy();
  });
});

describe('Dialog-Hülle: Fokus zurückholen nur aus dem Nichts', () => {
  // Dialog mit einem Knopf daneben; rerender zeichnet den Dialog neu
  function Harness({ tick, second = false }: { tick: number; second?: boolean }) {
    return (
      <>
        <button type="button">draußen</button>
        <DbModal labelledBy="h1" onEscape={vi.fn()}>
          <strong id="h1">{`Erster ${tick}`}</strong>
          <button type="button">innen</button>
        </DbModal>
        {second && (
          <DbModal labelledBy="h2" onEscape={vi.fn()}>
            <strong id="h2">Zweiter</strong>
            <button type="button">oben</button>
          </DbModal>
        )}
      </>
    );
  }

  it('lässt einem Element außerhalb den Fokus, auch nach erneutem Zeichnen', async () => {
    const { rerender } = render(<Harness tick={0} />);
    const outside = screen.getByRole('button', { name: 'draußen' });
    await userEvent.click(outside);
    rerender(<Harness tick={1} />);
    expect(document.activeElement).toBe(outside);
    outside.blur();
    rerender(<Harness tick={2} />);
    expect(document.activeElement).toBe(document.body);
  });

  it('holt den Fokus zurück, wenn er aus dem Dialog ins Leere geht', async () => {
    render(<Harness tick={0} />);
    const inside = screen.getByRole('button', { name: 'innen' });
    inside.focus();
    inside.blur();
    await waitFor(() => expect(document.activeElement).toBe(screen.getByRole('dialog', { name: 'Erster 0' })));
  });

  it('holt den Fokus nur im obersten Dialog zurück', async () => {
    const { rerender } = render(<Harness tick={0} second />);
    const inside = screen.getByRole('button', { name: 'innen' });
    inside.focus();
    inside.blur();
    rerender(<Harness tick={1} second />);
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(document.activeElement).toBe(document.body);
  });
});
