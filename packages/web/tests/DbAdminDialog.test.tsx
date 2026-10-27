// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter } from 'react-router-dom';
import { App } from '../src/App.js';
import { ApiProvider } from '../src/api/ApiContext.js';
import { ApiError } from '../src/api/ApiError.js';
import type { DbBoardDto } from '../src/api/dbTypes.js';
import { DbAdminDialog } from '../src/components/db/DbAdminDialog.js';
import { dbActive, dbBoards, dbProjects, dbView, fakeApi } from './helpers/fakeApi.js';
import { boardPickerTrigger } from './helpers/boardPicker.js';

afterEach(cleanup);

// Rendert die ganze App (Seitenleiste mit Board-Auswahl und Stift) unter /p/p1/board
function openFullApp(api: ReturnType<typeof fakeApi>) {
  render(
    <ApiProvider api={api}>
      <MemoryRouter initialEntries={['/p/p1/board']}>
        <App />
      </MemoryRouter>
    </ApiProvider>,
  );
}

// Rendert die Verwaltung mit austauschbaren Aufrufen
function open(overrides: Parameters<typeof fakeApi>[0] = {}) {
  const onChanged = vi.fn();
  const onClose = vi.fn();
  render(<ApiProvider api={fakeApi(overrides)}><DbAdminDialog projects={dbProjects} boards={dbBoards} onClose={onClose} onChanged={onChanged} /></ApiProvider>);
  return { onChanged, onClose };
}

// Öffnet den Projekt-Dialog über „+“, füllt ihn und legt an
async function addProject(key: string, name: string) {
  await userEvent.click(screen.getByRole('button', { name: 'Projekt hinzufügen' }));
  await userEvent.type(screen.getByLabelText('Projektkey'), key);
  await userEvent.type(screen.getByLabelText('Projektname'), name);
  await userEvent.click(screen.getByRole('button', { name: 'Projekt anlegen' }));
}

// Klappt das Projekt mit dem Key auf oder zu
async function toggleProject(key: string) {
  await userEvent.click(screen.getByRole('button', { name: new RegExp(`^${key}\\b`) }));
}

// Öffnet den Bearbeiten-Dialog des Boards und liefert ihn
async function editBoard(id: number) {
  await userEvent.click(screen.getByRole('button', { name: `Board ${id} bearbeiten` }));
  return screen.getByRole('dialog', { name: `Board ${id} bearbeiten` });
}

describe('DbAdminDialog', () => {
  it('zeigt Projekte einklappbar, das erste aufgeklappt', async () => {
    open();
    const arc = screen.getByRole('button', { name: /^ARC\b/ });
    const oth = screen.getByRole('button', { name: /^OTH\b/ });
    expect(arc.getAttribute('aria-expanded')).toBe('true');
    expect(oth.getAttribute('aria-expanded')).toBe('false');
    expect(arc.closest('.db-admin-project-head')!.textContent).toContain('2 Boards · 2 offen');
    expect(oth.closest('.db-admin-project-head')!.textContent).toContain('1 Board · 0 offen');
    expect(screen.queryByText('Fremd')).toBeNull();
    await userEvent.click(oth);
    expect(screen.getByText('Fremd')).toBeTruthy();
    await userEvent.click(arc);
    expect(arc.getAttribute('aria-expanded')).toBe('false');
    expect(screen.queryByText('Sprint')).toBeNull();
  });

  it('legt ein Projekt an und benennt eines über den Bearbeiten-Dialog um', async () => {
    const createDbProject = vi.fn(async (key: string, name: string) => ({ ...dbProjects[0], id: 9, key: key.toUpperCase(), name }));
    const renameDbProject = vi.fn(async (key: string, name: string) => ({ ...dbProjects[0], key, name }));
    const { onChanged } = open({ createDbProject, renameDbProject });
    await addProject('new', 'Neues');
    expect(createDbProject).toHaveBeenCalledWith('new', 'Neues');
    expect((await screen.findByRole('status')).textContent).toBe('Projekt NEW angelegt.');
    expect(screen.queryByRole('dialog', { name: 'Projekt hinzufügen' })).toBeNull();
    await userEvent.click(screen.getByRole('button', { name: 'Projekt ARC bearbeiten' }));
    const dialog = screen.getByRole('dialog', { name: 'Projekt ARC bearbeiten' });
    const name = within(dialog).getByLabelText('Projektname');
    expect((name as HTMLInputElement).value).toBe('Arches');
    await userEvent.clear(name);
    await userEvent.type(name, 'Arches neu');
    await userEvent.click(within(dialog).getByRole('button', { name: 'Speichern' }));
    expect(renameDbProject).toHaveBeenCalledWith('ARC', 'Arches neu');
    expect(screen.queryByRole('dialog', { name: 'Projekt ARC bearbeiten' })).toBeNull();
    expect(onChanged).toHaveBeenCalledTimes(2);
  });

  it('legt ein Board an und ändert Name und Repo-Pfad im Bearbeiten-Dialog', async () => {
    const createDbBoard = vi.fn(async (project: string, name: string) => ({ ...dbBoards[2], id: 9, projectKey: project, name }));
    const patchDbBoard = vi.fn(async (id: number, patch: object) => ({ ...dbBoards[0], id, ...patch }));
    open({ createDbBoard, patchDbBoard });
    expect(screen.queryByLabelText('Boardname in OTH')).toBeNull();
    await userEvent.click(screen.getByRole('button', { name: 'Board in OTH hinzufügen' }));
    const addDialog = await screen.findByRole('dialog', { name: 'Board hinzufügen' });
    await userEvent.type(within(addDialog).getByLabelText('Boardname'), 'Neu');
    await userEvent.click(within(addDialog).getByRole('button', { name: 'Board anlegen' }));
    expect(createDbBoard).toHaveBeenCalledWith('OTH', 'Neu');
    expect(screen.queryByRole('dialog', { name: 'Board hinzufügen' })).toBeNull();
    expect((await screen.findByRole('status')).textContent).toBe('Board „Neu“ angelegt.');
    const dialog = await editBoard(1);
    const name = within(dialog).getByLabelText('Boardname');
    await userEvent.clear(name);
    await userEvent.type(name, 'Sprint 2');
    await userEvent.type(within(dialog).getByLabelText('Mein Repo-Pfad'), 'C:/repo');
    await userEvent.click(within(dialog).getByRole('button', { name: 'Speichern' }));
    expect(patchDbBoard).toHaveBeenCalledWith(1, { name: 'Sprint 2', repoPath: 'C:/repo' });
    expect((await screen.findByRole('status')).textContent).toBe('Board 1 geändert.');
    expect(screen.queryByRole('dialog', { name: 'Board 1 bearbeiten' })).toBeNull();
  });

  it('sendet nur Geändertes und schließt ohne Änderung ohne Aufruf', async () => {
    const patchDbBoard = vi.fn(async (id: number, patch: object) => ({ ...dbBoards[0], id, ...patch }));
    open({ patchDbBoard });
    let dialog = await editBoard(1);
    await userEvent.type(within(dialog).getByLabelText('Boardname'), '!');
    await userEvent.click(within(dialog).getByRole('button', { name: 'Speichern' }));
    expect(patchDbBoard).toHaveBeenLastCalledWith(1, { name: 'Sprint!' });
    expect((await screen.findByRole('status')).textContent).toBe('Board 1 umbenannt.');
    dialog = await editBoard(2);
    await userEvent.click(within(dialog).getByRole('button', { name: 'Speichern' }));
    expect(patchDbBoard).toHaveBeenCalledTimes(1);
    expect(screen.queryByRole('dialog', { name: 'Board 2 bearbeiten' })).toBeNull();
  });

  it('wählt den Repo-Pfad über den Ordnerdialog dieses Rechners', async () => {
    const pickDbFolder = vi.fn()
      .mockResolvedValueOnce({ path: 'D:/Repos/Gewählt' })
      .mockResolvedValueOnce({ path: null });
    const patchDbBoard = vi.fn(async (id: number, patch: object) => ({ ...dbBoards[0], id, ...patch }));
    open({ pickDbFolder, patchDbBoard });
    const dialog = await editBoard(1);
    const path = within(dialog).getByLabelText('Mein Repo-Pfad') as HTMLInputElement;
    await userEvent.click(within(dialog).getByRole('button', { name: 'Ordner wählen' }));
    await waitFor(() => expect(path.value).toBe('D:/Repos/Gewählt'));
    expect(pickDbFolder).toHaveBeenLastCalledWith(undefined);
    await userEvent.click(within(dialog).getByRole('button', { name: 'Ordner wählen' }));
    await waitFor(() => expect(pickDbFolder).toHaveBeenCalledTimes(2));
    expect(pickDbFolder).toHaveBeenLastCalledWith('D:/Repos/Gewählt');
    expect(path.value).toBe('D:/Repos/Gewählt'); // Abbruch lässt den Pfad stehen
    await userEvent.click(within(dialog).getByRole('button', { name: 'Speichern' }));
    expect(patchDbBoard).toHaveBeenCalledWith(1, { repoPath: 'D:/Repos/Gewählt' });
  });

  it('zeigt einen Fehler der Ordnerauswahl im Dialog und lässt den Pfad tippbar', async () => {
    const pickDbFolder = vi.fn(async () => {
      throw new ApiError('folder-picker-unavailable', 'Die Ordnerauswahl gibt es nur unter Windows; den Pfad bitte eintippen.', 422);
    });
    open({ pickDbFolder });
    const dialog = await editBoard(1);
    await userEvent.click(within(dialog).getByRole('button', { name: 'Ordner wählen' }));
    expect((await within(dialog).findByRole('alert')).textContent).toContain('nur unter Windows');
    await userEvent.type(within(dialog).getByLabelText('Mein Repo-Pfad'), 'C:/Hand');
    expect((within(dialog).getByLabelText('Mein Repo-Pfad') as HTMLInputElement).value).toBe('C:/Hand');
  });

  it('löscht ein Board mit offenen Tickets erst mit ausdrücklicher Zustimmung', async () => {
    const deleteDbBoard = vi.fn()
      .mockRejectedValueOnce(new ApiError('board-has-open-tickets', 'Auf dem Board liegen noch offene Tickets: ARC-0001.', 422))
      .mockResolvedValue({ deletedTickets: ['ARC-0001'] });
    open({ deleteDbBoard });
    await userEvent.click(screen.getByRole('button', { name: 'Board 1 löschen' }));
    await userEvent.click(screen.getByRole('button', { name: 'Board endgültig löschen' }));
    expect(deleteDbBoard).toHaveBeenCalledWith(1, false);
    expect((await screen.findByRole('alert')).textContent).toContain('offene Tickets');
    await userEvent.click(screen.getByLabelText('Auch offene Tickets endgültig löschen'));
    await userEvent.click(screen.getByRole('button', { name: 'Board endgültig löschen' }));
    expect(deleteDbBoard).toHaveBeenLastCalledWith(1, true);
    expect((await screen.findByRole('status')).textContent).toBe('Board 1 gelöscht.');
  });

  it('wird über den Stift neben der Board-Auswahl geöffnet und lädt nach Änderungen Seitenleiste und Board neu', async () => {
    const getDbProjects = vi.fn(async () => dbProjects);
    const getDbBoardView = vi.fn(async (id: number) => dbView([], id));
    const renameDbProject = vi.fn(async (key: string, name: string) => ({ ...dbProjects[0], key, name }));
    openFullApp(fakeApi({ getDbProjects, getDbBoardView, renameDbProject }));
    await screen.findByRole('heading', { name: 'Board', level: 1 });
    expect(screen.queryByRole('button', { name: 'Verwaltung' })).toBeNull();
    await userEvent.click(await screen.findByRole('button', { name: 'Projekte und Boards verwalten' }));
    expect(screen.getByRole('dialog', { name: 'Verwaltung' })).toBeTruthy();
    await waitFor(() => expect(getDbBoardView).toHaveBeenCalled());
    const projectsBefore = getDbProjects.mock.calls.length;
    const viewsBefore = getDbBoardView.mock.calls.length;
    await userEvent.click(screen.getByRole('button', { name: 'Projekt ARC bearbeiten' }));
    await userEvent.type(screen.getByLabelText('Projektname'), '!');
    await userEvent.click(screen.getByRole('button', { name: 'Speichern' }));
    // Seitenleiste und Board-Seite laden die Projekte neu, die Board-Seite auch die Ansicht
    await waitFor(() => expect(getDbProjects.mock.calls.length).toBeGreaterThanOrEqual(projectsBefore + 2));
    await waitFor(() => expect(getDbBoardView.mock.calls.length).toBeGreaterThan(viewsBefore));
    await userEvent.click(screen.getByRole('button', { name: 'Verwaltung schließen' }));
    expect(screen.queryByRole('dialog', { name: 'Verwaltung' })).toBeNull();
  });
});

describe('DbAdminDialog – Bedienung', () => {
  it('Esc bricht erst die Löschabfrage ab und schließt dann den Dialog', async () => {
    const { onClose } = open();
    await userEvent.click(screen.getByRole('button', { name: 'Board 1 löschen' }));
    expect(screen.getByRole('alertdialog', { name: 'Board 1 löschen' })).toBeTruthy();
    await userEvent.keyboard('{Escape}');
    expect(screen.queryByRole('alertdialog')).toBeNull();
    expect(onClose).not.toHaveBeenCalled();
    await userEvent.keyboard('{Escape}');
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('nennt beim Löschen samt offener Tickets deren Anzahl', async () => {
    open();
    await userEvent.click(screen.getByRole('button', { name: 'Board 1 löschen' }));
    expect(screen.queryByText(/offene Tickets endgültig gelöscht/)).toBeNull();
    await userEvent.click(screen.getByLabelText('Auch offene Tickets endgültig löschen'));
    expect(screen.getByText('Dabei werden 2 offene Tickets endgültig gelöscht.')).toBeTruthy();
  });

  it('sperrt die Knöpfe während des Sendens und sendet nur einmal', async () => {
    let finish: (value: unknown) => void = () => undefined;
    const createDbProject = vi.fn(() => new Promise((resolve) => { finish = resolve; }));
    const { onChanged } = open({ createDbProject } as Parameters<typeof fakeApi>[0]);
    await userEvent.click(screen.getByRole('button', { name: 'Projekt hinzufügen' }));
    await userEvent.type(screen.getByLabelText('Projektkey'), 'NEW');
    await userEvent.type(screen.getByLabelText('Projektname'), 'Neu');
    const button = screen.getByRole('button', { name: 'Projekt anlegen' });
    await userEvent.click(button);
    expect((button as HTMLButtonElement).disabled).toBe(true);
    expect((screen.getByRole('button', { name: 'Board 1 bearbeiten' }) as HTMLButtonElement).disabled).toBe(true);
    expect((screen.getByRole('button', { name: 'Board 1 löschen' }) as HTMLButtonElement).disabled).toBe(true);
    await userEvent.click(button);
    expect(createDbProject).toHaveBeenCalledTimes(1);
    finish(dbProjects[0]);
    expect(await screen.findByRole('status')).toBeTruthy();
    expect(onChanged).toHaveBeenCalledTimes(1);
  });

  it('zeigt Serverfehler im Dialog und meldet keine Änderung', async () => {
    const createDbBoard = vi.fn(async () => {
      throw new ApiError('conflict', 'Ein Board „Neu“ gibt es schon.', 409);
    });
    const { onChanged } = open({ createDbBoard });
    await userEvent.click(screen.getByRole('button', { name: 'Board in ARC hinzufügen' }));
    const dialog = await screen.findByRole('dialog', { name: 'Board hinzufügen' });
    await userEvent.type(within(dialog).getByLabelText('Boardname'), 'Neu');
    await userEvent.click(within(dialog).getByRole('button', { name: 'Board anlegen' }));
    expect(createDbBoard).toHaveBeenCalledWith('ARC', 'Neu');
    expect((await within(dialog).findByRole('alert')).textContent).toBe('Ein Board „Neu“ gibt es schon.');
    expect(onChanged).not.toHaveBeenCalled();
    expect((within(dialog).getByLabelText('Boardname') as HTMLInputElement).value).toBe('Neu');
  });

  it('zeigt Fehler beim Speichern im Bearbeiten-Dialog, der offen bleibt', async () => {
    const patchDbBoard = vi.fn(async () => {
      throw new ApiError('conflict', 'Ein Board „Neu“ gibt es schon.', 409);
    });
    const { onChanged } = open({ patchDbBoard });
    const dialog = await editBoard(1);
    await userEvent.type(within(dialog).getByLabelText('Boardname'), 'x');
    await userEvent.click(within(dialog).getByRole('button', { name: 'Speichern' }));
    const alert = await screen.findByRole('alert');
    expect(dialog.contains(alert)).toBe(true);
    expect(onChanged).not.toHaveBeenCalled();
    expect(screen.getByRole('dialog', { name: 'Board 1 bearbeiten' })).toBeTruthy();
  });

  it('Esc schließt erst den Projekt-Dialog, dann die Verwaltung; Fehler erscheinen im Projekt-Dialog', async () => {
    const createDbProject = vi.fn(async () => {
      throw new ApiError('conflict', 'Projekt gibt es schon.', 409);
    });
    const { onClose } = open({ createDbProject });
    const plus = screen.getByRole('button', { name: 'Projekt hinzufügen' });
    await userEvent.click(plus);
    expect(screen.getByRole('dialog', { name: 'Projekt hinzufügen' })).toBeTruthy();
    await userEvent.type(screen.getByLabelText('Projektkey'), 'ARC');
    await userEvent.type(screen.getByLabelText('Projektname'), 'Doppelt');
    await userEvent.keyboard('{Enter}');
    const alert = await screen.findByRole('alert');
    expect(alert.textContent).toBe('Projekt gibt es schon.');
    expect(screen.getByRole('dialog', { name: 'Projekt hinzufügen' }).contains(alert)).toBe(true);
    await userEvent.keyboard('{Escape}');
    expect(screen.queryByRole('dialog', { name: 'Projekt hinzufügen' })).toBeNull();
    expect(screen.queryByRole('alert')).toBeNull();
    expect(onClose).not.toHaveBeenCalled();
    expect(document.activeElement).toBe(plus);
    await userEvent.keyboard('{Escape}');
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('Esc schließt den Bearbeiten-Dialog und gibt den Fokus an dessen Knopf zurück', async () => {
    const { onClose } = open();
    const edit = screen.getByRole('button', { name: 'Board 1 bearbeiten' });
    await editBoard(1);
    await userEvent.keyboard('{Escape}');
    expect(screen.queryByRole('dialog', { name: 'Board 1 bearbeiten' })).toBeNull();
    expect(onClose).not.toHaveBeenCalled();
    expect(document.activeElement).toBe(edit);
  });

  it('gibt den Fokus beim Schließen an den Stift zurück', async () => {
    openFullApp(fakeApi());
    const trigger = await screen.findByRole('button', { name: 'Projekte und Boards verwalten' });
    await userEvent.click(trigger);
    expect(document.activeElement).toBe(screen.getByRole('button', { name: 'Verwaltung schließen' }));
    await userEvent.keyboard('{Escape}');
    expect(screen.queryByRole('dialog', { name: 'Verwaltung' })).toBeNull();
    expect(document.activeElement).toBe(trigger);
  });
});

describe('DbAdminDialog – Eingaben und Neuladen', () => {
  it('schickt Projektkey und -name ohne Leerzeichen', async () => {
    const createDbProject = vi.fn(async (key: string, name: string) => ({ ...dbProjects[0]!, id: 9, key: key.toUpperCase(), name }));
    open({ createDbProject });
    await addProject('  new ', ' Neues ');
    expect(createDbProject).toHaveBeenCalledWith('new', 'Neues');
    expect((await screen.findByRole('status')).textContent).toBe('Projekt NEW angelegt.');
  });

  it('speichert einen geleerten Repo-Pfad als „kein Pfad“ (null)', async () => {
    const patchDbBoard = vi.fn(async (id: number, patch: object) => ({ ...dbBoards[0]!, id, ...patch }));
    const boards: DbBoardDto[] = [{ ...dbBoards[0]!, repoPath: 'C:/alt' }];
    render(<ApiProvider api={fakeApi({ patchDbBoard })}><DbAdminDialog projects={dbProjects} boards={boards} onClose={vi.fn()} onChanged={vi.fn()} /></ApiProvider>);
    expect(screen.getByText('C:/alt')).toBeTruthy();
    const dialog = await editBoard(1);
    const input = within(dialog).getByLabelText('Mein Repo-Pfad') as HTMLInputElement;
    expect(input.value).toBe('C:/alt');
    await userEvent.clear(input);
    await userEvent.click(within(dialog).getByRole('button', { name: 'Speichern' }));
    expect(patchDbBoard).toHaveBeenCalledWith(1, { repoPath: null });
  });

  it('kennzeichnet einen aus der DB übernommenen Pfad (Altbestand), legt ihn beim Speichern lokal ab und erklärt, dass der Pfad nur lokal gilt', async () => {
    const patchDbBoard = vi.fn(async (id: number, patch: object) => ({ ...dbBoards[0]!, id, ...patch }));
    const boards: DbBoardDto[] = [{ ...dbBoards[0]!, repoPath: 'C:/alt', repoSource: 'legacy' }, { ...dbBoards[1]!, repoPath: 'D:/mein', repoSource: 'local' }];
    render(<ApiProvider api={fakeApi({ patchDbBoard })}><DbAdminDialog projects={dbProjects} boards={boards} onClose={vi.fn()} onChanged={vi.fn()} /></ApiProvider>);
    let dialog = await editBoard(2);
    expect(within(dialog).queryByText(/Aus der gemeinsamen DB übernommen/)).toBeNull();
    expect(within(dialog).getByText(/nur für dich auf diesem Rechner/)).toBeTruthy();
    await userEvent.click(within(dialog).getByRole('button', { name: 'Abbrechen' }));
    dialog = await editBoard(1);
    expect(within(dialog).getByText(/Aus der gemeinsamen DB übernommen/)).toBeTruthy();
    await userEvent.click(within(dialog).getByRole('button', { name: 'Speichern' }));
    expect(patchDbBoard).toHaveBeenCalledWith(1, { repoPath: 'C:/alt' });
  });

  it('zeigt nach dem Neuladen wieder die Werte des Servers', async () => {
    const api = fakeApi();
    const { rerender } = render(<ApiProvider api={api}><DbAdminDialog projects={dbProjects} boards={dbBoards} onClose={vi.fn()} onChanged={vi.fn()} /></ApiProvider>);
    expect(screen.getByText('Sprint')).toBeTruthy();
    const reloaded = dbBoards.map((b) => (b.id === 1 ? { ...b, name: 'Vom Server' } : b));
    rerender(<ApiProvider api={api}><DbAdminDialog projects={[...dbProjects]} boards={reloaded} onClose={vi.fn()} onChanged={vi.fn()} /></ApiProvider>);
    expect(screen.queryByText('Sprint')).toBeNull();
    expect(screen.getByText('Vom Server')).toBeTruthy();
    const dialog = await editBoard(1);
    expect((within(dialog).getByLabelText('Boardname') as HTMLInputElement).value).toBe('Vom Server');
  });

  it('behält beim Neuladen den Entwurf im offenen Bearbeiten-Dialog', async () => {
    const api = fakeApi();
    const { rerender } = render(<ApiProvider api={api}><DbAdminDialog projects={dbProjects} boards={dbBoards} onClose={vi.fn()} onChanged={vi.fn()} /></ApiProvider>);
    const dialog = await editBoard(1);
    await userEvent.type(within(dialog).getByLabelText('Boardname'), '!');
    rerender(<ApiProvider api={api}><DbAdminDialog projects={dbProjects.map((p) => ({ ...p }))} boards={dbBoards.map((b) => ({ ...b }))} onClose={vi.fn()} onChanged={vi.fn()} /></ApiProvider>);
    expect((screen.getByLabelText('Boardname') as HTMLInputElement).value).toBe('Sprint!');
  });

  it('behält beim Neuladen den Namen im offenen Dialog „Board hinzufügen“', async () => {
    const api = fakeApi();
    const { rerender } = render(<ApiProvider api={api}><DbAdminDialog projects={dbProjects} boards={dbBoards} onClose={vi.fn()} onChanged={vi.fn()} /></ApiProvider>);
    await userEvent.click(screen.getByRole('button', { name: 'Board in ARC hinzufügen' }));
    const dialog = await screen.findByRole('dialog', { name: 'Board hinzufügen' });
    await userEvent.type(within(dialog).getByLabelText('Boardname'), 'Entwurf');
    rerender(<ApiProvider api={api}><DbAdminDialog projects={dbProjects.map((p) => ({ ...p }))} boards={dbBoards.map((b) => ({ ...b }))} onClose={vi.fn()} onChanged={vi.fn()} /></ApiProvider>);
    expect((screen.getByLabelText('Boardname') as HTMLInputElement).value).toBe('Entwurf');
  });

  it('nach dem Löschen des aktiven Boards zeigt der Kopf es nicht mehr und lädt es nicht mehr', async () => {
    let activeId: number | null = 1;
    let boards = dbBoards;
    const getDbActive = vi.fn(async () => dbActive(activeId));
    const getDbBoards = vi.fn(async () => boards);
    const getDbBoardView = vi.fn(async (id: number) => dbView([], id));
    const getDbNotes = vi.fn(async (_id: number) => []);
    const deleteDbBoard = vi.fn(async (id: number) => {
      boards = boards.filter((b) => b.id !== id);
      activeId = 2;
      return { deletedTickets: [] };
    });
    openFullApp(fakeApi({ getDbActive, getDbBoards, getDbBoardView, getDbNotes, deleteDbBoard }));
    await waitFor(() => expect(boardPickerTrigger().getAttribute('aria-label')).toBe('Board wählen: Arches › Sprint'));
    await userEvent.click(screen.getByRole('button', { name: 'Projekte und Boards verwalten' }));
    await userEvent.click(screen.getByRole('button', { name: 'Board 1 löschen' }));
    const viewsBefore = getDbBoardView.mock.calls.length;
    const notesBefore = getDbNotes.mock.calls.length;
    await userEvent.click(screen.getByRole('button', { name: 'Board endgültig löschen' }));
    expect((await screen.findByRole('status')).textContent).toBe('Board 1 gelöscht.');
    await waitFor(() => expect(getDbBoardView).toHaveBeenLastCalledWith(2));
    await waitFor(() => expect(boardPickerTrigger().getAttribute('aria-label')).toBe('Board wählen: Arches › Nächster'));
    expect(getDbBoardView.mock.calls.slice(viewsBefore).map(([id]) => id)).not.toContain(1);
    expect(getDbNotes.mock.calls.slice(notesBefore).map(([id]) => id)).not.toContain(1);
    expect(screen.queryByRole('button', { name: 'Board 1 bearbeiten' })).toBeNull();
  });
});
