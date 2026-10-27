import type { DocFileDto } from '../api/types.js';

// Eindeutiger Schlüssel einer Datei: der Pfad, bei Dateien aus fremden Boards mit deren ID davor („3:docs/…“).
// So kollidiert derselbe Pfad aus zwei Repos nicht, und Dateien aus dem Board des Ordners behalten ihre alten Schlüssel.
export const docKey = (doc: { path: string; board?: number }) => (doc.board === undefined ? doc.path : `${doc.board}:${doc.path}`);

// Adresse einer Datei auf der Docs-Seite; fremde Boards stehen als ?board= dahinter
export const docHref = (pid: string, doc: { path: string; board?: number }) =>
  `/p/${pid}/docs/${doc.path}${doc.board === undefined ? '' : `?board=${doc.board}`}`;

// Art eines Themas, gelesen am Ende seines Titels („… — Design-Spec“, „… — Mini-Spec“, „Design: …“, „… – Plan“)
export type DocKindTag = 'spec' | 'mini' | 'design' | 'plan' | 'other';

// Beschriftung der Arten für Filter und Tooltip
export const DOC_KIND_LABELS: Record<DocKindTag, string> = {
  spec: 'Design-Spec',
  mini: 'Mini-Spec',
  design: 'Design',
  plan: 'Plan',
  other: 'Sonstige',
};

// Erkennungsmuster der Arten; das erste passende gewinnt und fällt aus dem Titel heraus
const KIND_PATTERNS: Array<[DocKindTag, RegExp]> = [
  ['spec', /\s+[—–-]\s*Design-Spec$|\s*\(Design-Spec\)$/],
  ['mini', /\s+[—–-]\s*Mini-Spec$/],
  ['design', /\s+[—–-]\s*Design$|^Design:\s*/],
  ['plan', /\s+[—–-]\s*(?:Implementation\s+)?Plan$/i],
];

// Trennt die Art vom Titel; ohne bekanntes Suffix bleibt der Titel ganz und die Art ist „other“
export function splitDocTitle(title: string): { title: string; kind: DocKindTag } {
  for (const [kind, pattern] of KIND_PATTERNS) {
    const rest = title.replace(pattern, '').trim();
    if (rest !== title.trim() && rest.length > 0) return { title: rest, kind };
  }
  return { title, kind: 'other' };
}

// Sorte eines Knotens: Ordner (aus dem Pfad, bei mehreren Boards auch das Board als oberster Ordner) oder Datei
export type DocTreeKind = 'group' | 'file';

// Knoten des Explorers; `doc` ist die Datei, die ein Klick öffnet (bei Ordnern keine).
// `title` ist der Titel aus dem Dokument für den Tooltip, `tag` die Art einer Datei (für Filter und Symbol).
export interface DocTreeNode {
  id: string;
  label: string;
  kind: DocTreeKind;
  doc?: DocFileDto;
  title?: string;
  tag?: DocKindTag;
  mtimeMs: number;
  children: DocTreeNode[];
}

// Art einer Datei: Pläne sind immer „plan“, sonst entscheidet das Ende des Titels; Glossar und ADRs haben keine
function tagOf(doc: DocFileDto): DocKindTag | undefined {
  if (doc.group === 'plan') return 'plan';
  if (doc.group !== 'spec') return undefined;
  const { kind } = splitDocTitle(doc.title);
  return kind === 'plan' ? 'other' : kind;
}

// Ordner und Dateien wie im Explorer von VS Code: Ordner zuerst, dann Dateien, je alphabetisch (Zahlen natürlich sortiert)
const byExplorer = (a: DocTreeNode, b: DocTreeNode) =>
  (a.kind === b.kind ? 0 : a.kind === 'group' ? -1 : 1) || a.label.localeCompare(b.label, 'de', { numeric: true, sensitivity: 'base' });

// Löst Hüllordner auf, die nur Ordner enthalten („docs“, „superpowers“): ihre Ordner rücken eine Ebene hoch.
// Gleichnamige Ordner, die dabei nebeneinander landen, werden zu einem zusammengelegt.
function hoist(nodes: DocTreeNode[]): DocTreeNode[] {
  const merged: DocTreeNode[] = [];
  const lifted = nodes.flatMap((n) => (n.kind === 'group' && n.children.length > 0 && n.children.every((c) => c.kind === 'group') ? n.children : [n]));
  for (const node of lifted) {
    const twin = node.kind === 'group' ? merged.find((m) => m.kind === 'group' && m.label === node.label) : undefined;
    if (!twin) merged.push(node);
    else Object.assign(twin, { children: [...twin.children, ...node.children].sort(byExplorer), mtimeMs: Math.max(twin.mtimeMs, node.mtimeMs) });
  }
  return merged.sort(byExplorer);
}

// Hängt die Dateien unter ihre Ordner (ohne Hüllordner, siehe hoist); `prefix` macht die IDs je Board eindeutig.
// Die jüngste Änderung reicht von den Dateien nach oben, damit ein zugeklappter Ordner das Badge „neu“ trägt.
function folderTree(docs: DocFileDto[], prefix: string): DocTreeNode[] {
  const root: DocTreeNode = { id: prefix, label: '', kind: 'group', mtimeMs: 0, children: [] };
  for (const doc of docs) {
    const parts = doc.path.split('/');
    let here = root;
    parts.slice(0, -1).forEach((part, i) => {
      const id = `dir:${prefix}${parts.slice(0, i + 1).join('/')}`;
      let next = here.children.find((c) => c.id === id);
      if (!next) {
        next = { id, label: part, kind: 'group', mtimeMs: 0, children: [] };
        here.children.push(next);
      }
      here = next;
    });
    here.children.push({ id: `file:${docKey(doc)}`, label: parts[parts.length - 1], kind: 'file', doc, title: doc.title, tag: tagOf(doc), mtimeMs: doc.mtimeMs, children: [] });
  }
  const finish = (node: DocTreeNode): DocTreeNode => {
    const children = hoist(node.children.map(finish));
    return { ...node, children, mtimeMs: children.reduce((max, c) => Math.max(max, c.mtimeMs), node.mtimeMs) };
  };
  return finish(root).children;
}

// Baut aus der Dateiliste den Explorer-Baum nach den Ordnern der Pfade. Mit `byBoard` steht jedes Board als oberster Ordner
// (Dateien des Ordner-Boards ohne `board` zuerst); leere Ordner gibt es nicht, weil Ordner nur aus Dateipfaden entstehen.
export function buildDocTree(docs: DocFileDto[], { byBoard = false }: { byBoard?: boolean } = {}): DocTreeNode[] {
  if (!byBoard) return folderTree(docs, '');
  // sort() stellt undefined immer ans Ende, daher über -1 als Schlüssel für das Ordner-Board
  const boards = [...new Set(docs.map((d) => d.board ?? -1))].sort((a, b) => a - b).map((b) => (b === -1 ? undefined : b));
  return boards.map((board) => {
    const files = docs.filter((d) => d.board === board);
    const children = folderTree(files, `${board ?? 'home'}:`);
    return {
      id: `board:${board ?? 'home'}`,
      label: files[0]?.boardName ?? 'Board',
      kind: 'group',
      mtimeMs: children.reduce((max, c) => Math.max(max, c.mtimeMs), 0),
      children,
    };
  });
}

// Filter des Explorers: Teil des Dateinamens oder Titels (ohne Groß/Klein) und Art; „all“ lässt jede Art durch.
// `drafts` (Schlüssel wie docKey) lässt nur Dateien mit gespeicherten Ticket-Entwürfen durch.
export interface DocTreeFilter {
  query: string;
  kind: DocKindTag | 'all';
  drafts?: ReadonlySet<string>;
}

// Alle Dateien unter den Knoten
const filesOf = (tree: DocTreeNode[]): DocTreeNode[] => tree.flatMap((n) => (n.kind === 'file' ? [n] : filesOf(n.children)));

// Wendet den Filter an; Ordner ohne Treffer entfallen. Glossar und ADRs haben keine Art und fallen bei einem Art-Filter weg.
export function filterDocTree(tree: DocTreeNode[], { query, kind, drafts }: DocTreeFilter): DocTreeNode[] {
  const q = query.trim().toLowerCase();
  if (!q && kind === 'all' && !drafts) return tree;
  const hit = (n: DocTreeNode) => (kind === 'all' || n.tag === kind)
    && (!drafts || (n.doc !== undefined && drafts.has(docKey(n.doc))))
    && (!q || n.label.toLowerCase().includes(q) || (n.title ?? '').toLowerCase().includes(q));
  const walk = (nodes: DocTreeNode[]): DocTreeNode[] => nodes.flatMap((n) => {
    if (n.kind === 'file') return hit(n) ? [n] : [];
    const children = walk(n.children);
    return children.length > 0 ? [{ ...n, children }] : [];
  });
  return walk(tree);
}

// Zahl der Dateien im Kopf des Explorers: „24 Docs“ oder gefiltert „3 von 24“
export function countLabel(all: DocTreeNode[], shown: DocTreeNode[]): string {
  const total = filesOf(all).length;
  const visible = filesOf(shown).length;
  return visible === total ? `${total} Docs` : `${visible} von ${total}`;
}

// Arten, die unter den Dateien vorkommen, in fester Reihenfolge
export function kindsOf(tree: DocTreeNode[]): DocKindTag[] {
  const present = new Set(filesOf(tree).map((n) => n.tag));
  return (Object.keys(DOC_KIND_LABELS) as DocKindTag[]).filter((k) => present.has(k));
}

// Die Ordner über einer Datei, von oben nach unten; leer, wenn der Schlüssel (siehe docKey) nicht im Baum steht
export function ancestorIds(tree: DocTreeNode[], key: string | null): string[] {
  if (key === null) return [];
  const found: string[] = [];
  const walk = (node: DocTreeNode, trail: string[]): boolean => {
    if (node.children.length === 0) return node.doc !== undefined && docKey(node.doc) === key;
    const here = [...trail, node.id];
    if (!node.children.some((c) => walk(c, here))) return false;
    found.push(...here);
    return true;
  };
  tree.forEach((n) => walk(n, []));
  return [...new Set(found)];
}

// Standard-Aufklappung: Ordner offen
export const openByDefault = (node: DocTreeNode) => node.kind === 'group';
