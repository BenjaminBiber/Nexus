// @vitest-environment jsdom
import { describe, it, expect, afterEach, beforeEach, vi } from 'vitest';
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { Link, MemoryRouter, Route, Routes } from 'react-router-dom';
import { ApiProvider } from '../src/api/ApiContext.js';
import { DocTicketJobsIndicator } from '../src/components/DocTicketJobsIndicator.js';
import { draftDependents } from '../src/components/DraftDialog.js';
import { DocTicketJobsProvider } from '../src/hooks/docTicketJobs.js';
import userEvent from '@testing-library/user-event';
import { ApiError } from '../src/api/ApiError.js';
import type { MarkdownSectionDto, TicketDraftDto } from '../src/api/types.js';
import { DocsPage } from '../src/pages/DocsPage.js';
import { fakeApi } from './helpers/fakeApi.js';
import { renderWithApp } from './helpers/renderWithApp.js';

vi.mock('mermaid', () => ({ default: { initialize: vi.fn(), render: vi.fn(async () => ({ svg: '<svg></svg>' })) } }));

afterEach(cleanup);
beforeEach(() => window.localStorage.clear());

const PATH = 'docs/superpowers/specs/2026-10-22-x-design.md';
const CONTENT = '# Spec\n\n## Ziel\n\nEin Ticket wird angelegt.\n\n## Weiteres\n\nNoch mehr Text.\n';
const SECTIONS: MarkdownSectionDto[] = [
  { heading: 'Ziel', level: 2, startLine: 3, endLine: 6 },
  { heading: 'Weiteres', level: 2, startLine: 7, endLine: 9 },
];
const specDoc = { path: PATH, title: '001-x · spec.md', group: 'spec' as const, kind: 'spec' as const, topic: null, date: null, mtimeMs: 1, exists: true };

// Entwurf mit Standardwerten; die Überschreibungen gewinnen
function draft(title: string, extra: Partial<TicketDraftDto> = {}): TicketDraftDto {
  return {
    title, description: 'Beschreibung', acceptance: 'Abnahme', steps: ['Schritt eins'], dependsOn: [], labels: ['web'],
    source: { heading: 'Ziel', startLine: 5, endLine: 5 }, ...extra,
  };
}

const DRAFTS = {
  drafts: [draft('Erstes Ticket'), draft('Zweites Ticket')],
  dropped: ['Eine Stelle war zu kurz'],
  costUsd: 0.12,
  at: '2026-10-23T08:00:00.000Z',
};

// Rendert die Docs-Seite mit einem Spec-Dokument; die Doppel aus den Überschreibungen gewinnen
function open(overrides: Record<string, unknown> = {}) {
  const createTicketDrafts = vi.fn(async () => DRAFTS);
  const createDocTickets = vi.fn(async () => ({ created: [{ index: 0, id: 'AIH-0101' }], skipped: [] }));
  const api = fakeApi({
    getDocs: async () => ({ source: { root: 'C:/repo', boardId: 1, boardName: 'Sprint', reason: null }, docs: [specDoc] }),
    getDoc: async (_p: string, p: string) => ({ path: p, content: CONTENT, hash: 'h1', sections: SECTIONS }),
    getDocComments: async () => [],
    createTicketDrafts,
    createDocTickets,
    ...overrides,
  });
  renderWithApp(<DocsPage />, { api, path: `/p/p1/docs/${PATH}`, route: '/p/:pid/docs/*' });
  return {
    createTicketDrafts: (overrides.createTicketDrafts ?? createTicketDrafts) as ReturnType<typeof vi.fn>,
    createDocTickets: (overrides.createDocTickets ?? createDocTickets) as ReturnType<typeof vi.fn>,
  };
}

// Markiert einen Teil eines Absatzes wie mit der Maus
function select(match: RegExp, from: number, to: number) {
  const text = screen.getByText(match).firstChild!;
  const range = document.createRange();
  range.setStart(text, from);
  range.setEnd(text, to);
  window.getSelection()!.removeAllRanges();
  window.getSelection()!.addRange(range);
  fireEvent.mouseUp(document.querySelector('article.doc')!);
}

// Knopf „Tickets erzeugen“ oben in der Werkzeugspalte
const docButton = () => within(document.querySelector('.doc-side-actions') as HTMLElement).getByRole('button', { name: 'Tickets erzeugen' });

// Lässt das ganze Dokument zerlegen und liefert den Entwurfs-Dialog
async function makeDrafts() {
  await screen.findByRole('heading', { name: 'Ziel' });
  await userEvent.click(docButton());
  return within(await screen.findByRole('dialog', { name: 'Ticket-Entwürfe' }));
}

describe('Markieren und Entwürfe', () => {
  it('bietet im Markier-Menü keine Ticket-Auswahl an', async () => {
    open();
    await screen.findByRole('heading', { name: 'Ziel' });
    select(/Ein Ticket wird angelegt/, 0, 24);
    const group = within(await screen.findByRole('group', { name: 'Aktionen zur Auswahl' }));
    expect(group.queryByRole('button', { name: 'Zur Ticket-Auswahl' })).toBeNull();
    expect(screen.queryByRole('region', { name: 'Ticket-Auswahl' })).toBeNull();
  });

  it('lässt Karten bearbeiten und verwerfen', async () => {
    open();
    const dialog = await makeDrafts();
    expect(dialog.getByText('Eine Stelle war zu kurz')).toBeTruthy();
    const title = dialog.getByRole('textbox', { name: 'Titel (Entwurf 1)' });
    await userEvent.clear(title);
    await userEvent.type(title, 'Von Hand benannt');
    await userEvent.click(dialog.getByRole('button', { name: 'Schritt hinzufügen (Entwurf 1)' }));
    await userEvent.type(dialog.getByRole('textbox', { name: 'Schritt 2 (Entwurf 1)' }), 'Schritt zwei');
    await userEvent.click(dialog.getByRole('button', { name: 'Entwurf 2 verwerfen' }));
    expect(dialog.queryByRole('textbox', { name: 'Titel (Entwurf 2)' })).toBeNull();
    expect((dialog.getByRole('textbox', { name: 'Titel (Entwurf 1)' }) as HTMLInputElement).value).toBe('Von Hand benannt');
  });

  it('schickt genau die bearbeiteten Entwürfe an docs/tickets', async () => {
    const { createDocTickets } = open();
    const dialog = await makeDrafts();
    const title = dialog.getByRole('textbox', { name: 'Titel (Entwurf 1)' });
    await userEvent.clear(title);
    await userEvent.type(title, 'Von Hand benannt');
    await userEvent.click(dialog.getByRole('button', { name: 'Schritt hinzufügen (Entwurf 1)' }));
    await userEvent.type(dialog.getByRole('textbox', { name: 'Schritt 2 (Entwurf 1)' }), 'Schritt zwei');
    await userEvent.click(dialog.getByRole('button', { name: 'Entwurf 2 verwerfen' }));
    await userEvent.click(dialog.getByRole('button', { name: 'Akzeptieren' }));
    expect(createDocTickets).toHaveBeenCalledWith('p1', {
      path: PATH,
      drafts: [draft('Von Hand benannt', { steps: ['Schritt eins', 'Schritt zwei'] })],
    }, undefined);
  });

  it('zeigt angelegte IDs als Links und die übersprungene Dublette', async () => {
    open({
      createDocTickets: vi.fn(async () => ({
        created: [{ index: 0, id: 'AIH-0101' }],
        skipped: [{ index: 1, reason: 'Dublette', existingId: 'AIH-0042' }],
      })),
    });
    const dialog = await makeDrafts();
    await userEvent.click(dialog.getByRole('button', { name: 'Akzeptieren' }));
    const link = await dialog.findByRole('link', { name: 'AIH-0101' });
    expect(link.getAttribute('href')).toBe('/p/p1/board/t/AIH-0101');
    expect(dialog.getByText(/übersprungen: Dublette/)).toBeTruthy();
    expect(dialog.getByRole('link', { name: 'bereits vorhanden' }).getAttribute('href')).toBe('/p/p1/board/t/AIH-0042');
  });


  it('meldet den Fehler von ticket-drafts in der Werkzeugspalte und legt nichts an', async () => {
    const { createDocTickets } = open({
      createTicketDrafts: vi.fn(async () => { throw new ApiError('analysis-failed', 'Claude konnte keine Entwürfe schreiben.', 502); }),
    });
    await screen.findByRole('heading', { name: 'Ziel' });
    await userEvent.click(docButton());
    const side = within(document.querySelector('.doc-side-actions') as HTMLElement);
    expect((await side.findByRole('alert')).textContent).toContain('Claude konnte keine Entwürfe schreiben.');
    expect(createDocTickets).not.toHaveBeenCalled();
    expect(screen.queryByRole('dialog', { name: 'Ticket-Entwürfe' })).toBeNull();
  });
});

// Ticket aus dem Dokument, wie es GET …/docs/tickets liefert
const ref = (id: string) => ({ id, title: `Ticket ${id}`, status: 'open', column: 'backlog', heading: 'Ziel', startLine: 5, endLine: 5 });

describe('Tickets aus dem ganzen Dokument', () => {
  it('zerlegt das ganze Dokument ohne Markierung, zeigt die Entwürfe und legt sie nach „Akzeptieren“ an', async () => {
    const getDocTickets = vi.fn(async () => [] as ReturnType<typeof ref>[]);
    const { createTicketDrafts, createDocTickets } = open({ getDocTickets });
    await screen.findByRole('heading', { name: 'Ziel' });
    await userEvent.click(docButton());
    expect(createTicketDrafts).toHaveBeenCalledWith('p1', { path: PATH, selections: [], note: '', whole: true }, undefined);
    const dialog = within(await screen.findByRole('dialog', { name: 'Ticket-Entwürfe' }));
    expect(dialog.getByDisplayValue('Erstes Ticket')).toBeTruthy();
    getDocTickets.mockResolvedValue([ref('AIH-0101')]);
    await userEvent.click(dialog.getByRole('button', { name: 'Akzeptieren' }));
    await waitFor(() => expect(createDocTickets).toHaveBeenCalledTimes(1));
    // Danach kennt das Dokument die neuen Tickets und sperrt den Knopf
    await waitFor(() => expect(getDocTickets).toHaveBeenCalledTimes(2));
  });

  it('sperrt den Knopf, wenn aus dem Dokument schon Tickets entstanden sind', async () => {
    const { createTicketDrafts } = open({ getDocTickets: async () => [ref('AIH-0101'), ref('AIH-0102')] });
    await screen.findByRole('heading', { name: 'Ziel' });
    await screen.findByText('AIH-0101');
    await waitFor(() => expect((docButton() as HTMLButtonElement).disabled).toBe(true));
    expect(screen.getByText(/sind schon 2 Tickets entstanden \(AIH-0101, AIH-0102\)/)).toBeTruthy();
    await userEvent.click(docButton());
    expect(createTicketDrafts).not.toHaveBeenCalled();
  });

  it('bietet den Knopf nur an Specs und Plänen an', async () => {
    open({
      getDocs: async () => ({ source: { root: 'C:/repo', boardId: 1, boardName: 'Sprint', reason: null }, docs: [{ ...specDoc, path: PATH, group: 'glossary' as const }] }),
    });
    await screen.findByRole('heading', { name: 'Ziel' });
    expect(document.querySelector('.doc-side-actions')).toBeNull();
  });
});

describe('Entwurfs-Dialog mit Kacheln', () => {
  it('zeigt links die Entwürfe als Kacheln und rechts die Details der gewählten', async () => {
    open();
    await screen.findByRole('heading', { name: 'Ziel' });
    await userEvent.click(docButton());
    const dialog = within(await screen.findByRole('dialog', { name: 'Ticket-Entwürfe' }));
    const tiles = within(dialog.getByRole('list', { name: 'Entwürfe' }));
    expect(tiles.getAllByRole('listitem')).toHaveLength(2);
    expect(dialog.getByRole('region', { name: 'Entwurf 1' })).toBeTruthy();
    expect(dialog.queryByRole('textbox', { name: 'Titel (Entwurf 2)' })).toBeNull();

    await userEvent.click(tiles.getByRole('button', { name: /Zweites Ticket/ }));
    expect((dialog.getByRole('textbox', { name: 'Titel (Entwurf 2)' }) as HTMLInputElement).value).toBe('Zweites Ticket');
    await userEvent.click(dialog.getByRole('checkbox', { name: /Erstes Ticket/ }));
    expect(tiles.getByText('nach #1')).toBeTruthy();

    // #2 hängt jetzt von #1 ab: #1 darf nicht mehr auf #2 warten
    await userEvent.click(tiles.getByRole('button', { name: /Erstes Ticket/ }));
    const back = dialog.getByRole('checkbox', { name: /Zweites Ticket/ }) as HTMLInputElement;
    expect(back.disabled).toBe(true);
    expect(dialog.getByText('setzt dieses Ticket voraus')).toBeTruthy();
  });

  it('findet abhängige Entwürfe auch über Umwege', () => {
    const cards = [{ dependsOn: [] }, { dependsOn: [0] }, { dependsOn: [1] }, { dependsOn: [] }];
    expect([...draftDependents(cards, 0)].sort()).toEqual([1, 2]);
    expect([...draftDependents(cards, 3)]).toEqual([]);
  });
});

describe('Ticket-Erzeugung über Seitenwechsel', () => {
  // Docs-Seite und eine andere Seite im App-Rahmen mit gemeinsamem Speicher der Erzeugungen
  function openWithApp(createTicketDrafts: ReturnType<typeof vi.fn>, extra: Record<string, unknown> = {}) {
    const api = fakeApi({
      getDocs: async () => ({ source: { root: 'C:/repo', boardId: 1, boardName: 'Sprint', reason: null }, docs: [specDoc] }),
      getDoc: async (_p: string, p: string) => ({ path: p, content: CONTENT, hash: 'h1', sections: SECTIONS }),
      getDocComments: async () => [],
      createTicketDrafts,
      ...extra,
    });
    render(
      <ApiProvider api={api}>
        <DocTicketJobsProvider>
          <MemoryRouter initialEntries={[`/p/p1/docs/${PATH}`]}>
            <DocTicketJobsIndicator />
            <Link to="/p/p1/board">Zum Board</Link>
            <Routes>
              <Route path="/p/:pid/docs/*" element={<DocsPage />} />
              <Route path="*" element={<div>Andere Seite</div>} />
            </Routes>
          </MemoryRouter>
        </DocTicketJobsProvider>
      </ApiProvider>,
    );
  }

  it('läuft beim Wechsel auf eine andere Seite weiter und führt über den Hinweis zu den Entwürfen zurück', async () => {
    let finish: (value: typeof DRAFTS) => void = () => {};
    const createTicketDrafts = vi.fn(() => new Promise<typeof DRAFTS>((resolve) => { finish = resolve; }));
    openWithApp(createTicketDrafts);
    await screen.findByRole('heading', { name: 'Ziel' });
    await userEvent.click(docButton());
    // Am offenen Dokument kein Hinweis unten rechts, dort steht die Ladeanzeige
    expect(screen.queryByRole('list', { name: 'Ticket-Erzeugung' })).toBeNull();

    await userEvent.click(screen.getByRole('link', { name: 'Zum Board' }));
    await screen.findByText('Andere Seite');
    const jobs = within(screen.getByRole('list', { name: 'Ticket-Erzeugung' }));
    expect(jobs.getByText('Claude erzeugt Ticket-Entwürfe …')).toBeTruthy();

    finish(DRAFTS);
    expect(await jobs.findByText('2 Entwürfe bereit')).toBeTruthy();
    await userEvent.click(jobs.getByRole('link', { name: 'Ansehen' }));
    const dialog = within(await screen.findByRole('dialog', { name: 'Ticket-Entwürfe' }));
    expect(dialog.getByRole('textbox', { name: 'Titel (Entwurf 1)' })).toBeTruthy();
    expect(createTicketDrafts).toHaveBeenCalledTimes(1);
  });

  it('zeigt im Baum das pulsierende Claude-Symbol, solange die Erzeugung läuft, und bricht sie ab', async () => {
    let finish: (value: typeof DRAFTS) => void = () => {};
    const createTicketDrafts = vi.fn(() => new Promise<typeof DRAFTS>((resolve) => { finish = resolve; }));
    const cancelTicketDrafts = vi.fn(async () => ({ cancelled: true }));
    openWithApp(createTicketDrafts, { cancelTicketDrafts });
    await screen.findByRole('heading', { name: 'Ziel' });
    expect(document.querySelector('.doc-tree-claude')).toBeNull();
    await userEvent.click(docButton());
    expect(await screen.findByRole('img', { name: 'Claude erzeugt Tickets' })).toBeTruthy();

    const side = within(document.querySelector<HTMLElement>('.doc-side-actions')!);
    await userEvent.click(side.getByRole('button', { name: 'Erzeugung abbrechen' }));
    expect(cancelTicketDrafts).toHaveBeenCalledWith('p1');
    expect(screen.queryByRole('img', { name: 'Claude erzeugt Tickets' })).toBeNull();
    expect(side.queryByRole('status')).toBeNull();
    expect((side.getByRole('button', { name: /Tickets erzeugen/ }) as HTMLButtonElement).disabled).toBe(false);

    // Die späte Antwort des abgebrochenen Laufs öffnet keinen Dialog mehr
    finish(DRAFTS);
    await new Promise((r) => setTimeout(r, 0));
    expect(screen.queryByRole('dialog', { name: 'Ticket-Entwürfe' })).toBeNull();
  });

  it('bricht die Erzeugung auch über den Hinweis auf einer anderen Seite ab', async () => {
    const createTicketDrafts = vi.fn(() => new Promise<typeof DRAFTS>(() => {}));
    const cancelTicketDrafts = vi.fn(async () => ({ cancelled: true }));
    openWithApp(createTicketDrafts, { cancelTicketDrafts });
    await screen.findByRole('heading', { name: 'Ziel' });
    await userEvent.click(docButton());
    await userEvent.click(screen.getByRole('link', { name: 'Zum Board' }));
    await screen.findByText('Andere Seite');
    const jobs = within(screen.getByRole('list', { name: 'Ticket-Erzeugung' }));
    await userEvent.click(jobs.getByRole('button', { name: /abbrechen/ }));
    expect(cancelTicketDrafts).toHaveBeenCalledWith('p1');
    expect(screen.queryByRole('list', { name: 'Ticket-Erzeugung' })).toBeNull();
  });
});

describe('Gespeicherte Entwürfe', () => {
  const side = () => within(document.querySelector('.doc-side-actions') as HTMLElement);

  it('bietet gespeicherte Entwürfe mit „Entwürfe öffnen“ an und speichert Änderungen beim Schließen', async () => {
    const getSavedTicketDrafts = vi.fn(async () => ({ result: DRAFTS }));
    const saveTicketDrafts = vi.fn(async (_p: string, _path: string, drafts: TicketDraftDto[]) => ({ result: { ...DRAFTS, drafts } }));
    const { createTicketDrafts } = open({ getSavedTicketDrafts, saveTicketDrafts });
    await screen.findByRole('heading', { name: 'Ziel' });
    await userEvent.click(await side().findByRole('button', { name: 'Entwürfe öffnen' }));
    expect(getSavedTicketDrafts).toHaveBeenCalledWith('p1', PATH, undefined);
    expect(side().getByText(/2 gespeicherte Entwürfe/)).toBeTruthy();
    const dialog = within(await screen.findByRole('dialog', { name: 'Ticket-Entwürfe' }));
    expect(createTicketDrafts).not.toHaveBeenCalled();

    const title = dialog.getByRole('textbox', { name: 'Titel (Entwurf 1)' });
    await userEvent.clear(title);
    await userEvent.type(title, 'Geändert');
    await userEvent.click(dialog.getByRole('button', { name: 'Schließen' }));
    expect(screen.queryByRole('dialog', { name: 'Ticket-Entwürfe' })).toBeNull();
    expect(saveTicketDrafts).toHaveBeenCalledWith('p1', PATH, expect.any(Array), undefined);
    const saved = saveTicketDrafts.mock.calls[0][2];
    expect(saved.map((d) => d.title)).toEqual(['Geändert', 'Zweites Ticket']);

    // Wieder geöffnet zeigt der Dialog den geänderten Stand
    await userEvent.click(side().getByRole('button', { name: 'Entwürfe öffnen' }));
    expect(within(await screen.findByRole('dialog', { name: 'Ticket-Entwürfe' })).getByDisplayValue('Geändert')).toBeTruthy();
  });

  it('merkt sich frisch erzeugte Entwürfe nach dem Schließen', async () => {
    open();
    await screen.findByRole('heading', { name: 'Ziel' });
    await userEvent.click(docButton());
    const dialog = within(await screen.findByRole('dialog', { name: 'Ticket-Entwürfe' }));
    await userEvent.click(dialog.getByRole('button', { name: 'Schließen' }));
    expect(side().getByRole('button', { name: 'Entwürfe öffnen' })).toBeTruthy();
  });

  it('verwirft die gespeicherten Entwürfe; danach heißt der Knopf wieder „Tickets erzeugen“', async () => {
    const discardTicketDrafts = vi.fn(async () => ({ ok: true }));
    open({ getSavedTicketDrafts: async () => ({ result: DRAFTS }), discardTicketDrafts });
    await screen.findByRole('heading', { name: 'Ziel' });
    await userEvent.click(await side().findByRole('button', { name: 'Entwürfe öffnen' }));
    const dialog = within(await screen.findByRole('dialog', { name: 'Ticket-Entwürfe' }));
    await userEvent.click(dialog.getByRole('button', { name: 'Entwürfe verwerfen' }));
    expect(discardTicketDrafts).toHaveBeenCalledWith('p1', PATH, undefined);
    expect(screen.queryByRole('dialog', { name: 'Ticket-Entwürfe' })).toBeNull();
    expect(docButton()).toBeTruthy();
  });

  it('löscht die gespeicherten Entwürfe nach Rückfrage direkt über die Werkzeugspalte', async () => {
    const discardTicketDrafts = vi.fn(async () => ({ ok: true }));
    open({ getSavedTicketDrafts: async () => ({ result: DRAFTS }), discardTicketDrafts });
    await screen.findByRole('heading', { name: 'Ziel' });
    await userEvent.click(await side().findByRole('button', { name: 'Entwürfe löschen' }));
    const ask = within(side().getByRole('alertdialog', { name: 'Gespeicherte Entwürfe löschen?' }));
    await userEvent.click(ask.getByRole('button', { name: 'Abbrechen' }));
    expect(discardTicketDrafts).not.toHaveBeenCalled();
    await userEvent.click(side().getByRole('button', { name: 'Entwürfe löschen' }));
    await userEvent.click(within(side().getByRole('alertdialog')).getByRole('button', { name: 'Löschen' }));
    expect(discardTicketDrafts).toHaveBeenCalledWith('p1', PATH, undefined);
    expect(docButton()).toBeTruthy();
    expect(side().queryByRole('button', { name: 'Entwürfe öffnen' })).toBeNull();
  });

  it('filtert den Baum auf Dateien mit gespeicherten Entwürfen', async () => {
    const plan = { ...specDoc, path: 'docs/superpowers/plans/2026-10-22-y.md', title: '002-y · plan.md', group: 'plan' as const, kind: 'plan' as const };
    open({
      getDocs: async () => ({ source: { root: 'C:/repo', boardId: 1, boardName: 'Sprint', reason: null }, docs: [{ ...specDoc, hasDrafts: true }, plan] }),
    });
    await screen.findByRole('heading', { name: 'Ziel' });
    const tree = within(screen.getByRole('tree'));
    expect(tree.getByText(/2026-10-22-y/)).toBeTruthy();
    await userEvent.click(screen.getByRole('button', { name: 'Entwürfe vorhanden' }));
    expect(tree.queryByText(/2026-10-22-y/)).toBeNull();
    expect(tree.getByText(/2026-10-22-x-design/)).toBeTruthy();
    await userEvent.click(screen.getByRole('button', { name: 'Entwürfe vorhanden' }));
    expect(tree.getByText(/2026-10-22-y/)).toBeTruthy();
  });

  it('erzeugt über „Neu erzeugen“ frische Entwürfe statt der gespeicherten', async () => {
    const { createTicketDrafts } = open({ getSavedTicketDrafts: async () => ({ result: { ...DRAFTS, drafts: [draft('Alt')] } }) });
    await screen.findByRole('heading', { name: 'Ziel' });
    await userEvent.click(await side().findByRole('button', { name: 'Neu erzeugen' }));
    expect(createTicketDrafts).toHaveBeenCalledTimes(1);
    const dialog = within(await screen.findByRole('dialog', { name: 'Ticket-Entwürfe' }));
    expect(dialog.getByDisplayValue('Erstes Ticket')).toBeTruthy();
  });
});

describe('Datei öffnen', () => {
  it('öffnet die Datei über das Menü am Pfad im Explorer oder in VS Code', async () => {
    const openDoc = vi.fn(async () => ({ ok: true }));
    open({ openDoc });
    await screen.findByRole('heading', { name: 'Ziel' });
    await userEvent.click(screen.getByRole('button', { name: 'Datei öffnen' }));
    await userEvent.click(screen.getByRole('menuitem', { name: /Im Explorer zeigen/ }));
    expect(openDoc).toHaveBeenCalledWith('p1', PATH, 'explorer', undefined);
    expect(screen.queryByRole('menu')).toBeNull();
    await userEvent.click(screen.getByRole('button', { name: 'Datei öffnen' }));
    await userEvent.click(screen.getByRole('menuitem', { name: /In VS Code öffnen/ }));
    expect(openDoc).toHaveBeenLastCalledWith('p1', PATH, 'vscode', undefined);
  });

  it('meldet, wenn sich das Programm nicht öffnen ließ', async () => {
    open({ openDoc: vi.fn(async () => { throw new ApiError('open-unavailable', 'VS Code (Befehl „code“) ließ sich nicht öffnen (Code 1).', 422); }) });
    await screen.findByRole('heading', { name: 'Ziel' });
    await userEvent.click(screen.getByRole('button', { name: 'Datei öffnen' }));
    await userEvent.click(screen.getByRole('menuitem', { name: /In VS Code öffnen/ }));
    expect((await screen.findByRole('alert')).textContent).toContain('ließ sich nicht öffnen');
  });
});
