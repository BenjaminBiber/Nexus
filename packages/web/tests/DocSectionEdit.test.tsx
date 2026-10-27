// @vitest-environment jsdom
import { describe, it, expect, afterEach, vi } from 'vitest';
import { cleanup, fireEvent, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { ApiError } from '../src/api/ApiError.js';
import type { MarkdownSectionDto } from '../src/api/types.js';
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
const CONTENT = '# Spec\n\n## Ziel\n\nEin Ticket wird angelegt.\n\n## Weiteres\n\nNoch mehr Text.\n';
const SECTIONS: MarkdownSectionDto[] = [
  { heading: 'Ziel', level: 2, startLine: 3, endLine: 6 },
  { heading: 'Weiteres', level: 2, startLine: 7, endLine: 9 },
];
const specDoc = { path: PATH, title: '001-x · spec.md', group: 'spec' as const, kind: 'spec' as const, topic: null, date: null, mtimeMs: 1, exists: true };
const GLOSSARY = 'glossar.md';
const glossaryDoc = { ...specDoc, path: GLOSSARY, title: 'Glossar', group: 'glossary' as const, kind: null };

// Rendert die Docs-Seite mit einem Dokument samt Hash und Abschnitten
function open(overrides: Record<string, unknown> = {}, path = PATH, docs: unknown[] = [specDoc]) {
  const writeDocSection = vi.fn(async () => ({ path, hash: 'h2', sections: SECTIONS }));
  const getDoc = vi.fn(async (_p: string, p: string) => ({ path: p, content: CONTENT, hash: 'h1', sections: SECTIONS }));
  const api = fakeApi({
    getDocs: async () => ({ source: { root: 'C:/repo', boardId: 1, boardName: 'Sprint', reason: null }, docs }),
    getDoc,
    getDocComments: async () => [],
    writeDocSection,
    ...overrides,
  });
  renderWithApp(<DocsPage />, { api, path: `/p/p1/docs/${path}`, route: '/p/:pid/docs/*' });
  return { writeDocSection, getDoc };
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

// Markiert und öffnet den Abschnittseditor über „Bearbeiten“
async function edit() {
  selectTicket();
  await userEvent.click(await screen.findByRole('button', { name: 'Bearbeiten' }));
  return within(await screen.findByRole('dialog', { name: 'Abschnitt bearbeiten' }));
}

describe('Abschnitt im Dokument bearbeiten', () => {
  it('speichert die markierten Zeilen mit Bereich und Hash und zeigt den neuen Inhalt', async () => {
    const { writeDocSection, getDoc } = open();
    await screen.findByRole('heading', { name: 'Ziel' });
    const dialog = await edit();
    const box = dialog.getByRole('textbox', { name: 'Markdown' }) as HTMLTextAreaElement;
    expect(box.value).toBe('Ein Ticket wird angelegt.');
    await userEvent.clear(box);
    await userEvent.type(box, 'Ein Ticket entsteht.');
    getDoc.mockImplementation(async (_p: string, p: string) => ({
      path: p, content: '# Spec\n\n## Ziel\n\nEin Ticket entsteht.\n\n## Weiteres\n\nNoch mehr Text.\n', hash: 'h2', sections: SECTIONS,
    }));
    await userEvent.click(dialog.getByRole('button', { name: 'Speichern' }));
    expect(writeDocSection).toHaveBeenCalledWith('p1', {
      path: PATH, startLine: 5, endLine: 5, content: 'Ein Ticket entsteht.', expectedHash: 'h1',
    }, undefined);
    expect(await screen.findByText(/Ein Ticket entsteht/)).toBeTruthy();
    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Abschnitt bearbeiten' })).toBeNull());
  });

  it('meldet den Konflikt bei 409 und behält den getippten Text', async () => {
    open({ writeDocSection: vi.fn(async () => { throw new ApiError('doc-changed', 'Das Dokument wurde zwischenzeitlich geändert.', 409); }) });
    await screen.findByRole('heading', { name: 'Ziel' });
    const dialog = await edit();
    const box = dialog.getByRole('textbox', { name: 'Markdown' }) as HTMLTextAreaElement;
    await userEvent.clear(box);
    await userEvent.type(box, 'Mein eigener Text');
    await userEvent.click(dialog.getByRole('button', { name: 'Speichern' }));
    expect(await dialog.findByRole('alert')).toBeTruthy();
    expect(box.value).toBe('Mein eigener Text');
    expect(dialog.getByRole('button', { name: 'Dokument neu laden' })).toBeTruthy();
    expect((dialog.getByRole('button', { name: 'Speichern' }) as HTMLButtonElement).disabled).toBe(true);
  });

  it('bearbeitet über die Überschrift den ganzen Abschnitt', async () => {
    const { writeDocSection } = open();
    const heading = await screen.findByRole('heading', { name: 'Ziel' });
    fireEvent.mouseOver(heading);
    await userEvent.click(await screen.findByRole('button', { name: 'Abschnitt bearbeiten' }));
    const dialog = within(await screen.findByRole('dialog', { name: 'Abschnitt bearbeiten' }));
    const box = dialog.getByRole('textbox', { name: 'Markdown' }) as HTMLTextAreaElement;
    expect(box.value).toBe('## Ziel\n\nEin Ticket wird angelegt.\n');
    await userEvent.click(dialog.getByRole('button', { name: 'Speichern' }));
    expect(writeDocSection).toHaveBeenCalledWith('p1', {
      path: PATH, startLine: 3, endLine: 6, content: '## Ziel\n\nEin Ticket wird angelegt.\n', expectedHash: 'h1',
    }, undefined);
  });

  it('zeigt im Popover Bearbeiten, Mit Claude ändern und Kommentieren', async () => {
    open();
    await screen.findByRole('heading', { name: 'Ziel' });
    selectTicket();
    const group = within(await screen.findByRole('group', { name: 'Aktionen zur Auswahl' }));
    expect(group.getByRole('button', { name: 'Bearbeiten' })).toBeTruthy();
    expect(group.getByRole('button', { name: 'Kommentieren' })).toBeTruthy();
    expect((group.getByRole('button', { name: 'Mit Claude ändern' }) as HTMLButtonElement).disabled).toBe(false);
    expect(group.queryByRole('button', { name: 'Zur Ticket-Auswahl' })).toBeNull();
  });

  it('lässt Glossar-Dokumente nur lesbar', async () => {
    open({}, GLOSSARY, [glossaryDoc]);
    await screen.findByRole('heading', { name: 'Ziel' });
    expect(screen.getByText(/Dokument nur lesbar/)).toBeTruthy();
    selectTicket();
    const group = within(await screen.findByRole('group', { name: 'Aktionen zur Auswahl' }));
    expect(group.queryByRole('button', { name: 'Bearbeiten' })).toBeNull();
    expect(group.queryByRole('button', { name: 'Mit Claude ändern' })).toBeNull();
  });
});
