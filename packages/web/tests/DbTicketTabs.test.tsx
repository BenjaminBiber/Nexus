// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { ApiError } from '../src/api/ApiError.js';
import type { DbHistoryDto, DbTicketPatch } from '../src/api/dbTypes.js';
import { DETAIL, openPanel } from './helpers/dbPanel.js';
import { timelineFrom } from './helpers/fakeApi.js';

vi.mock('mermaid', () => ({ default: { initialize: vi.fn(), render: vi.fn(async () => ({ svg: '<svg viewBox="0 0 100 50"></svg>' })) } }));
afterEach(cleanup);

const HISTORY: DbHistoryDto[] = [
  { ticketId: 'ARC-0001', at: '2026-10-13T10:00:00.000Z', byId: 'u1', byName: 'claude', field: 'status', from: 'backlog', to: 'planned', note: 'verschoben' },
];

describe('DbTicketTabs', () => {
  it('bearbeitet den Plan im Reiter und speichert mit Strg+Enter', async () => {
    const { patchDbTicket } = openPanel();
    await userEvent.click(await screen.findByRole('tab', { name: 'Plan' }));
    expect(screen.getByText('Noch kein Plan.')).toBeTruthy();
    await userEvent.click(screen.getByRole('button', { name: 'Bearbeiten' }));
    await userEvent.type(screen.getByLabelText('Plan'), '# Vorgehen');
    await userEvent.keyboard('{Control>}{Enter}{/Control}');
    expect(patchDbTicket).toHaveBeenCalledWith('ARC-0001', { plan: '# Vorgehen', rev: 3 });
    expect(await screen.findByRole('heading', { name: 'Vorgehen' })).toBeTruthy();
    expect(screen.getByRole('tab', { name: 'Plan', selected: true })).toBeTruthy();
  });

  it('bricht das Bearbeiten ohne Speichern ab', async () => {
    const { patchDbTicket } = openPanel();
    await userEvent.click(await screen.findByRole('button', { name: 'Bearbeiten' }));
    await userEvent.type(screen.getByLabelText('Beschreibung'), 'Entwurf');
    await userEvent.click(screen.getByRole('button', { name: 'Abbrechen' }));
    expect(screen.getByText('Noch keine Beschreibung.')).toBeTruthy();
    expect(patchDbTicket).not.toHaveBeenCalled();
  });

  it('zeichnet das Diagramm aus dem Reiter', async () => {
    openPanel({}, { ...DETAIL, diagram: 'graph TD; A-->B' });
    await userEvent.click(await screen.findByRole('tab', { name: 'Diagramm' }));
    const mermaid = (await import('mermaid')).default;
    await waitFor(() => expect(mermaid.render).toHaveBeenCalledWith(expect.any(String), 'graph TD; A-->B'));
  });

  it('hängt einen Kommentar an und zeigt den Verlauf', async () => {
    const withHistory = { ...DETAIL, history: HISTORY };
    const addDbComment = vi.fn(async (_id: string, body: string) => ({
      ...withHistory, rev: 4,
      comments: [{ id: 1, ticketId: 'ARC-0001', authorId: 'u1', authorName: 'Florian', body, createdAt: '2026-10-13T11:00:00.000Z' }],
    }));
    openPanel({ addDbComment, getDbTicketTimeline: async () => timelineFrom(HISTORY) }, withHistory);
    await userEvent.click(await screen.findByRole('tab', { name: 'Kommentare (0)' }));
    expect(screen.queryByLabelText('Neuer Kommentar')).toBeNull();
    await userEvent.click(screen.getByRole('button', { name: 'Kommentar hinzufügen' }));
    expect(screen.getByRole('dialog', { name: 'Kommentar hinzufügen' })).toBeTruthy();
    await userEvent.type(screen.getByLabelText('Neuer Kommentar'), 'Hallo');
    await userEvent.click(screen.getByRole('button', { name: 'Kommentieren' }));
    expect(addDbComment).toHaveBeenCalledWith('ARC-0001', 'Hallo');
    expect(await screen.findByRole('tab', { name: 'Kommentare (1)' })).toBeTruthy();
    expect(screen.getByText('Hallo')).toBeTruthy();
    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Kommentar hinzufügen' })).toBeNull());
    await userEvent.click(screen.getByRole('button', { name: 'Kommentar hinzufügen' }));
    expect((screen.getByLabelText('Neuer Kommentar') as HTMLTextAreaElement).value).toBe('');
    await userEvent.keyboard('{Escape}');
    await userEvent.click(screen.getByRole('tab', { name: 'Verlauf' }));
    expect((await screen.findByText('Backlog → Geplant')).closest('li')?.textContent).toContain('claude');
  });

  it('fragt vor dem Schließen nach, wenn ein Inhalt ungespeichert ist', async () => {
    const { onClose } = openPanel();
    await userEvent.click(await screen.findByRole('button', { name: 'Bearbeiten' }));
    await userEvent.type(screen.getByLabelText('Beschreibung'), 'Neu');
    await userEvent.click(screen.getByRole('button', { name: 'Schließen' }));
    expect(await screen.findByRole('alertdialog', { name: 'Ungespeicherte Änderungen' })).toBeTruthy();
    await userEvent.click(screen.getByRole('button', { name: 'Weiter bearbeiten' }));
    expect((screen.getByLabelText('Beschreibung') as HTMLTextAreaElement).value).toBe('Neu');
    expect(onClose).not.toHaveBeenCalled();
  });
});

describe('DbTicketTabs – Schutz der Eingaben', () => {
  it('fragt vor dem Bearbeiten eines anderen Reiters, wenn ein Inhalt ungespeichert ist', async () => {
    openPanel();
    await userEvent.click(await screen.findByRole('button', { name: 'Bearbeiten' }));
    await userEvent.type(screen.getByRole('textbox', { name: 'Beschreibung' }), 'Neu');
    await userEvent.click(screen.getByRole('tab', { name: 'Plan' }));
    await userEvent.click(screen.getByRole('button', { name: 'Bearbeiten' }));
    expect(await screen.findByRole('alertdialog', { name: 'Ungespeicherter Inhalt' })).toBeTruthy();
    expect(screen.queryByRole('textbox', { name: 'Plan' })).toBeNull();
    await userEvent.click(screen.getByRole('button', { name: 'Weiter bearbeiten' }));
    expect((screen.getByRole('textbox', { name: 'Beschreibung' }) as HTMLTextAreaElement).value).toBe('Neu');
    await userEvent.click(screen.getByRole('tab', { name: 'Plan' }));
    await userEvent.click(screen.getByRole('button', { name: 'Bearbeiten' }));
    await userEvent.click(await screen.findByRole('button', { name: 'Verwerfen' }));
    expect(screen.getByRole('textbox', { name: 'Plan' })).toBeTruthy();
  });

  it('speichert bei wiederholtem Strg+Enter nur einmal', async () => {
    let release: () => void = () => {};
    const gate = new Promise<void>((resolve) => (release = resolve));
    const patchDbTicket = vi.fn(async (_id: string, patch: DbTicketPatch) => {
      await gate;
      return { ...DETAIL, ...patch, rev: DETAIL.rev + patchDbTicket.mock.calls.length };
    });
    openPanel({ patchDbTicket });
    await userEvent.click(await screen.findByRole('button', { name: 'Bearbeiten' }));
    const editor = screen.getByRole('textbox', { name: 'Beschreibung' });
    fireEvent.change(editor, { target: { value: 'Neu' } });
    fireEvent.keyDown(editor, { key: 'Enter', ctrlKey: true });
    fireEvent.keyDown(editor, { key: 'Enter', ctrlKey: true });
    release();
    await waitFor(() => expect(screen.queryByRole('textbox', { name: 'Beschreibung' })).toBeNull());
    await new Promise((resolve) => setTimeout(resolve, 20));
    expect(patchDbTicket).toHaveBeenCalledTimes(1);
  });

  it('schickt einen Kommentar bei wiederholtem Strg+Enter nur einmal', async () => {
    let release: () => void = () => {};
    const gate = new Promise<void>((resolve) => (release = resolve));
    const addDbComment = vi.fn(async () => {
      await gate;
      return { ...DETAIL, rev: 4 };
    });
    openPanel({ addDbComment });
    await userEvent.click(await screen.findByRole('tab', { name: 'Kommentare (0)' }));
    await userEvent.click(screen.getByRole('button', { name: 'Kommentar hinzufügen' }));
    const box = screen.getByLabelText('Neuer Kommentar');
    fireEvent.change(box, { target: { value: 'Hallo' } });
    fireEvent.keyDown(box, { key: 'Enter', ctrlKey: true });
    fireEvent.keyDown(box, { key: 'Enter', ctrlKey: true });
    release();
    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Kommentar hinzufügen' })).toBeNull());
    await new Promise((resolve) => setTimeout(resolve, 20));
    expect(addDbComment).toHaveBeenCalledTimes(1);
  });

  it('öffnet den Kommentardialog bei offenem Konflikt nicht', async () => {
    const patchDbTicket = vi.fn().mockRejectedValue(new ApiError('conflict', 'veraltet', 409, { ...DETAIL, title: 'Von jemand anderem', rev: 4 }));
    openPanel({ patchDbTicket });
    await userEvent.click(await screen.findByRole('button', { name: 'Bearbeiten' }));
    await userEvent.type(screen.getByRole('textbox', { name: 'Beschreibung' }), 'Neu');
    await userEvent.keyboard('{Control>}{Enter}{/Control}');
    await screen.findByRole('button', { name: 'Meine Änderung trotzdem speichern' });
    await userEvent.click(screen.getByRole('tab', { name: 'Kommentare (0)' }));
    await userEvent.click(screen.getByRole('button', { name: 'Kommentar hinzufügen' }));
    expect(screen.getByText('Erst den Konflikt auflösen.')).toBeTruthy();
    expect(screen.queryByRole('dialog', { name: 'Kommentar hinzufügen' })).toBeNull();
  });

  it('schließt den Editor, wenn „Meine Änderung trotzdem speichern“ den Inhalt gespeichert hat', async () => {
    const current = { ...DETAIL, title: 'Von jemand anderem', rev: 4 };
    const patchDbTicket = vi.fn()
      .mockRejectedValueOnce(new ApiError('conflict', 'veraltet', 409, current))
      .mockImplementation(async (_id: string, patch: DbTicketPatch) => ({ ...current, ...patch, rev: 5 }));
    openPanel({ patchDbTicket });
    await userEvent.click(await screen.findByRole('button', { name: 'Bearbeiten' }));
    await userEvent.type(screen.getByRole('textbox', { name: 'Beschreibung' }), 'Neu');
    await userEvent.keyboard('{Control>}{Enter}{/Control}');
    await userEvent.click(await screen.findByRole('button', { name: 'Meine Änderung trotzdem speichern' }));
    expect(patchDbTicket).toHaveBeenLastCalledWith('ARC-0001', { description: 'Neu', rev: 4 });
    await waitFor(() => expect(screen.queryByRole('textbox', { name: 'Beschreibung' })).toBeNull());
    expect(screen.getByText('Neu')).toBeTruthy();
  });
});

describe('DbTicketTabs – Kommentare und Kommentarfeld im Verlauf', () => {
  const comment = (id: number, body: string, at: string) => ({ id, ticketId: 'ARC-0001', authorId: 'u1', authorName: 'Florian', body, createdAt: at });

  it('zeigt Kommentare neueste oben und den Knopf darüber', async () => {
    openPanel({}, { ...DETAIL, comments: [comment(1, 'Erster', '2026-10-13T10:00:00.000Z'), comment(2, 'Zweiter', '2026-10-13T11:00:00.000Z')] });
    await userEvent.click(await screen.findByRole('tab', { name: 'Kommentare (2)' }));
    const panel = screen.getByRole('tabpanel');
    const items = within(panel).getAllByRole('listitem').map((li) => li.textContent ?? '');
    expect(items[0]).toContain('Zweiter');
    expect(items[1]).toContain('Erster');
    expect(within(panel).queryByLabelText('Neuer Kommentar')).toBeNull();
    const add = within(panel).getByRole('button', { name: 'Kommentar hinzufügen' });
    expect(add.compareDocumentPosition(within(panel).getAllByRole('listitem')[0]) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  });

  it('klappt lange Kommentare ein und auf', async () => {
    openPanel({}, { ...DETAIL, comments: [comment(1, `Anfang ${'lang '.repeat(100)}`, '2026-10-13T10:00:00.000Z')] });
    await userEvent.click(await screen.findByRole('tab', { name: 'Kommentare (1)' }));
    const toggle = screen.getByRole('button', { name: 'Mehr anzeigen' });
    expect(toggle.getAttribute('aria-expanded')).toBe('false');
    await userEvent.click(toggle);
    expect(screen.getByRole('button', { name: 'Weniger anzeigen' }).getAttribute('aria-expanded')).toBe('true');
  });

  it('schreibt aus dem Verlauf einen Kommentar über den Knopf in der Leiste', async () => {
    const addDbComment = vi.fn(async (_id: string, body: string) => ({ ...DETAIL, rev: 4, comments: [comment(1, body, '2026-10-13T11:00:00.000Z')] }));
    const getDbTicketTimeline = vi.fn(async () => timelineFrom([]));
    openPanel({ addDbComment, getDbTicketTimeline });
    await userEvent.click(await screen.findByRole('tab', { name: 'Verlauf' }));
    await userEvent.click(await screen.findByRole('button', { name: 'Kommentar hinzufügen' }));
    await userEvent.type(screen.getByLabelText('Neuer Kommentar'), 'Aus dem Verlauf');
    await userEvent.keyboard('{Control>}{Enter}{/Control}');
    expect(addDbComment).toHaveBeenCalledWith('ARC-0001', 'Aus dem Verlauf');
    await waitFor(() => expect(getDbTicketTimeline).toHaveBeenCalledTimes(2));
    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Kommentar hinzufügen' })).toBeNull());
  });

  it('behält den Entwurf, wenn der Dialog mit Esc geschlossen wird', async () => {
    openPanel();
    await userEvent.click(await screen.findByRole('tab', { name: 'Kommentare (0)' }));
    await userEvent.click(screen.getByRole('button', { name: 'Kommentar hinzufügen' }));
    await userEvent.type(screen.getByLabelText('Neuer Kommentar'), 'Entwurf');
    await userEvent.keyboard('{Escape}');
    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Kommentar hinzufügen' })).toBeNull());
    await userEvent.click(screen.getByRole('button', { name: 'Kommentar hinzufügen' }));
    expect((screen.getByLabelText('Neuer Kommentar') as HTMLTextAreaElement).value).toBe('Entwurf');
    await userEvent.keyboard('{Escape}');
    // der Entwurf zählt als ungespeicherte Eingabe
    await userEvent.click(screen.getByRole('button', { name: 'Schließen' }));
    expect(await screen.findByRole('alertdialog', { name: 'Ungespeicherte Änderungen' })).toBeTruthy();
  });
});
