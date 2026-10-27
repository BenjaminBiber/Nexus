import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterAll, describe } from 'vitest';
import { SqliteBoardDatabase } from '../../src/sqlite/SqliteBoardDatabase.js';
import { dispatchRunContract } from '../../../core/tests/unit/board/helpers/DispatchRunContract.js';

// Ein Ordner je Testdatei, eine frische SQLite-Datei je Test
const dir = mkdtempSync(join(tmpdir(), 'harness-sqlite-dispatch-'));
let count = 0;
const create = async () => SqliteBoardDatabase.open(join(dir, `board-${++count}.db`));
afterAll(() => rmSync(dir, { recursive: true, force: true, maxRetries: 5, retryDelay: 100 }));

describe('SQLite', () => {
  dispatchRunContract('SQLite', create);
});
