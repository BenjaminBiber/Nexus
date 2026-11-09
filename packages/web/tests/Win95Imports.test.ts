import { readdirSync, readFileSync, statSync } from 'node:fs';
import { dirname, join, relative, resolve, sep } from 'node:path';
import { describe, expect, it } from 'vitest';

const SRC = join(import.meta.dirname, '..', 'src');
const WIN95_DIR = join(SRC, 'components', 'win95');
const THEMES = join(SRC, 'themes');
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

// Liegt die Datei im Win95-Theme?
function isWin95(path: string): boolean {
  return path.startsWith(WIN95_DIR + sep) || (dirname(path) === THEMES && /^win95/.test(relative(THEMES, path))) || path.startsWith(join(THEMES, 'win95') + sep);
}

// Erlaubt sind React, das Theme-SDK und Dateien des Win95-Themes
export function violations(files: Array<{ path: string; text: string }>): string[] {
  return files.flatMap(({ path, text }) =>
    [...text.matchAll(SPECIFIER)]
      .map((match) => match[1]!)
      .filter((spec) => {
        if (spec === 'react' || spec.startsWith('react/') || spec === '@nexus/theme-sdk') return false;
        if (spec.startsWith('.')) return !isWin95(resolve(dirname(path), spec));
        return true;
      })
      .map((spec) => `${relative(SRC, path)}: ${spec}`),
  );
}

const WIN95_FILES = [...sources(WIN95_DIR), ...sources(THEMES).filter(isWin95)];

describe('Grenze des Win95-Themes', () => {
  it('findet die Dateien des Themes', () => {
    expect(WIN95_FILES.map((path) => relative(SRC, path))).toContain(join('themes', 'win95.tsx'));
  });

  it('importiert nur React, das Theme-SDK und eigene Dateien', () => {
    expect(violations(WIN95_FILES.map((path) => ({ path, text: readFileSync(path, 'utf8') })))).toEqual([]);
  });

  it('meldet fremde Importe mit Datei und Import', () => {
    const path = join(WIN95_DIR, 'Probe.tsx');
    const text = "import { NavLink } from 'react-router-dom';\nimport type { X } from '../BoardProjectSwitcher.js';\nimport { Win95Icon } from './Win95Icons.js';\nconst lazy = import('../../api/client.js');";
    expect(violations([{ path, text }])).toEqual([
      `${join('components', 'win95', 'Probe.tsx')}: react-router-dom`,
      `${join('components', 'win95', 'Probe.tsx')}: ../BoardProjectSwitcher.js`,
      `${join('components', 'win95', 'Probe.tsx')}: ../../api/client.js`,
    ]);
  });
});
