// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, cleanup, fireEvent, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { ApiError } from '../src/api/ApiError.js';
import type { DbTicketPatch } from '../src/api/dbTypes.js';
import { ACCEPTANCE_QUESTION } from '../src/components/db/DbAcceptanceDialog.js';
import { DbBoardPage } from '../src/pages/DbBoardPage.js';
import { DETAIL, openPanel } from './helpers/dbPanel.js';
import { dbCard, dbView, fakeApi } from './helpers/fakeApi.js';
import { FakeEventSource } from './helpers/FakeEventSource.js';
import { renderWithApp } from './helpers/renderWithApp.js';

vi.mock('mermaid', () => ({ default: { initialize: vi.fn(), render: vi.fn(async () => ({ svg: '<svg viewBox="0 0 100 50"></svg>' })) } }));
afterEach(cleanup);

describe('DbTicketPanel', () => {
  it('zeigt Titel, Version und Felder des Tickets', async () => {
    openPanel();
    expect(((await screen.findByLabelText('Titel')) as HTMLInputElement).value).toBe('Login bauen');
    expect((screen.getByLabelText('Status') as HTMLSelectElement).value).toBe('backlog');
    expect((screen.getByLabelText('Priorität') as HTMLSelectElement).value).toBe('normal');
    expect((screen.getByLabelText('Zuständig') as HTMLInputElement).value).toBe('claude');
    expect((screen.getByLabelText('Labels') as HTMLInputElement).value).toBe('ui');
    expect((screen.getByLabelText('Abnahme') as HTMLTextAreaElement).value).toBe('Login geht');
    expect((screen.getByLabelText('Braucht Planung') as HTMLInputElement).checked).toBe(true);
    expect(screen.getByText(/Version 3/)).toBeTruthy();
  });

  it('speichert den Titel mit Enter und Felder beim Verlassen, jeweils mit der gelesenen Version', async () => {
    const { patchDbTicket, onChanged } = openPanel();
    const title = await screen.findByLabelText('Titel');
    await userEvent.clear(title);
    await userEvent.type(title, 'Login fertig{Enter}');
    expect(patchDbTicket).toHaveBeenCalledWith('ARC-0001', { title: 'Login fertig', rev: 3 });
    await waitFor(() => expect(onChanged).toHaveBeenCalled());
    const labels = screen.getByLabelText('Labels');
    await userEvent.clear(labels);
    await userEvent.type(labels, 'ui, api');
    await userEvent.tab();
    expect(patchDbTicket).toHaveBeenLastCalledWith('ARC-0001', { labels: ['ui', 'api'], rev: 4 });
    await userEvent.selectOptions(screen.getByLabelText('Status'), 'planned');
    expect(patchDbTicket).toHaveBeenLastCalledWith('ARC-0001', { status: 'planned', rev: 5 });
    await userEvent.click(screen.getByLabelText('Braucht Planung'));
    expect(patchDbTicket).toHaveBeenLastCalledWith('ARC-0001', { needsPlanning: false, rev: 6 });
  });

  it('leert Zuständig und Blockiert-Grund als null', async () => {
    const { patchDbTicket } = openPanel();
    await userEvent.clear(await screen.findByLabelText('Zuständig'));
    await userEvent.tab();
    expect(patchDbTicket).toHaveBeenLastCalledWith('ARC-0001', { assignee: null, rev: 3 });
  });

  it('zeigt bei einem Konflikt den Dialog und speichert auf Wunsch auf den aktuellen Stand', async () => {
    const current = { ...DETAIL, title: 'Von jemand anderem', rev: 4 };
    const patchDbTicket = vi.fn()
      .mockRejectedValueOnce(new ApiError('conflict', 'veraltet', 409, current))
      .mockImplementation(async (_id: string, patch: DbTicketPatch) => ({ ...current, ...patch, rev: 5 }));
    openPanel({ patchDbTicket });
    await userEvent.selectOptions(await screen.findByLabelText('Priorität'), 'high');
    expect(await screen.findByRole('alertdialog', { name: /zwischenzeitlich geändert/ })).toBeTruthy();
    await userEvent.click(screen.getByRole('button', { name: 'Meine Änderung trotzdem speichern' }));
    expect(patchDbTicket).toHaveBeenLastCalledWith('ARC-0001', { priority: 'high', rev: 4 });
    await waitFor(() => expect(screen.queryByRole('alertdialog')).toBeNull());
  });

  it('lädt bei einem Konflikt auf Wunsch den aktuellen Stand', async () => {
    const current = { ...DETAIL, title: 'Von jemand anderem', rev: 4 };
    openPanel({ patchDbTicket: vi.fn(async () => { throw new ApiError('conflict', 'veraltet', 409, current); }) });
    await userEvent.selectOptions(await screen.findByLabelText('Priorität'), 'high');
    await userEvent.click(await screen.findByRole('button', { name: 'Aktuellen Stand laden' }));
    await waitFor(() => expect((screen.getByLabelText('Titel') as HTMLInputElement).value).toBe('Von jemand anderem'));
  });

  it('zeigt fachliche Fehler des Servers', async () => {
    openPanel({ patchDbTicket: vi.fn(async () => { throw new ApiError('acceptance-missing', 'ARC-0001 hat keine Abnahme.', 422); }) });
    await userEvent.selectOptions(await screen.findByLabelText('Status'), 'done');
    expect((await screen.findByRole('alert')).textContent).toContain('keine Abnahme');
  });

  it('fragt bei Status Erledigt ohne Abnahme erst die Abnahme ab und speichert beides zusammen', async () => {
    const { patchDbTicket } = openPanel({}, { ...DETAIL, acceptance: '' });
    await userEvent.selectOptions(await screen.findByLabelText('Status'), 'done');
    expect(await screen.findByRole('dialog', { name: ACCEPTANCE_QUESTION })).toBeTruthy();
    expect(patchDbTicket).not.toHaveBeenCalled();
    await userEvent.click(screen.getByRole('button', { name: 'Abbrechen' }));
    expect(screen.queryByRole('dialog', { name: ACCEPTANCE_QUESTION })).toBeNull();
    expect(patchDbTicket).not.toHaveBeenCalled();
    expect((screen.getByLabelText('Status') as HTMLSelectElement).value).toBe('backlog');
    await userEvent.selectOptions(screen.getByLabelText('Status'), 'done');
    await userEvent.type(await screen.findByRole('textbox', { name: ACCEPTANCE_QUESTION }), 'Login klappt');
    await userEvent.click(screen.getByRole('button', { name: 'Verschieben' }));
    expect(patchDbTicket).toHaveBeenCalledWith('ARC-0001', { status: 'done', acceptance: 'Login klappt', rev: 3 });
    await waitFor(() => expect((screen.getByLabelText('Status') as HTMLSelectElement).value).toBe('done'));
    expect(screen.getByRole('dialog', { name: 'Ticket ARC-0001' })).toBeTruthy();
  });

  it('löscht erst nach Bestätigung und schließt dann', async () => {
    const deleteDbTicket = vi.fn(async () => undefined);
    const { onClose, onChanged } = openPanel({ deleteDbTicket });
    await userEvent.click(await screen.findByRole('button', { name: 'Ticket löschen' }));
    expect(deleteDbTicket).not.toHaveBeenCalled();
    await userEvent.click(screen.getByRole('button', { name: 'Endgültig löschen' }));
    expect(deleteDbTicket).toHaveBeenCalledWith('ARC-0001');
    await waitFor(() => expect(onClose).toHaveBeenCalled());
    expect(onChanged).toHaveBeenCalled();
  });

  it('kopiert die Ticket-ID', async () => {
    const writeText = vi.fn(async () => undefined);
    Object.defineProperty(navigator, 'clipboard', { value: { writeText }, configurable: true });
    openPanel();
    await screen.findByLabelText('Titel');
    await userEvent.click(screen.getByRole('button', { name: 'Ticket-ID kopieren' }));
    expect(writeText).toHaveBeenCalledWith('ARC-0001');
    expect((await screen.findByRole('status')).textContent).toBe('Kopiert');
  });

  it('fragt bei Esc mit ungespeichertem Feld nach und verwirft auf Wunsch ohne zu speichern', async () => {
    const { onClose, patchDbTicket } = openPanel();
    await userEvent.type(await screen.findByLabelText('Zuständig'), 'x{Escape}');
    expect(await screen.findByRole('alertdialog', { name: 'Ungespeicherte Änderungen' })).toBeTruthy();
    expect(onClose).not.toHaveBeenCalled();
    await userEvent.click(screen.getByRole('button', { name: 'Verwerfen' }));
    expect(onClose).toHaveBeenCalled();
    expect(patchDbTicket).not.toHaveBeenCalled();
  });

  it('schließt ohne Rückfrage, wenn nichts offen ist', async () => {
    const { onClose } = openPanel();
    await screen.findByLabelText('Titel');
    await userEvent.click(screen.getByRole('button', { name: 'Schließen' }));
    expect(onClose).toHaveBeenCalled();
  });

  it('öffnet das Ticket aus der Karte im Seitenpanel mit eigener Adresse und schließt es wieder', async () => {
    const api = fakeApi({ getDbBoardView: async () => dbView([dbCard('ARC-0001', 'Login bauen')]), getDbTicket: async () => DETAIL });
    renderWithApp(<DbBoardPage />, { api, path: '/p/p1/board', route: '/p/:pid/board/*' });
    await userEvent.click(await screen.findByRole('button', { name: /^ARC-0001/ }));
    expect(await screen.findByRole('dialog', { name: 'Ticket ARC-0001' })).toBeTruthy();
    await userEvent.click(screen.getByRole('button', { name: 'Schließen' }));
    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Ticket ARC-0001' })).toBeNull());
  });
});

describe('DbTicketPanel – Speicherreihenfolge', () => {
  it('schickt kurz nacheinander ausgelöste Speichervorgänge nacheinander mit der jeweils neuen Version', async () => {
    // Server-Nachbau: prüft die Version wie der echte Server, die erste Antwort kommt erst auf Freigabe
    let current = DETAIL;
    let release: () => void = () => {};
    const gate = new Promise<void>((resolve) => (release = resolve));
    const patchDbTicket = vi.fn(async (_id: string, patch: DbTicketPatch) => {
      if (patchDbTicket.mock.calls.length === 1) await gate;
      if (patch.rev !== current.rev) throw new ApiError('conflict', 'veraltet', 409, current);
      current = { ...current, ...patch, rev: current.rev + 1 };
      return current;
    });
    openPanel({ patchDbTicket });
    const title = (await screen.findByLabelText('Titel')) as HTMLInputElement;
    fireEvent.change(title, { target: { value: 'Login bauen v2' } });
    fireEvent.blur(title);
    const acceptance = screen.getByLabelText('Abnahme') as HTMLTextAreaElement;
    fireEvent.change(acceptance, { target: { value: 'Login geht schnell' } });
    fireEvent.blur(acceptance);
    await waitFor(() => expect(patchDbTicket).toHaveBeenCalledTimes(1));
    await new Promise((resolve) => setTimeout(resolve, 20));
    expect(patchDbTicket).toHaveBeenCalledTimes(1);
    release();
    await waitFor(() => expect(patchDbTicket).toHaveBeenCalledTimes(2));
    expect(patchDbTicket.mock.calls[0][1]).toEqual({ title: 'Login bauen v2', rev: 3 });
    expect(patchDbTicket.mock.calls[1][1]).toEqual({ acceptance: 'Login geht schnell', rev: 4 });
    expect(await screen.findByText(/Version 5/)).toBeTruthy();
    expect(screen.queryByRole('alertdialog', { name: /zwischenzeitlich geändert/ })).toBeNull();
    expect(screen.queryByText('veraltet')).toBeNull();
  });
});

// Antwort, die erst auf Freigabe kommt
function gated() {
  let release: () => void = () => {};
  const gate = new Promise<void>((resolve) => (release = resolve));
  return { gate, release: () => release() };
}

const pause = () => new Promise((resolve) => setTimeout(resolve, 20));

describe('DbTicketPanel – Eingaben bleiben erhalten', () => {
  beforeEach(() => FakeEventSource.reset());

  it('schickt bei offenem Konflikt keine weiteren Änderungen; der übrige Entwurf bleibt nach „Aktuellen Stand laden“ erhalten', async () => {
    const current = { ...DETAIL, title: 'Von jemand anderem', rev: 4 };
    const { gate, release } = gated();
    const patchDbTicket = vi.fn(async () => {
      await gate;
      throw new ApiError('conflict', 'veraltet', 409, current);
    });
    const { onClose } = openPanel({ patchDbTicket });
    const title = (await screen.findByLabelText('Titel')) as HTMLInputElement;
    fireEvent.change(title, { target: { value: 'Login bauen v2' } });
    fireEvent.blur(title);
    const acceptance = screen.getByLabelText('Abnahme') as HTMLTextAreaElement;
    fireEvent.change(acceptance, { target: { value: 'Login geht schnell' } });
    fireEvent.blur(acceptance);
    await waitFor(() => expect(patchDbTicket).toHaveBeenCalledTimes(1));
    release();
    expect(await screen.findByRole('alertdialog', { name: /zwischenzeitlich geändert/ })).toBeTruthy();
    await pause();
    expect(patchDbTicket).toHaveBeenCalledTimes(1);
    await userEvent.click(screen.getByRole('button', { name: 'Aktuellen Stand laden' }));
    await waitFor(() => expect(title.value).toBe('Von jemand anderem'));
    expect(acceptance.value).toBe('Login geht schnell');
    expect(screen.getByText(/Version 4/)).toBeTruthy();
    await userEvent.click(screen.getByRole('button', { name: 'Schließen' }));
    expect(await screen.findByRole('alertdialog', { name: 'Ungespeicherte Änderungen' })).toBeTruthy();
    expect(onClose).not.toHaveBeenCalled();
  });

  it('behält bearbeitete Felder, wenn das Ticket durch eine andere Änderung neu kommt', async () => {
    openPanel();
    const assignee = (await screen.findByLabelText('Zuständig')) as HTMLInputElement;
    fireEvent.change(assignee, { target: { value: 'florian' } });
    fireEvent.change(screen.getByLabelText('Priorität'), { target: { value: 'high' } });
    expect(await screen.findByText(/Version 4/)).toBeTruthy();
    expect(assignee.value).toBe('florian');
    expect((screen.getByLabelText('Titel') as HTMLInputElement).value).toBe('Login bauen');
  });

  it('lädt eine Live-Meldung erst nach, wenn nichts mehr ungespeichert ist', async () => {
    const getDbTicket = vi.fn().mockResolvedValueOnce(DETAIL).mockResolvedValue({ ...DETAIL, assignee: 'florian', rev: 4 });
    openPanel({ getDbTicket });
    const assignee = (await screen.findByLabelText('Zuständig')) as HTMLInputElement;
    fireEvent.change(assignee, { target: { value: 'florian' } });
    act(() => FakeEventSource.current().emit({ type: 'dbboard', boardId: 1 }));
    await pause();
    expect(getDbTicket).toHaveBeenCalledTimes(1);
    fireEvent.blur(assignee);
    await waitFor(() => expect(getDbTicket).toHaveBeenCalledTimes(2));
  });

  it('behält bei fehlgeschlagenem Live-Nachladen die Anzeige und meldet den Fehler', async () => {
    const getDbTicket = vi.fn().mockResolvedValueOnce(DETAIL).mockRejectedValue(new Error('Netz weg'));
    openPanel({ getDbTicket });
    await screen.findByLabelText('Titel');
    act(() => FakeEventSource.current().emit({ type: 'dbboard', boardId: 1 }));
    expect((await screen.findByRole('alert')).textContent).toContain('Netz weg');
    expect((screen.getByLabelText('Titel') as HTMLInputElement).value).toBe('Login bauen');
  });

  it('speichert beim Klick auf Schließen das gerade bearbeitete Feld und schließt ohne Rückfrage', async () => {
    const { onClose, patchDbTicket } = openPanel();
    await userEvent.type(await screen.findByLabelText('Zuständig'), 'x');
    await userEvent.click(screen.getByRole('button', { name: 'Schließen' }));
    await waitFor(() => expect(onClose).toHaveBeenCalled());
    expect(patchDbTicket).toHaveBeenCalledWith('ARC-0001', { assignee: 'claudex', rev: 3 });
    expect(screen.queryByRole('alertdialog', { name: 'Ungespeicherte Änderungen' })).toBeNull();
  });

  it('bleibt beim Klick auf Schließen offen, wenn das Speichern des Feldes scheitert', async () => {
    const { onClose } = openPanel({ patchDbTicket: vi.fn(async () => { throw new ApiError('internal', 'kaputt', 500); }) });
    await userEvent.type(await screen.findByLabelText('Zuständig'), 'x');
    await userEvent.click(screen.getByRole('button', { name: 'Schließen' }));
    expect((await screen.findByRole('alert')).textContent).toContain('kaputt');
    await pause();
    expect(onClose).not.toHaveBeenCalled();
    expect(screen.getByRole('dialog', { name: 'Ticket ARC-0001' })).toBeTruthy();
  });

  it('löscht erst nach den noch laufenden Speichervorgängen', async () => {
    const { gate, release } = gated();
    const order: string[] = [];
    const patchDbTicket = vi.fn(async (_id: string, patch: DbTicketPatch) => {
      await gate;
      order.push('patch');
      return { ...DETAIL, ...patch, rev: 4 };
    });
    const deleteDbTicket = vi.fn(async () => {
      order.push('delete');
    });
    const { onClose } = openPanel({ patchDbTicket, deleteDbTicket });
    const title = await screen.findByLabelText('Titel');
    fireEvent.change(title, { target: { value: 'Login bauen v2' } });
    fireEvent.blur(title);
    fireEvent.click(screen.getByRole('button', { name: 'Ticket löschen' }));
    fireEvent.click(screen.getByRole('button', { name: 'Endgültig löschen' }));
    await pause();
    expect(deleteDbTicket).not.toHaveBeenCalled();
    release();
    await waitFor(() => expect(onClose).toHaveBeenCalled());
    expect(order).toEqual(['patch', 'delete']);
  });
});

describe('DbTicketPanel – normalisierte Felder', () => {
  beforeEach(() => FakeEventSource.reset());

  it('gilt nach dem Speichern von Labels "a,b" als gespeichert: keine Rückfrage beim Schließen, kein zweiter PATCH beim Verlassen', async () => {
    const { onClose, patchDbTicket } = openPanel();
    const labels = (await screen.findByLabelText('Labels')) as HTMLInputElement;
    await userEvent.clear(labels);
    await userEvent.type(labels, 'a,b');
    await userEvent.tab();
    await waitFor(() => expect(labels.value).toBe('a, b'));
    expect(patchDbTicket).toHaveBeenCalledTimes(1);
    await userEvent.click(labels);
    await userEvent.tab();
    await pause();
    expect(patchDbTicket).toHaveBeenCalledTimes(1);
    await userEvent.click(screen.getByRole('button', { name: 'Schließen' }));
    await waitFor(() => expect(onClose).toHaveBeenCalled());
    expect(screen.queryByRole('alertdialog', { name: 'Ungespeicherte Änderungen' })).toBeNull();
  });

  it('gilt „x “ in Zuständig nach dem Speichern als gespeichert', async () => {
    const { onClose, patchDbTicket } = openPanel();
    const assignee = (await screen.findByLabelText('Zuständig')) as HTMLInputElement;
    await userEvent.clear(assignee);
    await userEvent.type(assignee, 'x ');
    await userEvent.tab();
    await waitFor(() => expect(assignee.value).toBe('x'));
    expect(patchDbTicket).toHaveBeenCalledWith('ARC-0001', { assignee: 'x', rev: 3 });
    await userEvent.click(screen.getByRole('button', { name: 'Schließen' }));
    await waitFor(() => expect(onClose).toHaveBeenCalled());
    expect(screen.queryByRole('alertdialog', { name: 'Ungespeicherte Änderungen' })).toBeNull();
  });

  it('lässt den Entwurf eines anderen bearbeiteten Feldes beim Speichern unangetastet', async () => {
    openPanel();
    const labels = (await screen.findByLabelText('Labels')) as HTMLInputElement;
    const assignee = screen.getByLabelText('Zuständig') as HTMLInputElement;
    fireEvent.change(assignee, { target: { value: 'florian' } });
    fireEvent.change(labels, { target: { value: 'a,b' } });
    fireEvent.blur(labels);
    await waitFor(() => expect(labels.value).toBe('a, b'));
    expect(assignee.value).toBe('florian');
  });

  it('wertet „ui ,backend“ gegen gespeichertes „ui, backend“ nicht als ungespeichert', async () => {
    const { onClose, patchDbTicket } = openPanel({}, { ...DETAIL, labels: ['ui', 'backend'] });
    const labels = (await screen.findByLabelText('Labels')) as HTMLInputElement;
    fireEvent.change(labels, { target: { value: 'ui ,backend' } });
    fireEvent.blur(labels);
    await pause();
    expect(patchDbTicket).not.toHaveBeenCalled();
    await userEvent.click(screen.getByRole('button', { name: 'Schließen' }));
    await waitFor(() => expect(onClose).toHaveBeenCalled());
  });

  it('nimmt die Fehlermeldung des Nachladens zurück, wenn ein späteres Nachladen gelingt', async () => {
    const getDbTicket = vi.fn().mockResolvedValueOnce(DETAIL).mockRejectedValueOnce(new Error('Netz weg')).mockResolvedValue({ ...DETAIL, rev: 4 });
    openPanel({ getDbTicket });
    await screen.findByLabelText('Titel');
    act(() => FakeEventSource.current().emit({ type: 'dbboard', boardId: 1 }));
    expect((await screen.findByRole('alert')).textContent).toContain('Netz weg');
    act(() => FakeEventSource.current().emit({ type: 'dbboard', boardId: 1 }));
    await waitFor(() => expect(screen.queryByRole('alert')).toBeNull());
  });

  describe('bei offenem Konflikt', () => {
    const conflictPanel = async () => {
      const current = { ...DETAIL, title: 'Von jemand anderem', rev: 4 };
      const patchDbTicket = vi.fn(async () => { throw new ApiError('conflict', 'veraltet', 409, current); });
      openPanel({ patchDbTicket });
      await userEvent.selectOptions(await screen.findByLabelText('Priorität'), 'high');
      await screen.findByRole('alertdialog', { name: /zwischenzeitlich geändert/ });
      return { patchDbTicket };
    };

    it('zeigt bei einer Statusänderung den Hinweis', async () => {
      const { patchDbTicket } = await conflictPanel();
      await userEvent.selectOptions(screen.getByLabelText('Status'), 'planned');
      expect(await screen.findByText('Erst den Konflikt auflösen.')).toBeTruthy();
      expect(patchDbTicket).toHaveBeenCalledTimes(1);
    });

    it('zeigt bei Strg+Enter im Editor den Hinweis', async () => {
      const { patchDbTicket } = await conflictPanel();
      await userEvent.click(screen.getByRole('button', { name: 'Bearbeiten' }));
      const editor = screen.getByRole('textbox', { name: 'Beschreibung' });
      fireEvent.change(editor, { target: { value: 'neu' } });
      fireEvent.keyDown(editor, { key: 'Enter', ctrlKey: true });
      expect(await screen.findByText('Erst den Konflikt auflösen.')).toBeTruthy();
      expect(patchDbTicket).toHaveBeenCalledTimes(1);
    });
  });
});

describe('DbTicketPanel – Zielbranch', () => {
  it('speichert den Zielbranch beim Verlassen', async () => {
    const { patchDbTicket } = openPanel();
    const field = await screen.findByLabelText('Zielbranch');
    await userEvent.type(field, 'release/1');
    await userEvent.tab();
    expect(patchDbTicket).toHaveBeenCalledWith('ARC-0001', { targetBranch: 'release/1', rev: 3 });
  });

  it('zeigt den Platzhalter und lässt Leerzeichen am Anfang weg', async () => {
    const { patchDbTicket } = openPanel();
    const field = (await screen.findByLabelText('Zielbranch')) as HTMLInputElement;
    expect(field.placeholder).toBe('ausgecheckter Branch');
    await userEvent.type(field, '  release/1');
    await userEvent.tab();
    expect(patchDbTicket).toHaveBeenCalledWith('ARC-0001', { targetBranch: 'release/1', rev: 3 });
    await waitFor(() => expect(field.value).toBe('release/1'));
  });

  it('leert den Zielbranch als null', async () => {
    const { patchDbTicket } = openPanel({}, { ...DETAIL, targetBranch: 'release/1' });
    const field = (await screen.findByLabelText('Zielbranch')) as HTMLInputElement;
    expect(field.value).toBe('release/1');
    await userEvent.clear(field);
    await userEvent.tab();
    expect(patchDbTicket).toHaveBeenCalledWith('ARC-0001', { targetBranch: null, rev: 3 });
  });

  it('speichert nichts, wenn der Zielbranch unverändert bleibt', async () => {
    const { patchDbTicket } = openPanel();
    await userEvent.click(await screen.findByLabelText('Zielbranch'));
    await userEvent.tab();
    await new Promise((resolve) => setTimeout(resolve, 20));
    expect(patchDbTicket).not.toHaveBeenCalled();
  });

  it('zeigt den Zielbranch nach dem Start nur lesbar', async () => {
    const { patchDbTicket } = openPanel({}, { ...DETAIL, branch: 'ticket/ARC-0001-login', targetBranch: 'release/1' });
    const field = (await screen.findByLabelText('Zielbranch')) as HTMLInputElement;
    expect(field.readOnly).toBe(true);
    expect(field.value).toBe('release/1');
    expect(field.title).toBe('Nach dem Start nicht mehr änderbar');
    await userEvent.type(field, 'x');
    await userEvent.tab();
    expect(field.value).toBe('release/1');
    expect(patchDbTicket).not.toHaveBeenCalled();
  });

  it('lässt den Zielbranch vor dem Start bearbeitbar', async () => {
    openPanel({}, { ...DETAIL, targetBranch: 'release/1' });
    const field = (await screen.findByLabelText('Zielbranch')) as HTMLInputElement;
    expect(field.readOnly).toBe(false);
    expect(field.title).toBe('');
  });

  it('zeigt die Ablehnung des Servers (ungültiger Name) im Hinweis des Panels', async () => {
    const patchDbTicket = vi.fn(async () => { throw new ApiError('target-branch-invalid', 'Der Zielbranch „a b“ ist kein gültiger Branchname.', 422); });
    openPanel({ patchDbTicket });
    await userEvent.type(await screen.findByLabelText('Zielbranch'), 'a..b');
    await userEvent.tab();
    expect((await screen.findByRole('alert')).textContent).toContain('kein gültiger Branchname');
  });
});

describe('DbTicketPanel – Zielbranch mit Vorschlägen', () => {
  // Vorschläge des Board-Repos; die Liste erscheint erst, wenn das Feld den Fokus hat
  const BRANCHES = { getDbBranches: async () => ({ branches: ['main', 'release/1', 'release/2'], reason: null }) };

  // Nur die Einträge der Vorschlagsliste (die Auswahlfelder des Panels bringen eigene <option>-Elemente mit)
  function branchOptions(): string[] {
    const list = screen.queryByRole('listbox', { name: 'Branches' });
    return list ? within(list).queryAllByRole('option').map((o) => o.textContent ?? '') : [];
  }

  it('zeigt die Branches des Repos beim Fokus und filtert beim Tippen', async () => {
    openPanel(BRANCHES);
    const field = await screen.findByLabelText('Zielbranch');
    await userEvent.click(field);
    await waitFor(() => expect(branchOptions()).toEqual(['main', 'release/1', 'release/2']));
    await userEvent.type(field, 'rel');
    expect(branchOptions()).toEqual(['release/1', 'release/2']);
  });

  it('übernimmt einen Vorschlag per Klick und speichert ihn', async () => {
    const { patchDbTicket } = openPanel(BRANCHES);
    await userEvent.click(await screen.findByLabelText('Zielbranch'));
    await waitFor(() => expect(branchOptions()).toContain('release/2'));
    await userEvent.click(within(screen.getByRole('listbox', { name: 'Branches' })).getByText('release/2'));
    expect(patchDbTicket).toHaveBeenCalledWith('ARC-0001', { targetBranch: 'release/2', rev: 3 });
  });

  it('übernimmt einen Vorschlag mit Pfeiltaste und Enter', async () => {
    const { patchDbTicket } = openPanel(BRANCHES);
    const field = await screen.findByLabelText('Zielbranch');
    await userEvent.click(field);
    await waitFor(() => expect(branchOptions()).toContain('main'));
    await userEvent.keyboard('{ArrowDown}{ArrowDown}{Enter}');
    expect(patchDbTicket).toHaveBeenCalledWith('ARC-0001', { targetBranch: 'release/1', rev: 3 });
  });

  it('speichert auch einen Branch, den es noch nicht gibt, und weist auf ihn hin', async () => {
    const { patchDbTicket } = openPanel(BRANCHES);
    const field = await screen.findByLabelText('Zielbranch');
    await userEvent.type(field, 'release/9');
    expect(branchOptions()).toEqual([]);
    expect(screen.getByText('Neuer Branch: release/9')).toBeTruthy();
    await userEvent.tab();
    expect(patchDbTicket).toHaveBeenCalledWith('ARC-0001', { targetBranch: 'release/9', rev: 3 });
  });

  it('ersetzt Leerzeichen beim Tippen durch Bindestriche', async () => {
    const { patchDbTicket } = openPanel(BRANCHES);
    const field = await screen.findByLabelText('Zielbranch');
    await userEvent.type(field, 'mein neuer branch');
    expect((field as HTMLInputElement).value).toBe('mein-neuer-branch');
    await userEvent.tab();
    expect(patchDbTicket).toHaveBeenCalledWith('ARC-0001', { targetBranch: 'mein-neuer-branch', rev: 3 });
  });

  it('zeigt den Speichern-Knopf nur bei einem neuen Branch', async () => {
    openPanel(BRANCHES);
    const field = await screen.findByLabelText('Zielbranch');
    await userEvent.type(field, 'main');
    expect(screen.queryByRole('button', { name: 'Branch erstellen' })).toBeNull();
    await userEvent.type(field, '2');
    expect(screen.getByRole('button', { name: 'Branch erstellen' })).toBeTruthy();
  });

  it('legt einen neuen Branch erst nach Bestätigung an und speichert ihn', async () => {
    const createDbBranch = vi.fn(async (_board: number, name: string) => ({ name, created: true, from: 'main' }));
    const { patchDbTicket } = openPanel({ ...BRANCHES, createDbBranch });
    await userEvent.type(await screen.findByLabelText('Zielbranch'), 'release 9');
    await userEvent.click(screen.getByRole('button', { name: 'Branch erstellen' }));
    const dialog = await screen.findByRole('alertdialog', { name: 'Wollen Sie Branch release-9 wirklich erstellen?' });
    expect(createDbBranch).not.toHaveBeenCalled();
    await userEvent.click(within(dialog).getByRole('button', { name: 'Erstellen' }));
    await waitFor(() => expect(screen.queryByRole('alertdialog')).toBeNull());
    expect(createDbBranch).toHaveBeenCalledWith(1, 'release-9');
    expect(patchDbTicket).toHaveBeenCalledWith('ARC-0001', { targetBranch: 'release-9', rev: 3 });
    expect(screen.queryByRole('button', { name: 'Branch erstellen' })).toBeNull();
  });

  it('legt beim Abbrechen keinen Branch an und zeigt Fehler im Dialog', async () => {
    const createDbBranch = vi.fn(async () => { throw new ApiError('target-branch-invalid', '„x“ ist kein gültiger Branch-Name.', 422); });
    openPanel({ ...BRANCHES, createDbBranch });
    const field = await screen.findByLabelText('Zielbranch');
    await userEvent.type(field, 'neu');
    await userEvent.click(screen.getByRole('button', { name: 'Branch erstellen' }));
    await userEvent.click(within(await screen.findByRole('alertdialog')).getByRole('button', { name: 'Abbrechen' }));
    expect(screen.queryByRole('alertdialog')).toBeNull();
    expect(createDbBranch).not.toHaveBeenCalled();
    await userEvent.click(screen.getByRole('button', { name: 'Branch erstellen' }));
    const dialog = await screen.findByRole('alertdialog');
    await userEvent.click(within(dialog).getByRole('button', { name: 'Erstellen' }));
    expect((await within(dialog).findByRole('alert')).textContent).toContain('kein gültiger Branch-Name');
  });

  it('wählt einen gewählten Branch über das X ab', async () => {
    const { patchDbTicket } = openPanel(BRANCHES, { ...DETAIL, targetBranch: 'release/1' });
    const field = (await screen.findByLabelText('Zielbranch')) as HTMLInputElement;
    await waitFor(() => expect(field.value).toBe('release/1'));
    await userEvent.click(screen.getByRole('button', { name: 'Zielbranch abwählen' }));
    expect(field.value).toBe('');
    expect(patchDbTicket).toHaveBeenCalledWith('ARC-0001', { targetBranch: null, rev: 3 });
    expect(screen.queryByRole('button', { name: 'Zielbranch abwählen' })).toBeNull();
  });

  it('zeigt bei einem neuen Branch kein X, nach dem Start keines von beiden', async () => {
    openPanel(BRANCHES);
    await userEvent.type(await screen.findByLabelText('Zielbranch'), 'neu');
    expect(screen.queryByRole('button', { name: 'Zielbranch abwählen' })).toBeNull();
    cleanup();
    openPanel(BRANCHES, { ...DETAIL, branch: 'ticket/ARC-0001-login', targetBranch: 'release/1' });
    await screen.findByLabelText('Zielbranch');
    expect(screen.queryByRole('button', { name: 'Zielbranch abwählen' })).toBeNull();
  });

  it('bietet ohne Repo keinen Speichern-Knopf an', async () => {
    openPanel({ getDbBranches: async () => ({ branches: [], reason: 'no-repo' as const }) });
    await userEvent.type(await screen.findByLabelText('Zielbranch'), 'neu');
    expect(screen.queryByRole('button', { name: 'Branch erstellen' })).toBeNull();
  });

  it('zeigt nach dem Start keine Vorschläge', async () => {
    openPanel(BRANCHES, { ...DETAIL, branch: 'ticket/ARC-0001-login', targetBranch: 'release/1' });
    await userEvent.click(await screen.findByLabelText('Zielbranch'));
    expect(branchOptions()).toEqual([]);
  });

  it('bleibt ohne Repo und bei einem Fehler ein reines Textfeld', async () => {
    for (const getDbBranches of [async () => ({ branches: [], reason: 'no-repo' as const }), async () => { throw new ApiError('boom', 'kaputt', 500); }]) {
      openPanel({ getDbBranches });
      await userEvent.click(await screen.findByLabelText('Zielbranch'));
      expect(branchOptions()).toEqual([]);
      expect(screen.queryByRole('alert')).toBeNull();
      cleanup();
    }
  });
});

describe('DbTicketPanel – Abhängigkeiten', () => {
  it('zeigt die Ablehnung des Servers (Abhängigkeitskreis) im Hinweis des Panels', async () => {
    const patchDbTicket = vi.fn(async () => { throw new ApiError('dependency-cycle', 'ARC-0003 setzt dieses Ticket bereits voraus.', 422); });
    openPanel({ patchDbTicket }, DETAIL, [dbCard('ARC-0001', 'Login bauen'), dbCard('ARC-0003', 'Kandidat')]);
    await userEvent.click(await screen.findByRole('button', { name: '+ Voraussetzung' }));
    await userEvent.click(screen.getByRole('menuitem', { name: 'ARC-0003 Kandidat' }));
    expect((await screen.findByRole('alert')).textContent).toContain('setzt dieses Ticket bereits voraus');
    expect(patchDbTicket).toHaveBeenCalledWith('ARC-0001', { dependsOn: ['ARC-0003'], rev: 3 });
  });
});
