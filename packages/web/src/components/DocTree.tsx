import { useEffect, useMemo, useRef, useState, type CSSProperties, type KeyboardEvent } from 'react';
import { Link, useNavigate } from 'react-router-dom';
import { useApi } from '../api/ApiContext.js';
import type { DocFileDto } from '../api/types.js';
import { ancestorIds, buildDocTree, docHref, docKey, filterDocTree, openByDefault, type DocTreeFilter, type DocTreeNode } from '../board/docsTree.js';
import { loadDocsTreeOpen, saveDocsTreeOpen } from '../board/docsTreeOpen.js';
import { useAsync } from '../hooks/useAsync.js';
import { ThemeIcon } from '../theme/ThemeIcon.js';
import { ClaudeIcon } from './NavIcons.js';
import { SpecProgressPie } from './SpecProgressBar.js';

const NO_BUSY: ReadonlySet<string> = new Set();

// Pfeil vor Ordnern; er zeigt nach unten, solange der Ordner offen ist
function Chevron({ open }: { open: boolean }) {
  return (
    <svg className={`doc-tree-chevron${open ? ' open' : ''}`} width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      <path d="M6 4l4 4-4 4" />
    </svg>
  );
}

// Sorte des Dateisymbols: Spec/Design-Dokument, Plan, Glossar, ADR oder sonstiges Markdown
type FileIconKind = 'doc' | 'plan' | 'glossary' | 'adr' | 'other';

const iconKindOf = (doc: DocFileDto | undefined): FileIconKind => {
  if (!doc) return 'other';
  if (doc.group === 'plan') return 'plan';
  if (doc.group === 'glossary') return 'glossary';
  if (doc.group === 'adr') return 'adr';
  return doc.group === 'spec' ? 'doc' : 'other';
};

// Symbol einer Datei wie im Explorer von VS Code, je Sorte mit eigener Form und Farbe (Farben in docs.css);
// der Slot „Icon“ des Themes darf es ersetzen (sonstige Dateien heißen dort `file`)
function FileIcon({ doc }: { doc: DocFileDto | undefined }) {
  const kind = iconKindOf(doc);
  return (
    <ThemeIcon name={kind === 'other' ? 'file' : kind}>
      <StandardFileIcon kind={kind} />
    </ThemeIcon>
  );
}

// Linien-Symbol einer Datei je Sorte
function StandardFileIcon({ kind }: { kind: FileIconKind }) {
  return (
    <svg className={`doc-tree-icon is-${kind}`} width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      {kind === 'plan' && (
        <>
          {/* Klemmbrett mit abgehakten Schritten */}
          <rect x="3" y="2.5" width="10" height="12" rx="1.5" />
          <path d="M6 2.5v-1h4v1" />
          <path d="M5.2 6.6l.9.9 1.6-1.7M5.2 10.6l.9.9 1.6-1.7M9.3 7h1.9M9.3 11h1.9" />
        </>
      )}
      {kind === 'glossary' && (
        <>
          {/* Aufgeschlagenes Buch */}
          <path d="M8 4.2C6.8 3.2 4.9 3 2.5 3.2v9.3c2.4-.2 4.3 0 5.5 1 1.2-1 3.1-1.2 5.5-1V3.2C11.1 3 9.2 3.2 8 4.2z" />
          <path d="M8 4.2v9.3" />
        </>
      )}
      {kind === 'adr' && (
        <>
          {/* Wegweiser: eine getroffene Entscheidung */}
          <path d="M8 1.8v12.7M5.5 14.5h5" />
          <path d="M8 3h4.6l1.4 1.6-1.4 1.6H8zM8 7.8H3.4L2 9.4 3.4 11H8z" />
        </>
      )}
      {(kind === 'doc' || kind === 'other') && (
        <>
          {/* Blatt mit Eselsohr und Zeilen */}
          <path d="M9.5 1.8H4.5a1 1 0 00-1 1v10.4a1 1 0 001 1h7a1 1 0 001-1V4.8z" />
          <path d="M9.5 1.8v3h3" />
          <path d="M5.8 8h4.4M5.8 10.5h4.4" />
        </>
      )}
    </svg>
  );
}

// Dateiname mit hervorgehobenem Filtertreffer (erste Fundstelle, ohne Groß/Klein)
function Highlight({ text, query }: { text: string; query: string }) {
  const q = query.trim().toLowerCase();
  const at = q ? text.toLowerCase().indexOf(q) : -1;
  if (at < 0) return <>{text}</>;
  return (
    <>
      {text.slice(0, at)}
      <mark>{text.slice(at, at + q.length)}</mark>
      {text.slice(at + q.length)}
    </>
  );
}

// Einrückung je Ebene und Abstand der ersten Ebene vom Rand (px); die Hilfslinie eines Ordners steht unter der Mitte seines Pfeils
const STEP = 12;
const BASE = 8;
const ROW_MARGIN = 6;

// Eine Zeile des Baums samt Lage im Baum; `parent` trägt die Tastatur („links“ springt zum Elternknoten)
interface Row {
  node: DocTreeNode;
  level: number;
  parent: string | null;
}

// Alle sichtbaren Zeilen in der Reihenfolge, in der sie auf dem Bildschirm stehen
function rowsOf(nodes: DocTreeNode[], isOpen: (n: DocTreeNode) => boolean, level = 1, parent: string | null = null): Row[] {
  return nodes.flatMap((node) => [
    { node, level, parent },
    ...(node.kind === 'group' && isOpen(node) ? rowsOf(node.children, isOpen, level + 1, node.id) : []),
  ]);
}

const NO_FILTER: DocTreeFilter = { query: '', kind: 'all' };

// Explorer der Docs wie in VS Code: Ordner aus den Dateipfaden (Ordner zuerst, dann Dateien, je alphabetisch), Dateien mit Symbol
// nach Sorte (Spec, Plan, Glossar, ADR) und Hilfslinien unter offenen Ordnern. Zeigt der Explorer mehrere Boards, steht jedes als oberster Ordner.
// Blätter sind Links auf /p/:pid/docs/<pfad> (fremde Boards mit ?board=), damit Deep-Links und der Zurück-Knopf des Browsers weiter gehen.
// `current` ist der Schlüssel der offenen Datei (siehe docKey).
// Gemerkt werden nur zugeklappte Ordner (Standard: offen), damit neue Ordner offen erscheinen.
// Ein Filter klappt alle Ordner mit Treffern auf. Bedienung mit Maus und Tastatur (hoch, runter, rechts, links, Enter).
// `busy` enthält die Schlüssel der Dateien, zu denen Claude gerade Tickets erzeugt; dort pulsiert das Claude-Symbol statt des Dateisymbols.
export function DocTree({ docs, pid, current, seenMs = 0, filter = NO_FILTER, showBoards = false, busy = NO_BUSY }: {
  docs: DocFileDto[];
  pid: string;
  current: string | null;
  seenMs?: number;
  filter?: DocTreeFilter;
  showBoards?: boolean;
  busy?: ReadonlySet<string>;
}) {
  const api = useApi();
  const navigate = useNavigate();
  const full = useMemo(() => buildDocTree(docs, { byBoard: showBoards }), [docs, showBoards]);
  const tree = useMemo(() => filterDocTree(full, filter), [full, filter]);
  const filtering = filter.query.trim().length > 0 || filter.drafts !== undefined;
  // Stand der Specs: aus dem Board des Ordners und aus jedem fremden Board, das Dateien liefert; leer, wenn kein Board passt
  const foreign = [...new Set(docs.map((d) => d.board).filter((b): b is number => b !== undefined))].sort((a, b) => a - b);
  const dbProgress = useAsync(async () => {
    const own = (await api.getSpecProgress(pid).catch(() => [])).map((p) => ({ ...p, board: undefined as number | undefined }));
    const others = await Promise.all(foreign.map((id) => api.getDbSpecProgress(id).then((list) => list.map((p) => ({ ...p, board: id })), () => [])));
    return [...own, ...others.flat()];
  }, [api, pid, foreign.join(',')]);
  const progress = dbProgress.state.status === 'ready' ? dbProgress.state.data : [];
  const [toggled, setToggled] = useState<Set<string>>(() => new Set(loadDocsTreeOpen(pid) ?? []));
  const isOpen = (node: DocTreeNode) => (filtering && node.kind === 'group') || openByDefault(node) !== toggled.has(node.id);
  const rows = rowsOf(tree, isOpen);
  const [focusId, setFocusId] = useState<string | null>(null);
  const items = useRef(new Map<string, HTMLLIElement>());

  // Klappt einen Knoten auf oder zu; gemerkt wird, ob er damit vom Standard abweicht
  const setOpen = (node: DocTreeNode, want: boolean) => setToggled((prev) => {
    const next = new Set(prev);
    if (want !== openByDefault(node)) next.add(node.id);
    else next.delete(node.id);
    return next;
  });
  const toggle = (node: DocTreeNode, want?: boolean) => setOpen(node, want ?? !isOpen(node));

  // Ein von außen geöffnetes Dokument (Deep-Link, Suchtreffer) klappt die Ordner darüber auf
  useEffect(() => {
    const above = ancestorIds(full, current);
    if (above.length === 0) return;
    setToggled((prev) => (above.some((id) => prev.has(id)) ? new Set([...prev].filter((id) => !above.includes(id))) : prev));
  }, [full, current]);

  // Jede Änderung der Aufklappung wird gemerkt
  useEffect(() => { saveDocsTreeOpen(pid, [...toggled]); }, [pid, toggled]);

  // Setzt den Tastatur-Fokus auf eine Zeile
  const focus = (id: string | undefined) => {
    if (!id) return;
    setFocusId(id);
    items.current.get(id)?.focus();
  };

  // Ein Knoten wird ausgewählt: Ordner klappen um, Dateien öffnen ihr Dokument
  const activate = (node: DocTreeNode) => {
    setFocusId(node.id);
    if (node.kind === 'group') toggle(node);
    else if (node.doc) navigate(docHref(pid, node.doc));
  };

  // Tastatur des Baums: hoch und runter bewegen, rechts und links auf und zu, Enter öffnet
  const onKeyDown = (e: KeyboardEvent<HTMLUListElement>) => {
    const index = rows.findIndex((r) => r.node.id === focusId);
    const row = rows[index];
    if (!row) return;
    const expandable = row.node.kind === 'group';
    const keys: Record<string, () => void> = {
      ArrowDown: () => focus(rows[index + 1]?.node.id),
      ArrowUp: () => focus(rows[index - 1]?.node.id),
      Home: () => focus(rows[0]?.node.id),
      End: () => focus(rows[rows.length - 1]?.node.id),
      ArrowRight: () => {
        if (!expandable) return;
        if (!isOpen(row.node)) toggle(row.node, true);
        else focus(rows[index + 1]?.node.id);
      },
      ArrowLeft: () => {
        if (expandable && isOpen(row.node)) toggle(row.node, false);
        else focus(row.parent ?? undefined);
      },
      Enter: () => activate(row.node),
      ' ': () => activate(row.node),
    };
    const run = keys[e.key];
    if (!run) return;
    e.preventDefault();
    run();
  };

  // Der Tastatur-Fokus startet auf der ersten Zeile, damit der Baum mit einem Tab erreichbar ist
  const tabStop = focusId !== null && rows.some((r) => r.node.id === focusId) ? focusId : rows[0]?.node.id;

  // Stand der Tickets zu einer Spec oder einem Plan (Pläne zählen, wenn es zum Thema keine Spec-Datei gibt)
  const progressOf = (node: DocTreeNode) =>
    node.doc ? progress.find((p) => p.spec === node.doc?.path && p.board === node.doc?.board) : undefined;

  // Symbol einer Datei: Claude bei laufender Erzeugung, sonst der Stand der Tickets als Kreisdiagramm, ohne Tickets das Dateisymbol
  const fileIcon = (node: DocTreeNode) => {
    if (node.doc && busy.has(docKey(node.doc))) {
      return <span className="doc-tree-claude" title="Claude erzeugt Tickets" aria-label="Claude erzeugt Tickets" role="img"><ClaudeIcon /></span>;
    }
    const pie = progressOf(node);
    return pie ? <SpecProgressPie progress={pie} /> : <FileIcon doc={node.doc} />;
  };

  // Inhalt einer Zeile wie im Explorer von VS Code. Ordner: Pfeil und Name. Datei: Platz in Pfeilbreite, Symbol, Dateiname.
  // Das Badge „neu“ steht an Dateien und an zugeklappten Ordnern, die neue Dateien verbergen.
  const content = (node: DocTreeNode, isNew: boolean) => (
    <>
      {node.kind === 'group' ? <Chevron open={isOpen(node)} /> : <span className="doc-tree-spacer" aria-hidden="true" />}
      {node.kind === 'file' && fileIcon(node)}
      {/* Ordner haben im Standard kein Symbol; ein Theme kann über den Slot „Icon“ eines zeigen (z. B. den Explorer-Ordner) */}
      {node.kind === 'group' && <ThemeIcon name={isOpen(node) ? 'folderOpen' : 'folder'} />}
      <span className="doc-tree-label">
        {node.kind === 'file' ? <Highlight text={node.label} query={filter.query} /> : node.label}
      </span>
      {/* Im schmalen Explorer reicht ein Punkt; der Text bleibt für Screenreader */}
      {isNew && <span className="doc-tree-new" title="neu"><span className="sr-only">neu</span></span>}
    </>
  );

  const render = (row: Row) => {
    const { node, level } = row;
    const leaf = node.kind === 'file' && node.doc;
    const selected = Boolean(leaf) && node.doc !== undefined && docKey(node.doc) === current;
    const open = isOpen(node);
    const isNew = node.mtimeMs > seenMs && (node.kind === 'file' || !open);
    const indent = BASE + (level - 1) * STEP;
    return (
      <li
        key={node.id}
        ref={(el) => { if (el) items.current.set(node.id, el); else items.current.delete(node.id); }}
        role="treeitem"
        aria-level={level}
        aria-label={isNew ? `${node.label}, neu` : node.label}
        aria-expanded={node.kind === 'group' ? open : undefined}
        aria-selected={leaf ? selected : undefined}
        tabIndex={node.id === tabStop ? 0 : -1}
        className={`doc-tree-item is-${node.kind}`}
        onFocus={(e) => { if (e.target === e.currentTarget) setFocusId(node.id); }}
      >
        {leaf && node.doc ? (
          <Link
            to={docHref(pid, node.doc)}
            tabIndex={-1}
            title={node.title}
            aria-current={selected ? 'page' : undefined}
            className={`doc-tree-row${selected ? ' on' : ''}`}
            style={{ paddingLeft: `${indent}px` }}
            onClick={() => setFocusId(node.id)}
          >
            {content(node, isNew)}
          </Link>
        ) : (
          <span className="doc-tree-row" style={{ paddingLeft: `${indent}px` }} onClick={() => activate(node)}>
            {content(node, isNew)}
          </span>
        )}
        {node.kind === 'group' && open && (
          <ul role="group" className="doc-tree-group" style={{ '--doc-tree-guide': `${ROW_MARGIN + indent + 7.5}px` } as CSSProperties}>
            {node.children.map((child) => render({ node: child, level: level + 1, parent: node.id }))}
          </ul>
        )}
      </li>
    );
  };

  if (tree.length === 0) {
    return <p className="doc-tree-empty">Keine Docs{filtering ? ` für „${filter.query.trim()}“` : ''}.</p>;
  }

  return (
    <ul role="tree" aria-label="Dokumente" className="doc-tree" onKeyDown={onKeyDown}>
      {tree.map((node) => render({ node, level: 1, parent: null }))}
    </ul>
  );
}
