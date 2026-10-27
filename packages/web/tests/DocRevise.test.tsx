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

afterEach(cleanup);

const PATH = 'docs/superpowers/specs/2026-10-22-x-design.md';
const CONTENT = '# Spec\n\n## Ziel\n\nEin Ticket wird angelegt.\n\n## Weiteres\n\nNoch mehr Text.\n';
const SECTIONS: MarkdownSectionDto[] = [
  { heading: 'Ziel', level: 2, startLine: 3, endLine: 6 },
  { heading: 'Weiteres', level: 2, startLine: 7, endLine: 9 },
];
const specDoc = { path: PATH, title: '001-x · spec.md', group: 'spec' as const, kind: 'spec' as const, topic: null, date: null, mtimeMs: 1, exists: true };
const REVISED = { original: 'Ein Ticket wird angelegt.', proposal: 'Ein Ticket entsteht.\nZusätzlich mit Hinweis.', startLine: 5, endLine: 5, hash: 'h1' };

// Rendert die Docs-Seite mit einem Spec-Dokument; die Doppel aus den Überschreibungen gewinnen
function open(overrides: Record<string, unknown> = {}) {
  const reviseDoc = vi.fn(async () => REVISED);
  const writeDocSection = vi.fn(async () => ({ path: PATH, hash: 'h2', sections: SECTIONS }));
  const getDoc = vi.fn(async (_p: string, p: string) => ({ path: p, content: CONTENT, hash: 'h1', sections: SECTIONS }));
  const api = fakeApi({
    getDocs: async () => ({ source: { root: 'C:/repo', boardId: 1, boardName: 'Sprint', reason: null }, docs: [specDoc] }),
    getDoc,
    getDocComments: async () => [],
    reviseDoc,
    writeDocSection,
    ...overrides,
  });
  renderWithApp(<DocsPage />, { api, path: `/p/p1/docs/${PATH}`, route: '/p/:pid/docs/*' });
  return { reviseDoc: (overrides.reviseDoc ?? reviseDoc) as ReturnType<typeof vi.fn>, writeDocSection: (overrides.writeDocSection ?? writeDocSection) as ReturnType<typeof vi.fn>, getDoc };
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

// Markiert und öffnet den Dialog „Mit Claude ändern“
async function openDialog() {
  await screen.findByRole('heading', { name: 'Ziel' });
  selectTicket();
  await userEvent.click(await screen.findByRole('button', { name: 'Mit Claude ändern' }));
  return within(await screen.findByRole('dialog', { name: 'Mit Claude ändern' }));
}

// Tippt die Anweisung und schickt sie ab
async function ask(dialog: ReturnType<typeof within>, instruction = 'Kürzer schreiben') {
  await userEvent.type(dialog.getByRole('textbox', { name: 'Änderungsanweisung' }), instruction);
  await userEvent.click(dialog.getByRole('button', { name: 'Änderung vorschlagen' }));
}

describe('Mit Claude ändern', () => {
  it('schickt Bereich und Anweisung an revise', async () => {
    const { reviseDoc } = open();
    const dialog = await openDialog();
    await ask(dialog);
    expect(reviseDoc).toHaveBeenCalledWith('p1', {
      path: PATH, startLine: 5, endLine: 5, instruction: 'Kürzer schreiben', expectedHash: 'h1',
    }, undefined);
  });

  it('zeigt während des Laufs den Ladehinweis', async () => {
    open({ reviseDoc: vi.fn(() => new Promise(() => undefined)) });
    const dialog = await openDialog();
    await ask(dialog);
    expect(await dialog.findByText(/bis zu 5 Minuten/)).toBeTruthy();
  });

  it('zeigt im Diff entfernte und hinzugefügte Zeilen', async () => {
    open();
    const dialog = await openDialog();
    await ask(dialog);
    await dialog.findByRole('textbox', { name: 'Vorschlag' });
    expect(document.querySelectorAll('.diff-remove').length).toBe(1);
    expect(document.querySelectorAll('.diff-add').length).toBe(2);
  });

  it('übernimmt den geänderten Vorschlag mit dem Hash aus revise und lädt neu', async () => {
    const { writeDocSection, getDoc } = open();
    const dialog = await openDialog();
    await ask(dialog);
    const box = await dialog.findByRole('textbox', { name: 'Vorschlag' }) as HTMLTextAreaElement;
    expect(box.value).toBe(REVISED.proposal);
    await userEvent.clear(box);
    await userEvent.type(box, 'Von Hand nachgebessert.');
    const before = getDoc.mock.calls.length;
    await userEvent.click(dialog.getByRole('button', { name: 'Übernehmen' }));
    expect(writeDocSection).toHaveBeenCalledWith('p1', {
      path: PATH, startLine: 5, endLine: 5, content: 'Von Hand nachgebessert.', expectedHash: 'h1',
    }, undefined);
    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Mit Claude ändern' })).toBeNull());
    await waitFor(() => expect(getDoc.mock.calls.length).toBeGreaterThan(before));
  });

  it('schreibt beim Verwerfen nichts', async () => {
    const { writeDocSection } = open();
    const dialog = await openDialog();
    await ask(dialog);
    await dialog.findByRole('textbox', { name: 'Vorschlag' });
    await userEvent.click(dialog.getByRole('button', { name: 'Verwerfen' }));
    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Mit Claude ändern' })).toBeNull());
    expect(writeDocSection).not.toHaveBeenCalled();
  });

  it('zeigt den Fehler von revise und schreibt nichts', async () => {
    const { writeDocSection } = open({
      reviseDoc: vi.fn(async () => { throw new ApiError('analysis-failed', 'Claude konnte den Bereich nicht umschreiben.', 502); }),
    });
    const dialog = await openDialog();
    await ask(dialog);
    expect((await dialog.findByRole('alert')).textContent).toContain('Claude konnte den Bereich nicht umschreiben.');
    expect(writeDocSection).not.toHaveBeenCalled();
    expect((dialog.getByRole('textbox', { name: 'Änderungsanweisung' }) as HTMLTextAreaElement).value).toBe('Kürzer schreiben');
  });

  it('meldet den Konflikt beim Übernehmen und behält den Vorschlag', async () => {
    open({ writeDocSection: vi.fn(async () => { throw new ApiError('doc-changed', 'Das Dokument wurde zwischenzeitlich geändert.', 409); }) });
    const dialog = await openDialog();
    await ask(dialog);
    const box = await dialog.findByRole('textbox', { name: 'Vorschlag' }) as HTMLTextAreaElement;
    await userEvent.click(dialog.getByRole('button', { name: 'Übernehmen' }));
    expect((await dialog.findByRole('alert')).textContent).toContain('zwischenzeitlich geändert');
    expect(box.value).toBe(REVISED.proposal);
  });
});
