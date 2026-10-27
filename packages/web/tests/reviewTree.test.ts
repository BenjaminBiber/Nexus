import { describe, expect, it } from 'vitest';
import { reviewTree, type ReviewDir } from '../src/components/db/reviewTree.js';

// Geänderte Datei mit Zeilen
const f = (path: string, additions = 1, deletions = 0) => ({ path, status: 'M', additions, deletions, binary: false });

describe('reviewTree', () => {
  it('baut Ordner mit Summen, fasst Ketten zusammen und sortiert Ordner vor Dateien', () => {
    const tree = reviewTree([
      f('packages/server/src/a.ts', 3, 1), f('packages/web/src/b.tsx', 2), f('packages/web/src/c.tsx'), f('CHANGELOG.md', 5),
    ]);
    expect(tree.map((n) => n.name)).toEqual(['packages', 'CHANGELOG.md']);
    const packages = tree[0] as ReviewDir;
    expect(packages).toMatchObject({ files: 3, additions: 6, deletions: 1 });
    expect(packages.children.map((n) => n.name)).toEqual(['server/src', 'web/src']);
    expect((packages.children[1] as ReviewDir).children.map((n) => n.name)).toEqual(['b.tsx', 'c.tsx']);
  });

  it('fasst einen einzigen Pfad bis zur Datei zusammen', () => {
    const [dir] = reviewTree([f('a/b/c/d.ts')]);
    expect(dir).toMatchObject({ kind: 'dir', name: 'a/b/c', path: 'a/b/c', files: 1 });
  });
});
