import { readdirSync, readFileSync, statSync } from 'node:fs';
import { dirname, join, relative, resolve, sep } from 'node:path';
import { describe, expect, it } from 'vitest';

const SRC = join(import.meta.dirname, '..', 'src');
// Spezifizierer aus import/export … from, Seiteneffekt-Import und dynamischem import()
const SPECIFIER = /(?:\bfrom\s*|\bimport\s*\(?\s*)['"]([^'"]+)['"]/g;

// Alle .ts/.tsx-Dateien unter einem Ordner
function sources(dir: string): string[] {
  return readdirSync(dir).flatMap((name) => {
    const path = join(dir, name);
    if (statSync(path).isDirectory()) return sources(path);
    return /\.tsx?$/.test(name) ? [path] : [];
  });
}

// Erlaubt sind React, das Theme-SDK und Dateien des Pakets unter src/
export function violations(files: Array<{ path: string; text: string }>): string[] {
  return files.flatMap(({ path, text }) =>
    [...text.matchAll(SPECIFIER)]
      .map((match) => match[1]!)
      .filter((spec) => {
        if (spec === 'react' || spec.startsWith('react/') || spec === '@nexus/theme-sdk') return false;
        if (spec.startsWith('.')) return !resolve(dirname(path), spec).startsWith(SRC + sep);
        return true;
      })
      .map((spec) => `${relative(SRC, path)}: ${spec}`),
  );
}

const FILES = sources(SRC);

describe('Grenze des Win95-Themes', () => {
  it('findet die Dateien des Themes', () => {
    expect(FILES.map((path) => relative(SRC, path))).toContain('main.tsx');
  });

  it('importiert nur React, das Theme-SDK und eigene Dateien', () => {
    expect(violations(FILES.map((path) => ({ path, text: readFileSync(path, 'utf8') })))).toEqual([]);
  });

  it('meldet fremde Importe mit Datei und Import', () => {
    const path = join(SRC, 'Probe.tsx');
    const text = "import { NavLink } from 'react-router-dom';\nimport type { X } from '../../../packages/web/src/components/BoardProjectSwitcher.js';\nimport { Win95Icon } from './Win95Icons.js';\nconst lazy = import('../tests/helper.js');";
    expect(violations([{ path, text }])).toEqual([
      'Probe.tsx: react-router-dom',
      'Probe.tsx: ../../../packages/web/src/components/BoardProjectSwitcher.js',
      'Probe.tsx: ../tests/helper.js',
    ]);
  });
});
