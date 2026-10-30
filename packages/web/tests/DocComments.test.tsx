// @vitest-environment jsdom
import { describe, it, expect, afterEach, vi } from 'vitest';
import { cleanup, fireEvent, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import type { DocCommentDto } from '../src/api/types.js';
import { DocsPage } from '../src/pages/DocsPage.js';
import { fakeApi } from './helpers/fakeApi.js';
import { renderWithApp } from './helpers/renderWithApp.js';

vi.mock('mermaid', () => ({ default: { initialize: vi.fn(), render: vi.fn(async () => ({ svg: '<svg></svg>' })) } }));
const originalScroll = Element.prototype.scrollIntoView;

// Räumt die Darstellung auf und stellt scrollIntoView wieder her
afterEach(() => {
  cleanup();
  Element.prototype.scrollIntoView = originalScroll;
});

const PATH = 'docs/superpowers/specs/2026-10-22-x-design.md';
const docs = [{ path: PATH, title: '001-x · spec.md', group: 'spec' as const, kind: 'spec' as const, topic: null, date: null, mtimeMs: 1, exists: true }];
// Antwort von getDocs: Quelle mit Repo-Pfad plus Liste
const listing = (list = docs) => ({ source: { root: 'C:/repo', boardId: 1, boardName: 'Sprint', reason: null }, docs: list });
const existing: DocCommentDto = {
  id: 'K-aaaaaaaa', status: 'open', text: 'Bitte präzisieren', createdAt: '2026-10-10T10:00:00Z',
  quote: 'wird angelegt', heading: 'Ziel', lines: [5, 5], resolution: null,
};

// Rendert die Docs-Seite mit Kommentar-Mocks
function open(comments: DocCommentDto[] = [], query = '', overrides: Record<string, unknown> = {}) {
  const addDocComment = vi.fn(async (_p: string, _path: string, input: object) => ({ ...existing, id: 'K-bbbbbbbb', ...input }));
  const patchDocComment = vi.fn(async (_p: string, _path: string, id: string, patch: object) => ({ ...existing, id, ...patch }));
  const deleteDocComment = vi.fn(async () => undefined);
  const api = fakeApi({
    getDocs: async () => listing(),
    getDoc: async (_p: string, path: string) => ({ path, content: '# Spec\n\n## Ziel\n\nEin Ticket wird angelegt.\n' }),
    getDocComments: async () => comments,
    addDocComment,
    patchDocComment,
    deleteDocComment,
    ...overrides,
  });
  renderWithApp(<DocsPage />, { api, path: `/p/p1/docs/${PATH}${query}`, route: '/p/:pid/docs/*' });
  return { addDocComment, patchDocComment, deleteDocComment };
}

// Markiert einen Bereich des Absatzes wie mit der Maus
function selectRange(from: number, to: number) {
  const text = screen.getByText(/Ein Ticket wird angelegt/).firstChild!;
  const range = document.createRange();
  range.setStart(text, from);
  range.setEnd(text, to);
  window.getSelection()!.removeAllRanges();
  window.getSelection()!.addRange(range);
  fireEvent.mouseUp(document.querySelector('article.doc')!);
}

// Markiert den Text „Ticket“ im gerenderten Absatz wie mit der Maus
function selectTicket() {
  const text = screen.getByText(/Ein Ticket wird angelegt/).firstChild!;
  const range = document.createRange();
  range.setStart(text, 4);
  range.setEnd(text, 10);
  window.getSelection()!.removeAllRanges();
  window.getSelection()!.addRange(range);
  fireEvent.mouseUp(document.querySelector('article.doc')!);
}

// Wartet auf die Hervorhebung im Dokument; der Kommentartext steht oft schon da, bevor sie gesetzt ist
async function highlight() {
  return waitFor(() => {
    const mark = document.querySelector('mark.doc-comment');
    expect(mark).not.toBeNull();
    return mark!;
  });
}

// Öffnet das Popover über den Knopf an der Auswahl
async function compose() {
  await userEvent.click(await screen.findByRole('button', { name: 'Kommentieren' }));
  return within(screen.getByRole('dialog', { name: 'Kommentar zur Stelle' }));
}

describe('Kommentare im Dokument', () => {
  it('legt zur markierten Stelle einen Kommentar an', async () => {
    const { addDocComment } = open();
    await screen.findByRole('heading', { name: 'Ziel' });
    selectTicket();
    const popover = await compose();
    expect(popover.getByText('Ticket')).toBeTruthy();
    await userEvent.type(popover.getByRole('textbox', { name: 'Kommentar' }), 'Was ist ein Ticket?');
    await userEvent.click(popover.getByRole('button', { name: 'Speichern' }));
    expect(addDocComment).toHaveBeenCalledWith('p1', PATH, {
      text: 'Was ist ein Ticket?', quote: 'Ticket', heading: 'Ziel', lines: [5, 5],
    }, undefined);
    expect(screen.queryByRole('dialog')).toBeNull();
  });

  it('hebt offene Kommentare hervor, listet sie und erledigt sie', async () => {
    const { patchDocComment } = open([existing]);
    const panel = within(await screen.findByRole('complementary', { name: 'Kommentare' }));
    expect(await panel.findByText('Bitte präzisieren')).toBeTruthy();
    expect(document.querySelector('mark.doc-comment')?.textContent).toBe('wird angelegt');
    await userEvent.click(panel.getByRole('button', { name: 'Erledigt' }));
    expect(patchDocComment).toHaveBeenCalledWith('p1', PATH, 'K-aaaaaaaa', { status: 'resolved' }, undefined);
  });

  it('nennt im Kopf die Anzahl offener Kommentare', async () => {
    open([existing, { ...existing, id: 'K-cccccccc' }, { ...existing, id: 'K-dddddddd', status: 'resolved' }]);
    await screen.findByRole('complementary', { name: 'Kommentare' });
    expect(document.querySelector('.doc-path')!.textContent).toContain('· 2 offene Kommentare');
  });

  it('nennt einen offenen Kommentar in der Einzahl und bei keinem nichts', async () => {
    open([existing]);
    await screen.findByRole('complementary', { name: 'Kommentare' });
    expect(document.querySelector('.doc-path')!.textContent).toContain('· 1 offener Kommentar');
    expect(document.querySelector('.doc-path')!.textContent).not.toContain('Kommentare');
    cleanup();
    open([]);
    await screen.findByRole('heading', { name: 'Ziel' });
    expect(document.querySelector('.doc-path')!.textContent).not.toMatch(/offene/);
  });

  it('meldet, wenn die Stelle im Dokument nicht mehr vorkommt', async () => {
    open([{ ...existing, quote: 'gibt es nicht mehr' }]);
    const panel = within(await screen.findByRole('complementary', { name: 'Kommentare' }));
    expect(await panel.findByText('Stelle nicht mehr gefunden')).toBeTruthy();
  });

  it('hebt erledigte Kommentare nicht hervor', async () => {
    open([{ ...existing, status: 'resolved' }]);
    await screen.findByRole('heading', { name: 'Ziel' });
    const panel = within(await screen.findByRole('complementary', { name: 'Kommentare' }));
    await userEvent.click(panel.getByRole('button', { name: /Erledigt \(1\)/ }));
    expect(await panel.findByText('Bitte präzisieren')).toBeTruthy();
    expect(document.querySelector('mark.doc-comment')).toBeNull();
  });

  it('speichert einen bearbeiteten Text über patchDocComment', async () => {
    const { patchDocComment } = open([existing]);
    const panel = within(await screen.findByRole('complementary', { name: 'Kommentare' }));
    await userEvent.click(await panel.findByRole('button', { name: 'Bearbeiten' }));
    const field = panel.getByRole('textbox', { name: 'Kommentar bearbeiten' });
    await userEvent.clear(field);
    await userEvent.type(field, 'Neu formuliert');
    await userEvent.click(panel.getByRole('button', { name: 'Speichern' }));
    expect(patchDocComment).toHaveBeenCalledWith('p1', PATH, 'K-aaaaaaaa', { text: 'Neu formuliert' }, undefined);
  });

  it('löscht einen Kommentar über deleteDocComment', async () => {
    const { deleteDocComment } = open([existing]);
    const panel = within(await screen.findByRole('complementary', { name: 'Kommentare' }));
    await userEvent.click(await panel.findByRole('button', { name: 'Löschen' }));
    expect(deleteDocComment).toHaveBeenCalledWith('p1', PATH, 'K-aaaaaaaa', undefined);
  });

  it('hebt den Kommentar auch bei gleichzeitigem Suchbegriff hervor', async () => {
    open([existing], '?q=angelegt');
    const panel = within(await screen.findByRole('complementary', { name: 'Kommentare' }));
    expect(await panel.findByText('Bitte präzisieren')).toBeTruthy();
    const marks = Array.from(document.querySelectorAll('mark.doc-comment'));
    expect(marks.map((m) => m.textContent).join('')).toBe('wird angelegt');
    expect(panel.queryByText('Stelle nicht mehr gefunden')).toBeNull();
    expect(document.querySelector('article.doc mark:not(.doc-comment)')?.textContent).toBe('angelegt');
  });

  it('behält den Entwurf samt Text, wenn das Speichern fehlschlägt', async () => {
    open([], '', { addDocComment: vi.fn(async () => { throw new Error('Speichern fehlgeschlagen'); }) });
    await screen.findByRole('heading', { name: 'Ziel' });
    selectTicket();
    const popover = await compose();
    await userEvent.type(popover.getByRole('textbox', { name: 'Kommentar' }), 'Bleibt stehen');
    await userEvent.click(popover.getByRole('button', { name: 'Speichern' }));
    expect((await popover.findByRole('alert')).textContent).toBe('Speichern fehlgeschlagen');
    expect((popover.getByRole('textbox', { name: 'Kommentar' }) as HTMLTextAreaElement).value).toBe('Bleibt stehen');
  });

  it('wirft bei einer Kommentar-ID mit Sonderzeichen nicht', async () => {
    const scroll = vi.fn();
    Element.prototype.scrollIntoView = scroll;
    open([{ ...existing, id: 'K-"x]' }]);
    const panel = within(await screen.findByRole('complementary', { name: 'Kommentare' }));
    await userEvent.click(await panel.findByRole('button', { name: /wird angelegt/ }));
    expect(scroll.mock.contexts.at(-1)).toBe(document.querySelector('mark.doc-comment'));
  });

  it('fragt bei fehlender Datei keine Kommentare ab', async () => {
    const getDocComments = vi.fn(async () => []);
    const api = fakeApi({ getDocs: async () => listing([{ ...docs[0], exists: false }]), getDocComments });
    renderWithApp(<DocsPage />, { api, path: `/p/p1/docs/${PATH}`, route: '/p/:pid/docs/*' });
    await screen.findByText('Diese Datei existiert nicht mehr.');
    expect(getDocComments).not.toHaveBeenCalled();
  });

  it('übernimmt eine Tastaturauswahl als Entwurf', async () => {
    open();
    await screen.findByRole('heading', { name: 'Ziel' });
    const text = screen.getByText(/Ein Ticket wird angelegt/).firstChild!;
    const range = document.createRange();
    range.setStart(text, 4);
    range.setEnd(text, 10);
    window.getSelection()!.removeAllRanges();
    window.getSelection()!.addRange(range);
    fireEvent.keyUp(document.querySelector('article.doc')!);
    expect(await screen.findByRole('button', { name: 'Kommentieren' })).toBeTruthy();
  });

  it('erklärt einen leeren Reiter', async () => {
    open([existing]);
    const panel = within(await screen.findByRole('complementary', { name: 'Kommentare' }));
    await userEvent.click(await panel.findByRole('button', { name: /Erledigt \(0\)/ }));
    expect(panel.getByText('Keine erledigten Kommentare.')).toBeTruthy();
  });

  it('sendet beim Bearbeiten getrimmt und sperrt leeren Text', async () => {
    const { patchDocComment } = open([existing]);
    const panel = within(await screen.findByRole('complementary', { name: 'Kommentare' }));
    await userEvent.click(await panel.findByRole('button', { name: 'Bearbeiten' }));
    const field = panel.getByRole('textbox', { name: 'Kommentar bearbeiten' });
    await userEvent.clear(field);
    expect((panel.getByRole('button', { name: 'Speichern' }) as HTMLButtonElement).disabled).toBe(true);
    await userEvent.type(field, '  Mit Rand  ');
    await userEvent.click(panel.getByRole('button', { name: 'Speichern' }));
    expect(patchDocComment).toHaveBeenCalledWith('p1', PATH, 'K-aaaaaaaa', { text: 'Mit Rand' }, undefined);
  });

  it('speichert bei schnellem Doppelklick nur einmal und gibt den Knopf nach einem Fehler frei', async () => {
    let fail!: (e: Error) => void;
    const addDocComment = vi.fn(() => new Promise<never>((_, reject) => { fail = reject; }));
    open([], '', { addDocComment });
    await screen.findByRole('heading', { name: 'Ziel' });
    selectTicket();
    const popover = await compose();
    await userEvent.type(popover.getByRole('textbox', { name: 'Kommentar' }), 'Einmal');
    const save = popover.getByRole('button', { name: 'Speichern' }) as HTMLButtonElement;
    fireEvent.click(save);
    fireEvent.click(save);
    expect(addDocComment).toHaveBeenCalledTimes(1);
    fail(new Error('Speichern fehlgeschlagen'));
    await popover.findByRole('alert');
    expect((popover.getByRole('button', { name: 'Speichern' }) as HTMLButtonElement).disabled).toBe(false);
    expect((popover.getByRole('textbox', { name: 'Kommentar' }) as HTMLTextAreaElement).value).toBe('Einmal');
  });

  it('zeigt nach dem Markieren nur den Knopf, erst der Klick das Textfeld', async () => {
    open();
    await screen.findByRole('heading', { name: 'Ziel' });
    selectTicket();
    await screen.findByRole('button', { name: 'Kommentieren' });
    expect(screen.queryByRole('textbox', { name: 'Kommentar' })).toBeNull();
    await compose();
    expect(screen.getByRole('textbox', { name: 'Kommentar' })).toBeTruthy();
  });

  it('behält getippten Text, wenn währenddessen eine andere Stelle markiert wird', async () => {
    open();
    await screen.findByRole('heading', { name: 'Ziel' });
    selectTicket();
    const popover = await compose();
    await userEvent.type(popover.getByRole('textbox', { name: 'Kommentar' }), 'Text');
    selectRange(15, 23);
    expect((screen.getByRole('textbox', { name: 'Kommentar' }) as HTMLTextAreaElement).value).toBe('Text');
    expect(screen.getByRole('dialog')).toBeTruthy();
  });

  it('übernimmt eine neue Markierung, wenn das Textfeld leer ist', async () => {
    open();
    await screen.findByRole('heading', { name: 'Ziel' });
    selectTicket();
    await compose();
    fireEvent.mouseDown(document.querySelector('article.doc')!);
    selectRange(15, 23);
    expect(await screen.findByRole('button', { name: 'Kommentieren' })).toBeTruthy();
    expect(screen.queryByRole('dialog')).toBeNull();
  });

  it('erlaubt nach dem Speichern eine neue Markierung', async () => {
    open();
    await screen.findByRole('heading', { name: 'Ziel' });
    selectTicket();
    const popover = await compose();
    await userEvent.type(popover.getByRole('textbox', { name: 'Kommentar' }), 'Text');
    await userEvent.click(popover.getByRole('button', { name: 'Speichern' }));
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
    selectRange(15, 23);
    expect(await screen.findByRole('button', { name: 'Kommentieren' })).toBeTruthy();
  });

  it('verwirft den Entwurf mit Escape', async () => {
    open();
    await screen.findByRole('heading', { name: 'Ziel' });
    selectTicket();
    const popover = await compose();
    await userEvent.type(popover.getByRole('textbox', { name: 'Kommentar' }), 'x{Escape}');
    expect(screen.queryByRole('dialog')).toBeNull();
  });

  it('schließt bei Klick daneben nur, solange nichts getippt ist', async () => {
    open();
    await screen.findByRole('heading', { name: 'Ziel' });
    selectTicket();
    const popover = await compose();
    await userEvent.type(popover.getByRole('textbox', { name: 'Kommentar' }), 'Text');
    fireEvent.mouseDown(document.body);
    expect(screen.getByRole('dialog')).toBeTruthy();
    await userEvent.clear(popover.getByRole('textbox', { name: 'Kommentar' }));
    fireEvent.mouseDown(document.body);
    expect(screen.queryByRole('dialog')).toBeNull();
  });

  it('zeigt den Kommentar beim Überfahren der Hervorhebung', async () => {
    open([existing]);
    await screen.findByText('Bitte präzisieren');
    fireEvent.mouseOver(await highlight());
    const card = within(await screen.findByRole('dialog', { name: 'Kommentar' }));
    expect(card.getByText('Bitte präzisieren')).toBeTruthy();
    expect(card.queryByRole('textbox')).toBeNull();
  });

  it('öffnet den Kommentar per Klick auf die Hervorhebung zum Bearbeiten und speichert ihn', async () => {
    const { patchDocComment } = open([existing]);
    await screen.findByText('Bitte präzisieren');
    fireEvent.click(await highlight());
    const card = within(await screen.findByRole('dialog', { name: 'Kommentar' }));
    const field = card.getByRole('textbox', { name: 'Kommentar bearbeiten' });
    await userEvent.clear(field);
    await userEvent.type(field, 'Neu gefasst');
    await userEvent.click(card.getByRole('button', { name: 'Speichern' }));
    expect(patchDocComment).toHaveBeenCalledWith('p1', PATH, 'K-aaaaaaaa', { text: 'Neu gefasst' }, undefined);
  });

  it('wechselt im Hover über den Stift in die Bearbeitung und bricht mit Escape ab', async () => {
    open([existing]);
    await screen.findByText('Bitte präzisieren');
    fireEvent.mouseOver(await highlight());
    await userEvent.click(await screen.findByRole('button', { name: 'Kommentar bearbeiten' }));
    await userEvent.type(screen.getByRole('textbox', { name: 'Kommentar bearbeiten' }), '{Escape}');
    expect(screen.queryByRole('dialog', { name: 'Kommentar' })).toBeNull();
  });
});
