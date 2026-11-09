import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join, relative, sep } from 'node:path';
import { describe, expect, it } from 'vitest';

const SRC = join(import.meta.dirname, '..', 'src');
// Hier darf ein Theme sich selbst beim Namen nennen
const ALLOWED = [join('components', 'win95') + sep, 'themes' + sep, 'theme' + sep];
const SWITCH = /theme\s*[!=]==\s*'(?!light'|dark')[a-z0-9-]+'/;

// Alle .ts/.tsx-Dateien unter einem Ordner
function sources(dir: string): string[] {
  return readdirSync(dir).flatMap((name) => {
    const path = join(dir, name);
    if (statSync(path).isDirectory()) return sources(path);
    return /\.tsx?$/.test(name) ? [path] : [];
  });
}

describe('Theme-Weichen', () => {
  it('fragt außerhalb der Themes kein Theme beim Namen ab, sondern nutzt Slots', () => {
    const offenders = sources(SRC)
      .map((path) => relative(SRC, path))
      .filter((rel) => !ALLOWED.some((prefix) => rel.startsWith(prefix)))
      .filter((rel) => SWITCH.test(readFileSync(join(SRC, rel), 'utf8')));
    expect(offenders).toEqual([]);
  });
});
