import { useEffect, useMemo, useState, type CSSProperties } from 'react';
import { useParams, useSearchParams } from 'react-router-dom';
import { useApi } from '../api/ApiContext.js';
import type { DocFileDto, DocsSourceReason } from '../api/types.js';
import { loadDocsSeen, saveDocsSeen } from '../board/docsSeen.js';
import { buildDocTree, countLabel, DOC_KIND_LABELS, docKey, filterDocTree, kindsOf, type DocKindTag } from '../board/docsTree.js';
import { loadDocsBoards, loadDocsWidths, saveDocsBoards, saveDocsWidths, type DocsBoardSelection } from '../board/docsTreeOpen.js';
import { notifyOpenBoardAdmin } from '../dbboard/boardAdmin.js';
import { DocTree } from '../components/DocTree.js';
import { DocViewer } from '../components/DocViewer.js';
import { boardLabel, DocsBoardSwitcher } from '../components/DocsBoardSwitcher.js';
import { DocsGutter, DocsToolsContext } from '../components/DocsGutter.js';
import { EmptyState } from '../components/EmptyState.js';
import { ErrorState } from '../components/ErrorState.js';
import { LoadingState } from '../components/LoadingState.js';
import { SearchResults } from '../components/SearchResults.js';
import { useAsync } from '../hooks/useAsync.js';
import { useDocTicketJobs } from '../hooks/docTicketJobs.js';
import { useBoardDocLists, useDocList, useDocSearch } from '../hooks/useDocs.js';

// Erklärung, warum es keine Docs-Quelle gibt
const REASONS: Record<DocsSourceReason, string> = {
  'no-board': 'Für dieses Projekt gibt es kein Board.',
  'no-repo-path': 'Dem Board ist kein Repo-Pfad zugeordnet.',
  'missing-folder': 'Im Repo fehlt der Ordner docs/superpowers.',
};

// Gründe, die sich im Dialog „Board bearbeiten“ beheben lassen
const FIXABLE: DocsSourceReason[] = ['no-board', 'no-repo-path'];

// Grenzen der ziehbaren Spalten in Pixeln
const LIMITS = { explorer: { min: 200, max: 520 }, tools: { min: 220, max: 520 } };

// Docs: Superpowers-Specs mit ihren Plänen, Glossar und ADRs lesen und durchsuchen (nur lesend).
// Die Seite ist eine Werkbank wie ein Code-Editor: links der Explorer mit dem Dateibaum, in der Mitte das Dokument,
// rechts die Werkzeuge (Inhaltsverzeichnis, Kommentare, Tickets) – die rechte Spalte liefert der DocViewer.
// Neue oder geänderte Dateien kommen über die Live-Meldung „docs-changed“ und tragen das Badge „neu“, bis die Seite
// den Stand gemerkt hat; der gemerkte Stand wird beim Betreten eingefroren, damit das Badge beim Lesen sichtbar bleibt.
// Der Board-Umschalter im Explorer zeigt statt der Docs des Ordner-Boards die Docs aller oder ausgewählter Boards (aus deren Repos);
// Boards, die dasselbe Repo teilen, liefern ihre Dateien nur einmal. Dateien fremder Boards öffnen mit ?board=.
export function DocsPage() {
  const api = useApi();
  const { pid = '', '*': splat } = useParams();
  const [params] = useSearchParams();
  const [query, setQuery] = useState('');
  const [kind, setKind] = useState<DocKindTag | 'all'>('all');
  const docs = useDocList(pid);
  const path = splat ? decodeURIComponent(splat) : null;
  const boardParam = Number(params.get('board'));
  const openBoard = Number.isInteger(boardParam) && boardParam > 0 ? boardParam : undefined;
  const term = query.trim().length >= 2 ? query.trim() : (params.get('q') ?? '');
  const source = docs.state.status === 'ready' ? docs.state.data.source : null;
  // Boards mit Repo-Pfad stehen im Umschalter; ohne Board-Datenbank bleibt er weg
  const dbBoards = useAsync(() => api.getDbBoards().catch(() => []), [api]);
  const boards = dbBoards.state.status === 'ready' ? dbBoards.state.data.filter((b) => b.repoPath?.trim()) : [];
  const [selection, setSelection] = useState<DocsBoardSelection | null>(() => loadDocsBoards(pid));
  const home = source?.boardId ?? null;
  // Gezeigte Boards: alle, die gewählten oder (ohne Wahl) das Board des Ordners
  const shown = selection === 'all' ? boards.map((b) => b.id) : selection ?? (home === null ? [] : [home]);
  const withHome = home !== null && shown.includes(home);
  const foreignIds = useMemo(() => shown.filter((id) => id !== home), [shown.join(','), home]); // eslint-disable-line react-hooks/exhaustive-deps
  const foreign = useBoardDocLists(pid, foreignIds);
  const foreignLists = foreign.state.status === 'ready' ? foreign.state.data : [];
  // Dateien des Ordner-Boards ohne Board-Kennung (alte Adressen bleiben gültig), danach die fremden Boards; ein Repo nur einmal
  const merged = useMemo(() => {
    // Namen wie im Umschalter, damit die Board-Ordner im Explorer gleich heißen
    const nameOf = (id: number | null | undefined, fallback?: string) => {
      const b = boards.find((x) => x.id === id);
      return b ? boardLabel(boards, b) : fallback;
    };
    const own: DocFileDto[] = withHome && docs.state.status === 'ready'
      ? docs.state.data.docs.map((d) => ({ ...d, boardName: nameOf(home, source?.boardName ?? undefined) }))
      : [];
    const roots = new Set(withHome && source?.root ? [source.root] : []);
    const lists = foreignLists.filter((l) => {
      if (!l.source?.root) return l.docs.length > 0;
      if (roots.has(l.source.root)) return false;
      roots.add(l.source.root);
      return true;
    });
    return { docs: [...own, ...lists.flatMap((l) => l.docs.map((d) => ({ ...d, boardName: nameOf(l.boardId, d.boardName) })))], boards: lists.map((l) => l.boardId) };
  }, [docs.state, foreignLists, withHome, source, boards, home]);
  const list = merged.docs;
  // Schlüssel der offenen Datei; ?board= mit dem Board des Ordners ist dieselbe Datei wie ohne
  const current = path === null ? null : docKey({ path, board: openBoard === home ? undefined : openBoard });
  // Repo-Ordner der offenen Datei: der des Ordner-Boards, bei ?board= der des fremden Boards (für das Öffnen in der Desktop-App)
  const openRoot = openBoard === undefined || openBoard === home
    ? source?.root ?? null
    : foreignLists.find((l) => l.boardId === openBoard)?.source?.root ?? null;
  // Dateien, zu denen Claude gerade Ticket-Entwürfe schreibt (im Baum pulsiert dort das Claude-Symbol)
  const { list: ticketJobs } = useDocTicketJobs();
  const jobKeyOf = (j: { path: string; board?: number }) => docKey({ path: j.path, board: j.board === home ? undefined : j.board });
  const busy = useMemo(() => new Set(ticketJobs.filter((j) => j.pid === pid && j.running).map(jobKeyOf)), [ticketJobs, pid, home]); // eslint-disable-line react-hooks/exhaustive-deps
  // Dateien mit gespeicherten Ticket-Entwürfen: Stand der Liste, überholt von dem, was diese Sitzung gespeichert oder verworfen hat
  const draftKeys = useMemo(() => {
    const keys = new Set(list.filter((d) => d.hasDrafts).map(docKey));
    for (const j of ticketJobs.filter((x) => x.pid === pid)) {
      if (j.saved) keys.add(jobKeyOf(j));
      else keys.delete(jobKeyOf(j));
    }
    return keys;
  }, [list, ticketJobs, pid, home]); // eslint-disable-line react-hooks/exhaustive-deps
  const [onlyDrafts, setOnlyDrafts] = useState(false);
  const hits = useDocSearch(pid, query, [...(withHome ? [undefined] : []), ...merged.boards]);
  const [seenMs] = useState(() => loadDocsSeen(pid));
  const newest = list.reduce((max, d) => Math.max(max, d.mtimeMs), 0);
  const [widths, setWidths] = useState(() => loadDocsWidths(pid) ?? { explorer: 340, tools: 300 });
  // Schmale Fenster zeigen nur eine Spalte; dieser Wert sagt, welche Seitenspalte darüber liegt
  const [panel, setPanel] = useState<'none' | 'explorer' | 'tools'>('none');
  // Der Explorer filtert seine Titel mit derselben Eingabe, die auch den Inhalt durchsucht
  const filter = useMemo(() => ({ query, kind, drafts: onlyDrafts ? draftKeys : undefined }), [query, kind, onlyDrafts, draftKeys]);
  // Das Board steht als oberster Ordner über plans/specs, sobald sein Name bekannt ist (auch bei nur einem Board)
  const byBoard = list.some((d) => d.boardName !== undefined);
  const tree = useMemo(() => buildDocTree(list, { byBoard }), [list, byBoard]);
  const kinds = useMemo(() => kindsOf(tree), [tree]);
  const count = useMemo(() => countLabel(tree, filterDocTree(tree, filter)), [tree, filter]);
  const tools = useMemo(() => ({ width: widths.tools, setWidth: (w: number) => setWidths((prev) => ({ ...prev, tools: w })) }), [widths.tools]);

  // Was die Seite gezeigt hat, gilt als gesehen (der Zähler am Menüpunkt fällt damit auf 0)
  useEffect(() => {
    if (newest > 0) saveDocsSeen(pid, newest);
  }, [pid, newest]);

  // Die Board-Auswahl überlebt ein Neuladen; die Wahl nur des Ordner-Boards ist der Standard und wird vergessen
  useEffect(() => {
    saveDocsBoards(pid, Array.isArray(selection) && selection.length === 1 && selection[0] === home ? null : selection);
  }, [pid, selection, home]);

  // Die gezogenen Spaltenbreiten überleben ein Neuladen
  useEffect(() => { saveDocsWidths(pid, widths); }, [pid, widths]);

  // Ein Board schaltet sich dazu oder weg (das letzte bleibt); sind danach alle gewählt, gilt wieder „Alle“, damit neue Boards dazukommen
  const toggleBoard = (id: number) => {
    const next = shown.includes(id) ? shown.filter((b) => b !== id) : [...shown, id];
    if (next.length === 0) return;
    setSelection(boards.every((b) => next.includes(b.id)) ? 'all' : next);
  };
  // „Alle“ an zeigt jedes Board; wieder aus fällt auf das Board des Ordners (sonst das erste) zurück
  const toggleAll = () => setSelection(selection === 'all' ? [home ?? boards[0].id] : 'all');
  const switcher = boards.length > 1 && (
    <DocsBoardSwitcher boards={boards} shown={shown} all={selection === 'all'} onAll={toggleAll} onToggle={toggleBoard} />
  );

  if (docs.state.status === 'loading') return <div className="page"><LoadingState text="Dokumente werden geladen …" /></div>;
  if (docs.state.status === 'error') return <ErrorState message={docs.state.message} onRetry={docs.retry} />;
  if (source?.reason && foreignIds.length === 0) {
    const reason = source.reason;
    return (
      <div className="page">
        {switcher}
        <EmptyState title="Keine Dokumente erreichbar">
          <span>{REASONS[reason]}</span>
          {FIXABLE.includes(reason) && (
            <>
              {' '}
              <button type="button" className="link-button" onClick={notifyOpenBoardAdmin}>
                Board bearbeiten
              </button>
            </>
          )}
        </EmptyState>
      </div>
    );
  }
  if (list.length === 0) {
    if (foreign.state.status === 'loading') return <div className="page">{switcher}<LoadingState text="Dokumente werden geladen …" /></div>;
    return (
      <div className="page">
        {switcher}
        <EmptyState title="Noch keine Specs">/superpowers:brainstorming legt die erste unter docs/superpowers/specs an.</EmptyState>
      </div>
    );
  }

  // Im schmalen Fenster blendet derselbe Knopf die Spalte wieder aus
  const togglePanel = (which: 'explorer' | 'tools') => setPanel((prev) => (prev === which ? 'none' : which));

  return (
    <DocsToolsContext.Provider value={tools}>
      <div
        className="docs-workbench"
        data-panel={panel}
        style={{ '--docs-explorer-w': `${widths.explorer}px`, '--docs-tools-w': `${widths.tools}px` } as CSSProperties}
      >
        <aside className="docs-explorer" aria-label="Explorer">
          <div className="docs-explorer-head">
            <div className="docs-explorer-title">
              <h1>Docs</h1>
              <span className="docs-explorer-count">{count}</span>
            </div>
            {switcher}
            <label className="search">
              <span className="sr-only">In Docs suchen</span>
              <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" aria-hidden="true">
                <circle cx="7" cy="7" r="4.5" />
                <path d="M10.5 10.5L14 14" />
              </svg>
              <input type="search" value={query} onChange={(e) => setQuery(e.target.value)} placeholder="Titel filtern, Inhalt durchsuchen …" />
              {query && (
                <button type="button" className="search-clear" aria-label="Suche leeren" onClick={() => setQuery('')}>
                  <svg width="10" height="10" viewBox="0 0 10 10" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" aria-hidden="true">
                    <path d="M2 2l6 6M8 2L2 8" />
                  </svg>
                </button>
              )}
            </label>
            {(kinds.length > 1 || draftKeys.size > 0 || onlyDrafts) && (
              <div className="docs-kinds" role="group" aria-label="Art filtern">
                {kinds.length > 1 && (['all', ...kinds] as const).map((k) => (
                  <button key={k} type="button" className={`fchip${kind === k ? ' on' : ''}`} aria-pressed={kind === k} onClick={() => setKind(k)}>
                    {k === 'all' ? 'Alle' : DOC_KIND_LABELS[k]}
                  </button>
                ))}
                {/* Zusätzlich zur Art: nur Dateien, zu denen Ticket-Entwürfe gespeichert sind */}
                {(draftKeys.size > 0 || onlyDrafts) && (
                  <button type="button" className={`fchip${onlyDrafts ? ' on' : ''}`} aria-pressed={onlyDrafts} onClick={() => setOnlyDrafts((v) => !v)}>
                    <span className="material-icons" aria-hidden="true">edit_note</span>
                    Entwürfe vorhanden
                  </button>
                )}
              </div>
            )}
            {hits !== null && <span className="muted">{hits.length} Treffer im Inhalt</span>}
          </div>
          <div className="docs-explorer-tree">
            <DocTree docs={list} pid={pid} current={current} seenMs={seenMs} filter={filter} showBoards={byBoard} busy={busy} />
          </div>
        </aside>
        <DocsGutter
          label="Breite des Explorers"
          width={widths.explorer}
          min={LIMITS.explorer.min}
          max={LIMITS.explorer.max}
          sign={1}
          onChange={(w) => setWidths((prev) => ({ ...prev, explorer: w }))}
        />
        <section className="docs-main">
          <div className="docs-bar">
            <button type="button" className="btn small" aria-pressed={panel === 'explorer'} onClick={() => togglePanel('explorer')}>Dateien</button>
            <button type="button" className="btn small" aria-pressed={panel === 'tools'} onClick={() => togglePanel('tools')}>Werkzeuge</button>
          </div>
          {hits !== null && (
            <div className="panel">
              <SearchResults hits={hits} pid={pid} term={query.trim()} />
            </div>
          )}
          {path ? (
            <DocViewer
              key={`${pid}:${current}`}
              pid={pid}
              path={path}
              root={openRoot}
              board={openBoard}
              doc={list.find((d) => docKey(d) === current)}
              term={term}
            />
          ) : (
            <EmptyState title="Wähle links ein Dokument." />
          )}
        </section>
      </div>
    </DocsToolsContext.Provider>
  );
}
