// @vitest-environment jsdom
import { describe, it, expect, afterEach, vi } from 'vitest';
import { cleanup, render, screen, within } from '@testing-library/react';
import { MemoryRouter, Route, Routes, useLocation } from 'react-router-dom';
import { ApiProvider } from '../src/api/ApiContext.js';
import userEvent from '@testing-library/user-event';
import { ApiError } from '../src/api/ApiError.js';
import type { DbSpecProgressDto } from '../src/api/dbTypes.js';
import type { DocFileDto, DocsSourceReason } from '../src/api/types.js';
import { LiveProvider } from '../src/live/LiveProvider.js';
import { DocsPage } from '../src/pages/DocsPage.js';
import { dbBoards, fakeApi } from './helpers/fakeApi.js';
import { FakeEventSource } from './helpers/FakeEventSource.js';
import { renderWithApp } from './helpers/renderWithApp.js';

vi.mock('mermaid', () => ({ default: { initialize: vi.fn(), render: vi.fn(async () => ({ svg: '<svg></svg>' })) } }));

// Zeigt die aktuelle Adresse zur Prüfung an
function AddressProbe() {
  const l = useLocation();
  return <span data-testid="address">{l.pathname + l.search + l.hash}</span>;
}

const originalScroll = Element.prototype.scrollIntoView;

// Räumt auf, stellt scrollIntoView wieder her und leert den gemerkten Docs-Stand
afterEach(() => {
  cleanup();
  Element.prototype.scrollIntoView = originalScroll;
  FakeEventSource.reset();
  window.localStorage.clear();
});

const SPEC = 'docs/superpowers/specs/2026-10-22-spec-docs-rework-design.md';
const PLAN = 'docs/superpowers/plans/2026-10-22-spec-docs-rework-plan.md';
const ALT = 'docs/superpowers/specs/2026-09-01-alt-design.md';

const docs: DocFileDto[] = [
  { path: SPEC, title: 'Spec-Docs-Rework – Design', group: 'spec', kind: 'spec', topic: 'spec-docs-rework', date: '2026-10-22', mtimeMs: 300, exists: true },
  { path: PLAN, title: 'Spec-Docs-Rework – Plan', group: 'plan', kind: 'plan', topic: 'spec-docs-rework', date: '2026-10-22', mtimeMs: 200, exists: true },
  { path: ALT, title: 'Altes Thema – Design', group: 'spec', kind: 'spec', topic: 'alt', date: '2026-09-01', mtimeMs: 100, exists: true },
  { path: 'GLOSSARY.md', title: 'Glossar', group: 'glossary', kind: null, topic: null, date: null, mtimeMs: 50, exists: true },
  { path: 'docs/adr/0001-eins.md', title: '0001-eins', group: 'adr', kind: null, topic: null, date: null, mtimeMs: 60, exists: true },
];

// Eine weitere, neuere Spec – erscheint erst nach einer Live-Meldung
const neu: DocFileDto = {
  path: 'docs/superpowers/specs/2026-10-23-ganz-neu-design.md',
  title: 'Ganz neu – Design', group: 'spec', kind: 'spec', topic: 'ganz-neu', date: '2026-10-23', mtimeMs: 900, exists: true,
};

// Antwort von getDocs: Quelle mit Repo-Pfad plus Liste
const listing = (list: DocFileDto[] = docs) => ({
  source: { root: 'C:/repo', boardId: 1, boardName: 'Sprint', reason: null },
  docs: list,
});

// Antwort von getDocs ohne Quelle: nur der Grund
const noSource = (reason: DocsSourceReason) => ({
  source: { root: null, boardId: null, boardName: null, reason },
  docs: [] as DocFileDto[],
});

const contents: Record<string, string> = {
  [SPEC]: '# Spec X\n\nEin Ticket wird angelegt.',
  [ALT]: '# Spec Z\n\n## Der `harness`-Befehl\n\nText\n\n## Doppelt\n\n## Doppelt\n\n### Unter\n',
  'GLOSSARY.md': '# Glossar\n\n**Ticket**: Arbeitseinheit',
  'docs/adr/0001-eins.md': '# Entscheidung Eins',
};

// Rendert die Docs-Seite unter einem Pfad
function open(path = '/p/p1/docs', overrides: Parameters<typeof fakeApi>[0] = {}) {
  const getDoc = vi.fn(async (_pid: string, p: string) => ({ path: p, content: contents[p] ?? '' }));
  const searchDocs = vi.fn(async () => [
    { path: 'GLOSSARY.md', line: 3, excerpt: '**Ticket**: Arbeitseinheit' },
    { path: SPEC, line: 3, excerpt: 'Ein Ticket wird angelegt.' },
  ]);
  const api = fakeApi({ getDocs: async () => listing(), getDoc, searchDocs, getDocComments: async () => [], ...overrides });
  renderWithApp(<DocsPage />, { api, path, route: '/p/:pid/docs/*' });
  return { getDoc, searchDocs };
}

// Der Knoten einer Datei oder eines Ordners; ohne Badge „neu“ ist sein Name der Datei- bzw. Ordnername
const node = (name: string | RegExp) => screen.getByRole('treeitem', { name });

describe('DocsPage', () => {
  it('zeigt die Ordner der Pfade wie im Explorer, Ordner vor Dateien', async () => {
    window.localStorage.setItem('harness.docsSeen.p1', '1000');
    open();
    const tree = within(await screen.findByRole('tree', { name: 'Dokumente' }));
    const roots = tree.getAllByRole('treeitem').filter((n) => n.getAttribute('aria-level') === '1');
    // Das Board steht als oberster Ordner; Hüllordner wie docs/superpowers entfallen
    expect(roots.map((n) => n.getAttribute('aria-label'))).toEqual(['Sprint']);
    const sub = within(roots[0]).getAllByRole('treeitem').filter((n) => n.getAttribute('aria-level') === '2');
    expect(sub.map((n) => n.getAttribute('aria-label'))).toEqual(['adr', 'plans', 'specs', 'GLOSSARY.md']);
    expect(screen.getByText('Wähle links ein Dokument.')).toBeTruthy();
  });

  it('zeigt Specs und Pläne als Dateien mit eigenem Symbol und dem Titel als Tooltip', async () => {
    window.localStorage.setItem('harness.docsSeen.p1', '1000');
    open();
    await screen.findByRole('tree', { name: 'Dokumente' });
    const specs = within(node('specs'));
    expect(specs.getAllByRole('link').map((a) => a.textContent)).toEqual(['2026-09-01-alt-design.md', '2026-10-22-spec-docs-rework-design.md']);
    const spec = node('2026-10-22-spec-docs-rework-design.md');
    expect(within(spec).getByRole('link').getAttribute('href')).toContain(SPEC);
    expect(within(spec).getByRole('link').getAttribute('title')).toBe('Spec-Docs-Rework – Design');
    expect(spec.querySelector('.doc-tree-icon.is-doc')).not.toBeNull();
    const plan = node('2026-10-22-spec-docs-rework-plan.md');
    expect(within(plan).getByRole('link').getAttribute('href')).toContain(PLAN);
    expect(plan.querySelector('.doc-tree-icon.is-plan')).not.toBeNull();
  });

  it('zeigt eine gewählte Datei gerendert', async () => {
    const { getDoc } = open();
    await screen.findByRole('tree', { name: 'Dokumente' });
    await userEvent.click(screen.getByRole('link', { name: /^GLOSSARY\.md/ }));
    expect(await screen.findByRole('heading', { name: 'Glossar', level: 1 })).toBeTruthy();
    expect(getDoc).toHaveBeenCalledWith('p1', 'GLOSSARY.md', undefined);
  });

  it('öffnet eine Datei direkt über die Adresse', async () => {
    open('/p/p1/docs/docs/adr/0001-eins.md');
    expect(await screen.findByRole('heading', { name: 'Entscheidung Eins' })).toBeTruthy();
  });

  it('sucht ab zwei Zeichen und zeigt Treffer mit Datei und Auszug', async () => {
    const { searchDocs } = open();
    await screen.findByRole('tree', { name: 'Dokumente' });
    const box = screen.getByLabelText('In Docs suchen');
    await userEvent.type(box, 't');
    await new Promise((r) => setTimeout(r, 400));
    expect(searchDocs).not.toHaveBeenCalled();
    await userEvent.type(box, 'icket');
    const results = await screen.findByRole('list', { name: 'Suchergebnisse' });
    expect(searchDocs).toHaveBeenCalledWith('p1', 'ticket', undefined);
    expect(within(results).getAllByRole('listitem')).toHaveLength(2);
    expect(within(results).getByText('Ein Ticket wird angelegt.')).toBeTruthy();
  });

  it('öffnet einen Treffer und hebt den Suchbegriff im Dokument hervor', async () => {
    open();
    await screen.findByRole('tree', { name: 'Dokumente' });
    await userEvent.type(screen.getByLabelText('In Docs suchen'), 'ticket');
    const results = await screen.findByRole('list', { name: 'Suchergebnisse' });
    await userEvent.click(within(results).getByText('Ein Ticket wird angelegt.'));
    expect(await screen.findByRole('heading', { name: 'Spec X' })).toBeTruthy();
    const marks = document.querySelectorAll('mark');
    expect(marks.length).toBeGreaterThan(0);
    expect(marks[0].textContent?.toLowerCase()).toBe('ticket');
  });

  it('meldet, wenn es keinen Treffer gibt', async () => {
    open('/p/p1/docs', { searchDocs: async () => [] });
    await screen.findByRole('tree', { name: 'Dokumente' });
    await userEvent.type(screen.getByLabelText('In Docs suchen'), 'nichts');
    expect(await screen.findByText('Keine Treffer.')).toBeTruthy();
  });

  it('zeigt Fehler beim Laden einer Datei mit der Meldung des Servers', async () => {
    open('/p/p1/docs/GLOSSARY.md', { getDoc: async () => { throw new ApiError('doc-not-found', 'Diese Datei gibt es nicht oder sie ist nicht freigegeben.', 404); } });
    expect(await screen.findByText('Diese Datei gibt es nicht oder sie ist nicht freigegeben.')).toBeTruthy();
  });

  it('springt im Inhaltsverzeichnis auch bei Code und doppelten Überschriften zur Stelle', async () => {
    const scroll = vi.fn();
    Element.prototype.scrollIntoView = scroll;
    open(`/p/p1/docs/${ALT}`);
    const toc = within(await screen.findByRole('navigation', { name: 'Inhaltsverzeichnis' }));
    await userEvent.click(toc.getByRole('button', { name: 'Der harness-Befehl' }));
    expect(scroll.mock.contexts.at(-1)).toHaveProperty('id', 'der-harness-befehl');
    await userEvent.click(toc.getAllByRole('button', { name: 'Doppelt' })[1]);
    expect(scroll.mock.contexts.at(-1)).toHaveProperty('id', 'doppelt-2');
    expect(toc.getByRole('button', { name: 'Unter' })).toBeTruthy();
  });

  it('behält beim Sprung im Inhaltsverzeichnis den Suchbegriff in der Adresse', async () => {
    Element.prototype.scrollIntoView = vi.fn();
    const api = fakeApi({ getDocs: async () => listing(), getDocComments: async () => [], getDoc: async (_pid: string, p: string) => ({ path: p, content: contents[p] ?? '' }) });
    render(
      <ApiProvider api={api}>
        <MemoryRouter initialEntries={[`/p/p1/docs/${ALT}?q=text`]}>
          <Routes>
            <Route path="/p/:pid/docs/*" element={<DocsPage />} />
          </Routes>
          <AddressProbe />
        </MemoryRouter>
      </ApiProvider>,
    );
    const toc = within(await screen.findByRole('navigation', { name: 'Inhaltsverzeichnis' }));
    await userEvent.click(toc.getByRole('button', { name: 'Unter' }));
    expect(screen.getByTestId('address').textContent).toBe(`/p/p1/docs/${ALT}?q=text#unter`);
  });
});

describe('DocsPage – Hinweis ohne Docs-Quelle', () => {
  it('nennt den fehlenden Repo-Pfad und öffnet über den Knopf die Board-Verwaltung', async () => {
    const opened = vi.fn();
    window.addEventListener('harness:db-admin-open', opened);
    open('/p/p1/docs', { getDocs: async () => noSource('no-repo-path') });
    expect(await screen.findByText('Dem Board ist kein Repo-Pfad zugeordnet.')).toBeTruthy();
    await userEvent.click(screen.getByRole('button', { name: 'Board bearbeiten' }));
    expect(opened).toHaveBeenCalled();
    window.removeEventListener('harness:db-admin-open', opened);
  });

  it('nennt das fehlende Board', async () => {
    open('/p/p1/docs', { getDocs: async () => noSource('no-board') });
    expect(await screen.findByText('Für dieses Projekt gibt es kein Board.')).toBeTruthy();
    expect(screen.getByRole('button', { name: 'Board bearbeiten' })).toBeTruthy();
  });

  it('nennt den fehlenden Ordner', async () => {
    open('/p/p1/docs', { getDocs: async () => noSource('missing-folder') });
    expect(await screen.findByText('Im Repo fehlt der Ordner docs/superpowers.')).toBeTruthy();
    expect(screen.queryByRole('button', { name: 'Board bearbeiten' })).toBeNull();
  });

  it('zeigt einen Leerzustand, wenn es Specs geben könnte, aber keine da sind', async () => {
    open('/p/p1/docs', { getDocs: async () => listing([]) });
    expect(await screen.findByText('Noch keine Specs')).toBeTruthy();
  });
});

describe('DocsPage – Fortschritt der Spec', () => {
  it('zeigt den Stand aus dem Board zum Pfad der Spec', async () => {
    const getSpecProgress = vi.fn(async () => [{ spec: SPEC, total: 4, done: 1, blocked: 1, waiting: 1, inProgress: 0, open: 1, notNeeded: 0 }]);
    open('/p/p1/docs', { getSpecProgress });
    expect(await screen.findByRole('img', { name: (n) => n.endsWith('1 von 4 erledigt, 1 blockiert, 1 wartet, 0 in Arbeit, 1 offen') })).toBeTruthy();
    expect(getSpecProgress).toHaveBeenCalledWith('p1');
  });

  it('zeigt ohne passende Tickets keinen Balken', async () => {
    open('/p/p1/docs');
    await screen.findByRole('tree', { name: 'Dokumente' });
    expect(screen.queryByText(/erledigt/)).toBeNull();
  });
});

// Rendert die Docs-Seite mit Live-Verbindung, damit docs-changed ankommt
function openLive(getDocs: () => Promise<ReturnType<typeof listing>>) {
  const api = fakeApi({ getDocs, getDocComments: async () => [], getDoc: async (_pid: string, p: string) => ({ path: p, content: contents[p] ?? '' }) });
  render(
    <ApiProvider api={api}>
      <MemoryRouter initialEntries={['/p/p1/docs']}>
        <LiveProvider pid="p1">
          <Routes>
            <Route path="/p/:pid/docs/*" element={<DocsPage />} />
          </Routes>
        </LiveProvider>
      </MemoryRouter>
    </ApiProvider>,
  );
}

describe('DocsPage – Live-Aktualisierung und Badge „neu“', () => {
  it('lädt bei docs-changed neu und zeigt die hinzugekommene Spec', async () => {
    let list = docs;
    const getDocs = vi.fn(async () => listing(list));
    openLive(getDocs);
    await screen.findByText('2026-10-22-spec-docs-rework-design.md');
    expect(getDocs).toHaveBeenCalledTimes(1);
    list = [neu, ...docs];
    FakeEventSource.current().emit({ type: 'docs-changed' });
    expect(await screen.findByText('2026-10-23-ganz-neu-design.md')).toBeTruthy();
    expect(getDocs).toHaveBeenCalledTimes(2);
  });

  it('trägt ohne gemerkten Stand an jedem Eintrag „neu“', async () => {
    open();
    await screen.findByRole('tree', { name: 'Dokumente' });
    // Alle fünf Dateien; offene Ordner tragen das Badge nicht
    expect(screen.getAllByText('neu')).toHaveLength(5);
  });

  it('trägt bei aktuellem gemerktem Stand nirgends „neu“', async () => {
    window.localStorage.setItem('harness.docsSeen.p1', '1000');
    open();
    await screen.findByRole('tree', { name: 'Dokumente' });
    expect(screen.queryByText('neu')).toBeNull();
  });

  it('kennzeichnet nach docs-changed genau den neuen Eintrag', async () => {
    window.localStorage.setItem('harness.docsSeen.p1', '400');
    let list = docs;
    openLive(async () => listing(list));
    await screen.findByText('2026-10-22-spec-docs-rework-design.md');
    expect(screen.queryByText('neu')).toBeNull();
    list = [neu, ...docs];
    FakeEventSource.current().emit({ type: 'docs-changed' });
    await screen.findByText('2026-10-23-ganz-neu-design.md');
    expect(within(node(/^2026-10-23-ganz-neu-design\.md/)).getByText('neu')).toBeTruthy();
    expect(screen.getAllByText('neu')).toHaveLength(1);
  });
});

// Stand einer Spec aus dem Board; nicht genannte Felder sind 0
const progressOf = (spec: string, extra: Partial<DbSpecProgressDto> = {}): DbSpecProgressDto => ({
  spec, total: 0, done: 0, blocked: 0, waiting: 0, inProgress: 0, open: 0, notNeeded: 0, ...extra,
});

describe('DocsPage – Fortschritt der Dateien', () => {
  it('zeigt den Balken unter der Datei der Spec und lässt Dateien ohne Tickets ohne Balken', async () => {
    window.localStorage.setItem('harness.docsSeen.p1', '1000');
    open('/p/p1/docs', { getSpecProgress: async () => [progressOf(SPEC, { total: 3, done: 1, open: 2 })] });
    const bar = await screen.findByRole('img', { name: (n) => n.endsWith('1 von 3 erledigt, 0 blockiert, 0 wartet, 0 in Arbeit, 2 offen') });
    expect(node('2026-10-22-spec-docs-rework-design.md').contains(bar)).toBe(true);
    expect(within(node('2026-09-01-alt-design.md')).queryByRole('img')).toBeNull();
  });

  it('zeigt den Balken auch unter einem Plan, zu dem es keine Spec gibt', async () => {
    const nurPlan: DocFileDto = {
      path: 'docs/superpowers/plans/2026-10-20-nur-plan-plan.md',
      title: 'Nur Plan – Plan', group: 'plan', kind: 'plan', topic: 'nur-plan', date: '2026-10-20', mtimeMs: 150, exists: true,
    };
    open('/p/p1/docs', {
      getDocs: async () => listing([...docs, nurPlan]),
      getSpecProgress: async () => [progressOf(nurPlan.path, { total: 2, done: 2 })],
    });
    const bar = await screen.findByRole('img', { name: (n) => n.endsWith('2 von 2 erledigt, 0 blockiert, 0 wartet, 0 in Arbeit, 0 offen') });
    expect(bar.closest('li')?.textContent).toContain('2026-10-20-nur-plan-plan.md');
  });
});

describe('DocsPage – Board-Umschalter', () => {
  const FREMD = 'docs/superpowers/specs/2026-10-05-fremd-design.md';
  const fremd: DocFileDto = { path: FREMD, title: 'Fremdes Thema – Design', group: 'spec', kind: 'spec', topic: 'fremd', date: '2026-10-05', mtimeMs: 10, exists: true };
  const boards = dbBoards.map((b) => ({ ...b, repoPath: b.id === 2 ? null : `C:/repo${b.id}` }));
  // Board 1 ist das des Ordners, Board 3 liefert ein eigenes Repo
  const getDocs = vi.fn(async (_pid: string, board?: number) => (board === 3
    ? { source: { root: 'C:/other', boardId: 3, boardName: 'Fremd', reason: null }, docs: [fremd] }
    : listing()));

  it('zeigt nur Boards mit Repo, startet beim Board des Ordners und schaltet weitere Boards dazu', async () => {
    getDocs.mockClear();
    open('/p/p1/docs', { getDocs, getDbBoards: async () => boards });
    const trigger = await screen.findByRole('button', { name: 'Boards: Sprint' });
    await userEvent.click(trigger);
    const switcher = within(screen.getByRole('group', { name: 'Boards wählen' }));
    expect(switcher.getAllByRole('checkbox').map((b) => b.closest('label')?.querySelector('span')?.textContent)).toEqual(['Alle Boards', 'Sprint', 'Fremd']);
    const sprint = switcher.getByRole('checkbox', { name: /Sprint/ }) as HTMLInputElement;
    // Das letzte gewählte Board lässt sich nicht abwählen
    expect(sprint.checked).toBe(true);
    expect(sprint.disabled).toBe(true);
    expect(screen.queryByText('2026-10-05-fremd-design.md')).toBeNull();

    await userEvent.click(switcher.getByRole('checkbox', { name: /Fremd/ }));
    expect(await screen.findByText('2026-10-05-fremd-design.md')).toBeTruthy();
    expect(getDocs).toHaveBeenCalledWith('p1', 3);
    // Jedes Board steht als oberster Ordner; Dateien des fremden Boards verlinken mit ?board=
    const tree = within(screen.getByRole('tree', { name: 'Dokumente' }));
    const roots = tree.getAllByRole('treeitem').filter((n) => n.getAttribute('aria-level') === '1');
    expect(roots.map((n) => n.getAttribute('aria-label'))).toEqual(['Sprint', 'Fremd']);
    const entry = screen.getByText('2026-10-05-fremd-design.md').closest('li') as HTMLElement;
    expect(roots[1].contains(entry)).toBe(true);
    expect(within(entry).getByRole('link').getAttribute('href')).toBe(`/p/p1/docs/${FREMD}?board=3`);
    // Sind alle Boards einzeln gewählt, gilt das als „Alle Boards“
    expect(screen.getByRole('button', { name: 'Boards: Alle Boards' })).toBeTruthy();
    expect(JSON.parse(window.localStorage.getItem('harness.docsBoards.p1') ?? 'null')).toBe('all');
  });

  it('zeigt mit „Alle Boards“ alle Boards und wählt per Häkchen einzelne ab', async () => {
    open('/p/p1/docs', { getDocs, getDbBoards: async () => boards });
    await userEvent.click(await screen.findByRole('button', { name: 'Boards: Sprint' }));
    const switcher = within(screen.getByRole('group', { name: 'Boards wählen' }));
    await userEvent.click(switcher.getByRole('checkbox', { name: 'Alle Boards' }));
    expect(await screen.findByText('2026-10-05-fremd-design.md')).toBeTruthy();
    expect(screen.getByText('2026-10-22-spec-docs-rework-design.md')).toBeTruthy();

    await userEvent.click(switcher.getByRole('checkbox', { name: /Sprint/ }));
    await screen.findByText('2026-10-05-fremd-design.md');
    expect(screen.queryByText('2026-10-22-spec-docs-rework-design.md')).toBeNull();
    expect(screen.getByRole('button', { name: 'Boards: Fremd' })).toBeTruthy();
    // Escape schließt die Liste
    await userEvent.keyboard('{Escape}');
    expect(screen.queryByRole('group', { name: 'Boards wählen' })).toBeNull();
  });

  it('öffnet eine Datei eines fremden Boards mit dessen Board-ID', async () => {
    const getDoc = vi.fn(async (_pid: string, p: string) => ({ path: p, content: '# Fremd\n\nText', hash: 'h', sections: [] }));
    open(`/p/p1/docs/${FREMD}?board=3`, { getDocs, getDoc, getDbBoards: async () => boards });
    await screen.findByRole('tree', { name: 'Dokumente' });
    expect(getDoc).toHaveBeenCalledWith('p1', FREMD, 3);
  });
});
