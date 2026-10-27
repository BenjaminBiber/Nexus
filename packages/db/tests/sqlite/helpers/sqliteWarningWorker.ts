import { NodeSqlite } from '../../../src/sqlite/NodeSqlite.js';
import { SqliteWarningFilter } from '../../../src/sqlite/SqliteWarningFilter.js';

// Hilfsprozess für SqliteWarningFilter.test.ts: Argument „ohne-filter“ lässt den Filter weg
const withFilter = process.argv[2] !== 'ohne-filter';
if (withFilter) SqliteWarningFilter.install();
const sqlite = withFilter ? NodeSqlite.load() : process.getBuiltinModule('node:sqlite');
const db = new sqlite.DatabaseSync(':memory:');
db.prepare('SELECT 1 AS x').get();
db.close();
// Node 24 meldet beim Laden nichts mehr; die Warnung von Node 22 deshalb nachstellen
process.emitWarning('SQLite is an experimental feature and might change at any time', 'ExperimentalWarning');
process.emitWarning('Testwarnung bleibt sichtbar', 'ExperimentalWarning');
process.emitWarning('Andere Warnung bleibt sichtbar');
