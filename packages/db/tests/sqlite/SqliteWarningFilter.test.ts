import { describe, it, expect, vi } from 'vitest';
import { spawnSync } from 'node:child_process';
import { readdirSync, readFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { pathToFileURL } from 'node:url';
import { SqliteWarningFilter } from '../../src/sqlite/SqliteWarningFilter.js';

const ROOT = resolve(__dirname, '../../../..');
const TSX_LOADER = pathToFileURL(join(ROOT, 'node_modules/tsx/dist/esm/index.mjs')).href;
const WORKER = join(__dirname, 'helpers/sqliteWarningWorker.ts');

// Startet den Hilfsprozess, der node:sqlite lädt und danach Warnungen ausgibt
function runWorker(...args: string[]) {
  return spawnSync(process.execPath, ['--import', TSX_LOADER, WORKER, ...args], { encoding: 'utf8', windowsHide: true });
}

describe('SqliteWarningFilter', () => {
  it('lädt node:sqlite ohne SQLite-Warnung auf stderr, andere Warnungen bleiben', () => {
    const result = runWorker();
    expect(result.status).toBe(0);
    expect(result.stderr).not.toMatch(/SQLite/);
    expect(result.stderr).toContain('Testwarnung bleibt sichtbar');
    expect(result.stderr).toContain('Andere Warnung bleibt sichtbar');
  });

  it('ohne Filter erscheint die SQLite-Warnung', () => {
    const result = runWorker('ohne-filter');
    expect(result.status).toBe(0);
    expect(result.stderr).toContain('SQLite is an experimental feature');
  });

  it('verwirft nur die SQLite-ExperimentalWarning und hüllt nur einmal ein', () => {
    const target = { emitWarning: vi.fn() };
    const original = target.emitWarning;
    SqliteWarningFilter.install(target);
    SqliteWarningFilter.install(target);

    target.emitWarning('SQLite is an experimental feature', 'ExperimentalWarning');
    target.emitWarning('SQLite is an experimental feature', { type: 'ExperimentalWarning' });
    target.emitWarning(Object.assign(new Error('SQLite is an experimental feature'), { name: 'ExperimentalWarning' }));
    expect(original).not.toHaveBeenCalled();

    target.emitWarning('x', 'ExperimentalWarning');
    target.emitWarning('SQLite veraltet', 'DeprecationWarning');
    expect(original).toHaveBeenCalledTimes(2);
  });

  it('kein Quellcode importiert Werte aus node:sqlite', () => {
    const offenders: string[] = [];
    for (const pkg of readdirSync(join(ROOT, 'packages'), { withFileTypes: true })) {
      if (!pkg.isDirectory()) continue;
      const src = join(ROOT, 'packages', pkg.name, 'src');
      let files: string[];
      try {
        files = readdirSync(src, { recursive: true, encoding: 'utf8' });
      } catch {
        continue;
      }
      for (const file of files) {
        if (!file.endsWith('.ts')) continue;
        const path = join(src, file);
        for (const line of readFileSync(path, 'utf8').split('\n')) {
          if (/^\s*import\s+(?!type\b)[^;]*from\s+'node:sqlite'/.test(line)) offenders.push(`${path}: ${line.trim()}`);
        }
      }
    }
    expect(offenders).toEqual([]);
  });
});
