import { SqliteWarningFilter } from './SqliteWarningFilter.js';

// Lädt node:sqlite erst bei Bedarf und nach dem Warnungsfilter. Ein statischer Import würde die ExperimentalWarning
// (Node 22) schon beim Laden des Modulgraphen auslösen – vor jedem Filter. Andere Dateien importieren aus
// 'node:sqlite' deshalb nur Typen.
export class NodeSqlite {
  static load(): typeof import('node:sqlite') {
    SqliteWarningFilter.install();
    return process.getBuiltinModule('node:sqlite');
  }
}
