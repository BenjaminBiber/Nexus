type WarningTarget = { emitWarning: (...args: any[]) => void };
const INSTALLED = Symbol.for('harness.sqliteWarningFilter');

// Unterdrückt genau die ExperimentalWarning von node:sqlite (Node 22/23), damit Hook-Ausgaben und --json sauber bleiben;
// alle anderen Warnungen gehen unverändert an Node weiter.
export class SqliteWarningFilter {
  // Hängt den Filter einmalig vor emitWarning; weitere Aufrufe tun nichts
  static install(target: WarningTarget = process): void {
    const marked = target as WarningTarget & { [INSTALLED]?: true };
    if (marked[INSTALLED]) return;
    const original = target.emitWarning;
    target.emitWarning = function (this: unknown, ...args: unknown[]) {
      if (SqliteWarningFilter.isSqliteWarning(args[0], args[1])) return;
      return original.apply(this ?? target, args);
    };
    marked[INSTALLED] = true;
  }

  // ExperimentalWarning, deren Text SQLite nennt – als Text mit Typ, mit Optionsobjekt { type } oder als Error-Objekt
  static isSqliteWarning(warning: unknown, typeOrOptions: unknown): boolean {
    const message = typeof warning === 'string' ? warning : warning instanceof Error ? warning.message : '';
    const type = warning instanceof Error
      ? warning.name
      : typeof typeOrOptions === 'string' ? typeOrOptions : (typeOrOptions as { type?: unknown } | null | undefined)?.type;
    return type === 'ExperimentalWarning' && message.includes('SQLite');
  }
}
