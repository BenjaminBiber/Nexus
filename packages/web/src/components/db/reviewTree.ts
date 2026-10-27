import type { DispatchReviewFileDto } from '../../api/dispatchTypes.js';

/** Ordner im Dateibaum des Reviews; `name` kann bei Ketten mit nur einem Unterordner mehrere Ebenen enthalten („src/dispatch“) */
export interface ReviewDir {
  kind: 'dir';
  name: string;
  /** Voller Pfad des Ordners */
  path: string;
  children: ReviewNode[];
  /** Dateien darunter (alle Ebenen) und ihre Zeilen */
  files: number;
  additions: number;
  deletions: number;
}

export interface ReviewLeaf {
  kind: 'file';
  name: string;
  file: DispatchReviewFileDto;
}

export type ReviewNode = ReviewDir | ReviewLeaf;

// Baut aus der Dateiliste einen Baum: Ordner zuerst, alphabetisch; Ordner mit genau einem Unterordner und ohne Dateien
// werden zusammengefasst („packages/server/src“)
export function reviewTree(files: DispatchReviewFileDto[]): ReviewNode[] {
  const root: ReviewDir = { kind: 'dir', name: '', path: '', children: [], files: 0, additions: 0, deletions: 0 };
  for (const file of files) {
    const parts = file.path.split('/');
    let dir = root;
    for (const part of parts.slice(0, -1)) {
      const path = dir.path ? `${dir.path}/${part}` : part;
      let next = dir.children.find((c): c is ReviewDir => c.kind === 'dir' && c.path === path);
      if (!next) {
        next = { kind: 'dir', name: part, path, children: [], files: 0, additions: 0, deletions: 0 };
        dir.children.push(next);
      }
      dir = next;
    }
    dir.children.push({ kind: 'file', name: parts[parts.length - 1], file });
  }
  return finish(root).children;
}

// Zählt, sortiert und fasst Ketten zusammen
function finish(dir: ReviewDir): ReviewDir {
  dir.children = dir.children.map((c) => (c.kind === 'dir' ? collapse(finish(c)) : c));
  dir.children.sort((a, b) => (a.kind === b.kind ? a.name.localeCompare(b.name) : a.kind === 'dir' ? -1 : 1));
  for (const c of dir.children) {
    dir.files += c.kind === 'dir' ? c.files : 1;
    dir.additions += c.kind === 'dir' ? c.additions : c.file.additions;
    dir.deletions += c.kind === 'dir' ? c.deletions : c.file.deletions;
  }
  return dir;
}

// Ordner, der nur einen Unterordner enthält, mit diesem zusammenlegen
function collapse(dir: ReviewDir): ReviewDir {
  const only = dir.children.length === 1 ? dir.children[0] : null;
  if (only?.kind !== 'dir') return dir;
  return { ...only, name: `${dir.name}/${only.name}` };
}
