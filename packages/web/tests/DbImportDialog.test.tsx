// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { ApiProvider } from '../src/api/ApiContext.js';
import { ApiError } from '../src/api/ApiError.js';
import type { DbBoardDto } from '../src/api/dbTypes.js';
import { DbImportDialog } from '../src/components/db/DbImportDialog.js';
import { dbBoards, dbCard, dbView, fakeApi } from './helpers/fakeApi.js';

afterEach(cleanup);

const KOLLEGIN: DbBoardDto = { ...dbBoards[1]!, id: 4, name: 'Kollegin', ownerId: 'u2', ownerName: 'Anna' };
const BOARDS = [...dbBoards, KOLLEGIN];

const SOURCE_CARDS = [
  dbCard('ARC-0005', 'Export', 'backlog', { boardId: 2, priority: 'high', assignee: 'claude' }),
  dbCard('ARC-0006', 'Import', 'backlog', { boardId: 2 }),
  dbCard('ARC-0007', 'Planbar', 'planned', { boardId: 2 }),
  dbCard('ARC-0008', 'Fertig', 'done', { boardId: 2, hasAcceptance: true }),
  dbCard('ARC-0009', 'Verworfen', 'not_needed', { boardId: 2 }),
];

// Rendert den Dialog für Board 1 (Sprint, ARC) mit austauschbaren Aufrufen
function open(overrides: Parameters<typeof fakeApi>[0] = {}, props: { onImported?: (n: number) => void; onClose?: () => void } = {}) {
  const getDbBoardView = vi.fn(async (id: number) => dbView(id === 2 ? SOURCE_CARDS : [dbCard('ARC-0010', 'Von Anna', 'backlog', { boardId: 4 })], id));
  const api = fakeApi({ getDbBoardView, ...overrides });
  const onImported = props.onImported ?? vi.fn();
  const onClose = props.onClose ?? vi.fn();
  render(<ApiProvider api={api}><DbImportDialog board={dbBoards[0]!} boards={BOARDS} onClose={onClose} onImported={onImported} /></ApiProvider>);
  return { api, getDbBoardView, onImported, onClose };
}

// Abschnitt einer Spalte (Kopf-Schalter mit aria-expanded)
const section = (title: RegExp) => screen.getByRole('button', { name: title });

describe('DbImportDialog', () => {
  it('bietet nur die anderen Boards desselben Projekts mit Besitzer an und lädt das erste', async () => {
    const { getDbBoardView } = open();
    expect(screen.getByRole('dialog', { name: 'Von anderem Board holen' })).toBeTruthy();
    const select = screen.getByLabelText('Quell-Board') as HTMLSelectElement;
    expect(Array.from(select.options).map((o) => o.text)).toEqual(['Nächster (Florian)', 'Kollegin (Anna)']);
    expect(await screen.findByText('Export')).toBeTruthy();
    expect(getDbBoardView).toHaveBeenCalledWith(2);
  });

  it('zeigt je Spalte einen Abschnitt mit Zahl; leere fehlen, Erledigt und Nicht mehr nötig sind zugeklappt', async () => {
    open();
    await screen.findByText('Export');
    expect(section(/^Backlog \(2\)/).getAttribute('aria-expanded')).toBe('true');
    expect(section(/^Geplant \(1\)/).getAttribute('aria-expanded')).toBe('true');
    expect(section(/^Erledigt \(1\)/).getAttribute('aria-expanded')).toBe('false');
    expect(section(/^Nicht mehr nötig \(1\)/).getAttribute('aria-expanded')).toBe('false');
    expect(screen.queryByRole('button', { name: /^In Arbeit/ })).toBeNull();
    expect(screen.queryByText('Fertig')).toBeNull();
    await userEvent.click(section(/^Erledigt/));
    expect(section(/^Erledigt/).getAttribute('aria-expanded')).toBe('true');
    expect(screen.getByRole('checkbox', { name: /^ARC-0008 Fertig/ })).toBeTruthy();
    await userEvent.click(section(/^Backlog/));
    expect(screen.queryByText('Export')).toBeNull();
  });

  it('zeigt Zuständigen und Priorität (außer normal) in der Zeile', async () => {
    open();
    const row = (await screen.findByRole('checkbox', { name: /^ARC-0005 Export/ })).closest('label')!;
    expect(within(row).getByText('@claude')).toBeTruthy();
    expect(within(row).getByText('hoch')).toBeTruthy();
    const plain = screen.getByRole('checkbox', { name: /^ARC-0006 Import/ }).closest('label')!;
    expect(within(plain).queryByText('normal')).toBeNull();
  });

  it('wählt je Spalte alle oder keine und zählt im Knopf mit', async () => {
    open();
    await screen.findByText('Export');
    const submit = () => screen.getByRole('button', { name: /^Importieren/ }) as HTMLButtonElement;
    expect(submit().textContent).toBe('Importieren (0)');
    expect(submit().disabled).toBe(true);
    await userEvent.click(screen.getByRole('button', { name: 'Alle in Backlog wählen' }));
    expect(submit().textContent).toBe('Importieren (2)');
    expect((screen.getByRole('checkbox', { name: /^ARC-0006/ }) as HTMLInputElement).checked).toBe(true);
    await userEvent.click(screen.getByRole('button', { name: 'Alle in Erledigt wählen' }));
    expect(submit().textContent).toBe('Importieren (3)');
    await userEvent.click(screen.getByRole('button', { name: 'Keine in Backlog' }));
    expect(submit().textContent).toBe('Importieren (1)');
    await userEvent.click(screen.getByRole('checkbox', { name: /^ARC-0007/ }));
    expect(submit().textContent).toBe('Importieren (2)');
    expect(submit().disabled).toBe(false);
  });

  it('holt die gewählten Tickets in Anzeige-Reihenfolge und meldet die Zahl', async () => {
    const importDbTickets = vi.fn(async () => ({ moved: [SOURCE_CARDS[1], SOURCE_CARDS[2]] }));
    const { onImported } = open({ importDbTickets });
    await userEvent.click(await screen.findByRole('checkbox', { name: /^ARC-0007/ }));
    await userEvent.click(screen.getByRole('checkbox', { name: /^ARC-0006/ }));
    await userEvent.click(screen.getByRole('button', { name: 'Importieren (2)' }));
    expect(importDbTickets).toHaveBeenCalledWith(1, ['ARC-0006', 'ARC-0007']);
    await waitFor(() => expect(onImported).toHaveBeenCalledWith(2));
  });

  it('zeigt einen Fehler im Dialog und behält die Auswahl', async () => {
    const importDbTickets = vi.fn(async () => { throw new ApiError('ticket-already-on-board', 'ARC-0006 liegt schon auf Board Sprint.', 422); });
    const { onImported, onClose } = open({ importDbTickets });
    await userEvent.click(await screen.findByRole('checkbox', { name: /^ARC-0006/ }));
    await userEvent.click(screen.getByRole('button', { name: 'Importieren (1)' }));
    expect((await screen.findByRole('alert')).textContent).toContain('liegt schon auf Board Sprint');
    expect((screen.getByRole('checkbox', { name: /^ARC-0006/ }) as HTMLInputElement).checked).toBe(true);
    expect(screen.getByRole('button', { name: 'Importieren (1)' })).toBeTruthy();
    expect(onImported).not.toHaveBeenCalled();
    expect(onClose).not.toHaveBeenCalled();
  });

  it('leert beim Wechsel des Quell-Boards die Auswahl und lädt dessen Ansicht', async () => {
    const { getDbBoardView } = open();
    await userEvent.click(await screen.findByRole('checkbox', { name: /^ARC-0006/ }));
    expect(screen.getByRole('button', { name: 'Importieren (1)' })).toBeTruthy();
    await userEvent.selectOptions(screen.getByLabelText('Quell-Board'), '4');
    expect(getDbBoardView).toHaveBeenLastCalledWith(4);
    expect(await screen.findByText('Von Anna')).toBeTruthy();
    expect(screen.getByRole('button', { name: 'Importieren (0)' })).toBeTruthy();
    await userEvent.selectOptions(screen.getByLabelText('Quell-Board'), '2');
    expect((await screen.findByRole('checkbox', { name: /^ARC-0006/ }) as HTMLInputElement).checked).toBe(false);
  });

  it('schließt mit Abbrechen und Esc', async () => {
    const { onClose } = open();
    await screen.findByText('Export');
    await userEvent.click(screen.getByRole('button', { name: 'Abbrechen' }));
    expect(onClose).toHaveBeenCalledTimes(1);
    await userEvent.keyboard('{Escape}');
    expect(onClose).toHaveBeenCalledTimes(2);
  });

  it('meldet ein leeres Quell-Board', async () => {
    open({ getDbBoardView: vi.fn(async (id: number) => dbView([], id)) });
    expect(await screen.findByText('Auf diesem Board liegen keine Tickets.')).toBeTruthy();
  });
});
