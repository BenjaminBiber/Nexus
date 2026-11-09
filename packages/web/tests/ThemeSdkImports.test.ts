import { existsSync, readdirSync, readFileSync, statSync } from 'node:fs';
import { join, relative } from 'node:path';
import { describe, expect, it } from 'vitest';

const WEB = join(import.meta.dirname, '..');
// Import oder Re-Export, der auf die alte Datei theme/slots zeigt
const OLD_IMPORT = /from\s+['"][^'"]*theme\/slots(\.js)?['"]|from\s+['"]\.\/slots(\.js)?['"]/;

// Alle .ts/.tsx-Dateien unter einem Ordner
function sources(dir: string): string[] {
  return readdirSync(dir).flatMap((name) => {
    const path = join(dir, name);
    if (statSync(path).isDirectory()) return sources(path);
    return /\.tsx?$/.test(name) ? [path] : [];
  });
}

describe('Slot-Typen aus dem Theme-SDK', () => {
  it('hat theme/slots.ts nicht mehr', () => {
    expect(existsSync(join(WEB, 'src', 'theme', 'slots.ts'))).toBe(false);
  });

  it('importiert nirgends mehr aus theme/slots, sondern aus @nexus/theme-sdk', () => {
    const offenders = [...sources(join(WEB, 'src')), ...sources(join(WEB, 'tests'))]
      .filter((path) => !path.endsWith('ThemeSdkImports.test.ts'))
      .filter((path) => OLD_IMPORT.test(readFileSync(path, 'utf8')))
      .map((path) => relative(WEB, path));
    expect(offenders).toEqual([]);
  });

  it('führt @nexus/theme-sdk als Abhängigkeit', () => {
    const pkg = JSON.parse(readFileSync(join(WEB, 'package.json'), 'utf8')) as { dependencies: Record<string, string> };
    expect(pkg.dependencies).toHaveProperty('@nexus/theme-sdk');
  });
});
