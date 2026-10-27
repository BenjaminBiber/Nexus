// @vitest-environment jsdom
import { describe, it, expect, afterEach, beforeEach, vi } from 'vitest';
import { cleanup, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { ApiError } from '../src/api/ApiError.js';
import type { MarkdownSectionDto, QuestionDraftDto } from '../src/api/types.js';
import { DocsPage } from '../src/pages/DocsPage.js';
import { fakeApi } from './helpers/fakeApi.js';
import { renderWithApp } from './helpers/renderWithApp.js';

vi.mock('mermaid', () => ({ default: { initialize: vi.fn(), render: vi.fn(async () => ({ svg: '<svg></svg>' })) } }));

afterEach(cleanup);
beforeEach(() => window.localStorage.clear());

const PATH = 'docs/superpowers/specs/2026-10-22-x-design.md';
const CONTENT = '# Spec\n\n## Ziel\n\nWelcher Speicher? TBD.\n\n## Weiteres\n\nNoch mehr Text.\n';
const SECTIONS: MarkdownSectionDto[] = [
  { heading: 'Ziel', level: 2, startLine: 3, endLine: 6 },
  { heading: 'Weiteres', level: 2, startLine: 7, endLine: 9 },
];
const specDoc = { path: PATH, title: '001-x · spec.md', group: 'spec' as const, kind: 'spec' as const, topic: null, date: null, mtimeMs: 1, exists: true };

const question = (title: string, heading = 'Ziel'): QuestionDraftDto => ({ title, description: `Kontext ${title}`, source: { heading, startLine: 5, endLine: 5 } });
const DRAFTS = { drafts: [question('Welcher Speicher?'), question('Wie viele Nutzer?', 'Weiteres')], dropped: [], costUsd: 0.05, at: '2026-10-26T08:00:00.000Z' };

// Rendert die Docs-Seite mit einem Spec-Dokument; die Doppel aus den Überschreibungen gewinnen
function open(overrides: Record<string, unknown> = {}) {
  const createQuestionDrafts = vi.fn(async () => DRAFTS);
  const createDocQuestions = vi.fn(async () => ({ created: [{ index: 0, id: 'AIH-F0007', answered: false }], skipped: [] }));
  const cancelQuestionDrafts = vi.fn(async () => ({ cancelled: true }));
  const api = fakeApi({
    getDocs: async () => ({ source: { root: 'C:/repo', boardId: 1, boardName: 'Sprint', reason: null }, docs: [specDoc] }),
    getDoc: async (_p: string, p: string) => ({ path: p, content: CONTENT, hash: 'h1', sections: SECTIONS }),
    getDocComments: async () => [],
    createQuestionDrafts, createDocQuestions, cancelQuestionDrafts,
    ...overrides,
  });
  renderWithApp(<DocsPage />, { api, path: `/p/p1/docs/${PATH}`, route: '/p/:pid/docs/*' });
  return {
    createQuestionDrafts: (overrides.createQuestionDrafts ?? createQuestionDrafts) as ReturnType<typeof vi.fn>,
    createDocQuestions: (overrides.createDocQuestions ?? createDocQuestions) as ReturnType<typeof vi.fn>,
    cancelQuestionDrafts,
  };
}

const sideActions = () => within(document.querySelector('.doc-side-actions') as HTMLElement);

// Lässt die offenen Fragen suchen und liefert den Dialog
async function findQuestions() {
  await screen.findByRole('heading', { name: 'Ziel' });
  await userEvent.click(sideActions().getByRole('button', { name: 'Fragen erzeugen' }));
  return within(await screen.findByRole('dialog', { name: 'Offene Fragen' }));
}

describe('Fragen erzeugen', () => {
  it('steht unter „Tickets erzeugen“ in der Werkzeugspalte', async () => {
    open();
    await screen.findByRole('heading', { name: 'Ziel' });
    const buttons = sideActions().getAllByRole('button').map((b) => b.textContent);
    expect(buttons.findIndex((t) => t?.includes('Fragen erzeugen'))).toBeGreaterThan(buttons.findIndex((t) => t?.includes('Tickets erzeugen')));
  });

  it('legt nur die gewählten und bearbeiteten Fragen an und verlinkt sie', async () => {
    const { createQuestionDrafts, createDocQuestions } = open();
    const dialog = await findQuestions();
    expect(createQuestionDrafts).toHaveBeenCalledWith('p1', PATH, undefined);
    const title = dialog.getByRole('textbox', { name: 'Titel (Frage 1)' });
    await userEvent.clear(title);
    await userEvent.type(title, 'Welcher Speicher passt?');
    await userEvent.click(dialog.getByRole('checkbox', { name: 'Frage 2 anlegen' }));
    await userEvent.click(dialog.getByRole('button', { name: '1 Frage anlegen' }));
    expect(createDocQuestions).toHaveBeenCalledWith('p1', PATH, [{ title: 'Welcher Speicher passt?', description: 'Kontext Welcher Speicher?', source: { heading: 'Ziel', startLine: 5, endLine: 5 } }], undefined);
    const link = await dialog.findByRole('link', { name: 'AIH-F0007' });
    expect(link.getAttribute('href')).toBe('/p/p1/questions/AIH-F0007');
    expect(dialog.getByText(/automatische Recherche/)).toBeTruthy();
  });

  it('schickt eine eingetragene Antwort mit und zeigt die Frage danach als entschieden', async () => {
    const createDocQuestions = vi.fn(async () => ({
      created: [{ index: 0, id: 'AIH-F0007', answered: true }, { index: 1, id: 'AIH-F0008', answered: false }], skipped: [],
    }));
    open({ createDocQuestions });
    const dialog = await findQuestions();
    await userEvent.type(dialog.getByRole('textbox', { name: 'Antwort (Frage 1)' }), '  SQLite lokal  ');
    // Die Kachel zeigt, dass die Frage schon beantwortet ist
    expect(within(dialog.getByRole('list', { name: 'Fragen' })).getAllByText('beantwortet')).toHaveLength(1);
    await userEvent.click(dialog.getByRole('button', { name: '2 Fragen anlegen, 1 davon entschieden' }));
    expect(createDocQuestions).toHaveBeenCalledWith('p1', PATH, [
      { title: 'Welcher Speicher?', description: 'Kontext Welcher Speicher?', source: { heading: 'Ziel', startLine: 5, endLine: 5 }, answer: 'SQLite lokal' },
      { title: 'Wie viele Nutzer?', description: 'Kontext Wie viele Nutzer?', source: { heading: 'Weiteres', startLine: 5, endLine: 5 } },
    ], undefined);
    await dialog.findByRole('link', { name: 'AIH-F0007' });
    expect(dialog.getByText('entschieden')).toBeTruthy();
    expect(dialog.getByText('offen')).toBeTruthy();
    // Noch eine offene Frage: der Hinweis auf die Recherche bleibt
    expect(dialog.getByText(/automatische Recherche/)).toBeTruthy();
  });

  it('nennt die Recherche nicht, wenn alle Fragen gleich entschieden wurden', async () => {
    open({ createDocQuestions: vi.fn(async () => ({ created: [{ index: 0, id: 'AIH-F0007', answered: true }], skipped: [] })) });
    const dialog = await findQuestions();
    await userEvent.click(dialog.getByRole('checkbox', { name: 'Frage 2 anlegen' }));
    await userEvent.type(dialog.getByRole('textbox', { name: 'Antwort (Frage 1)' }), 'SQLite');
    await userEvent.click(dialog.getByRole('button', { name: '1 Frage anlegen, 1 davon entschieden' }));
    await dialog.findByRole('link', { name: 'AIH-F0007' });
    expect(dialog.queryByText(/automatische Recherche/)).toBeNull();
  });

  it('sperrt „Anlegen“ ohne Auswahl oder mit leerem Titel', async () => {
    open();
    const dialog = await findQuestions();
    await userEvent.clear(dialog.getByRole('textbox', { name: 'Titel (Frage 1)' }));
    expect((dialog.getByRole('button', { name: '2 Fragen anlegen' }) as HTMLButtonElement).disabled).toBe(true);
    await userEvent.click(dialog.getByRole('checkbox', { name: 'Frage 1 anlegen' }));
    await userEvent.click(dialog.getByRole('checkbox', { name: 'Frage 2 anlegen' }));
    expect((dialog.getByRole('button', { name: '0 Fragen anlegen' }) as HTMLButtonElement).disabled).toBe(true);
  });

  it('fragt vor dem Schließen nach, weil die Entwürfe nicht gespeichert werden', async () => {
    open();
    const dialog = await findQuestions();
    await userEvent.click(dialog.getByRole('button', { name: 'Schließen' }));
    const confirm = within(dialog.getByRole('alertdialog', { name: 'Entwürfe verwerfen?' }));
    await userEvent.click(confirm.getByRole('button', { name: 'Weiter bearbeiten' }));
    expect(screen.getByRole('dialog', { name: 'Offene Fragen' })).toBeTruthy();
    await userEvent.click(dialog.getByRole('button', { name: 'Schließen' }));
    await userEvent.click(within(dialog.getByRole('alertdialog')).getByRole('button', { name: 'Verwerfen' }));
    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Offene Fragen' })).toBeNull());
  });

  it('meldet, wenn keine offenen Fragen gefunden wurden', async () => {
    open({ createQuestionDrafts: vi.fn(async () => ({ drafts: [], dropped: [], costUsd: null, at: '' })) });
    const dialog = await findQuestions();
    expect(dialog.getByText(/keine offenen Fragen gefunden/)).toBeTruthy();
  });

  it('zeigt während der Suche Fortschritt und Abbrechen, danach Fehler am Knopf', async () => {
    let fail: (e: Error) => void = () => {};
    const { cancelQuestionDrafts } = open({ createQuestionDrafts: vi.fn(() => new Promise((_, reject) => { fail = reject; })) });
    await screen.findByRole('heading', { name: 'Ziel' });
    await userEvent.click(sideActions().getByRole('button', { name: 'Fragen erzeugen' }));
    expect(sideActions().getByRole('button', { name: /Fragen werden gesucht/ })).toBeTruthy();
    expect(sideActions().getByRole('status').textContent).toContain('offenen Fragen');
    await userEvent.click(sideActions().getByRole('button', { name: 'Fragensuche abbrechen' }));
    expect(cancelQuestionDrafts).toHaveBeenCalledWith('p1');
    expect(sideActions().getByRole('button', { name: 'Fragen erzeugen' })).toBeTruthy();
    fail(new ApiError('analysis-failed', 'zu spät', 422));
    // Die Antwort eines abgebrochenen Laufs fällt weg
    await new Promise((r) => setTimeout(r, 10));
    expect(sideActions().queryByRole('alert')).toBeNull();
  });

  it('zeigt einen Fehler der Suche unter dem Knopf', async () => {
    open({ createQuestionDrafts: vi.fn(async () => { throw new ApiError('analysis-failed', 'Antwort nicht lesbar', 422); }) });
    await screen.findByRole('heading', { name: 'Ziel' });
    await userEvent.click(sideActions().getByRole('button', { name: 'Fragen erzeugen' }));
    expect((await sideActions().findByRole('alert')).textContent).toContain('Antwort nicht lesbar');
  });
});
