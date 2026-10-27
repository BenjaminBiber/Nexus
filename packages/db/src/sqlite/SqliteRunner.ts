import type { DatabaseSync, SQLInputValue } from 'node:sqlite';
import { HarnessError } from '@harness/core';

/** Werte, die SqliteRunner binden kann (SqlValue aus SqlRunner plus undefined) */
export type SqliteValue = string | number | boolean | Date | null | undefined;

// Benannte Parameter @name im SQL-Text
const PARAM = /@([A-Za-z_][A-Za-z0-9_]*)/g;
// Anweisungen, die Zeilen ändern; auch hinter einem WITH
const WRITE = /^\s*(INSERT|UPDATE|DELETE|REPLACE)\b/i;
const WITH_WRITE = /^\s*WITH\b[\s\S]*\b(INSERT|UPDATE|DELETE|REPLACE)\b/i;
const RETURNING = /\bRETURNING\b/i;
// Kommentare und Text in Anführungszeichen (dort zählen ; und @ nicht)
const NOISE = /--[^\n]*|\/\*[\s\S]*?\*\/|'(?:[^']|'')*'|"(?:[^"]|"")*"/g;

// Führt parametrisierte Abfragen auf einer node:sqlite-Verbindung aus; Gegenstück zu SqlRunner.
// Je Aufruf genau eine Anweisung (Mehranweisungs-Batches der MSSQL-Repositories in mehrere Aufrufe zerlegen).
export class SqliteRunner {
  constructor(private readonly db: DatabaseSync) {}

  // Bindet die Parameter (@name) und liefert Zeilen und Zahl der betroffenen Zeilen
  async query<T>(text: string, params: Record<string, SqliteValue> = {}): Promise<{ rows: T[]; affected: number }> {
    const plain = SqliteRunner.plain(text);
    if (/;\s*\S/.test(plain)) {
      throw new HarnessError('sqlite-multi-statement', 'SqliteRunner führt je Aufruf nur eine Anweisung aus; Batches in einzelne query-Aufrufe zerlegen.');
    }
    const statement = this.db.prepare(text);
    // Unbekannte Parameter erlauben (die MSSQL-Repositories geben oft mehr mit, als eine Anweisung braucht).
    // Die Methode gibt es erst ab Node 22.15; für 22.13/22.14 bindet bind() zusätzlich nur die benutzten Namen.
    if (typeof statement.setAllowUnknownNamedParameters === 'function') statement.setAllowUnknownNamedParameters(true);
    const bound = SqliteRunner.bind(plain, params);
    // Ändernde Anweisung ohne RETURNING: keine Zeilen, Zahl der geänderten Zeilen (ohne Trigger-Änderungen)
    if ((WRITE.test(plain) || WITH_WRITE.test(plain)) && !RETURNING.test(plain)) {
      const result = statement.run(bound);
      return { rows: [], affected: Number(result.changes) };
    }
    // node:sqlite liefert Objekte ohne Prototyp; als normale Objekte weitergeben
    const rows = statement.all(bound).map((row) => ({ ...row }) as unknown as T);
    return { rows, affected: rows.length };
  }

  // Nur die im SQL benutzten Parameter binden (node:sqlite lehnt unbekannte Namen ab); Werte umwandeln
  private static bind(plain: string, params: Record<string, SqliteValue>): Record<string, SQLInputValue> {
    const bound: Record<string, SQLInputValue> = {};
    for (const match of plain.matchAll(PARAM)) {
      const name = match[1];
      if (name in params) bound[name] = SqliteRunner.toSqlite(params[name]);
    }
    return bound;
  }

  // boolean → 0/1, Date → ISO-Text mit Millisekunden (wie datetime2(3)), undefined → NULL
  private static toSqlite(value: SqliteValue): SQLInputValue {
    if (value === undefined) return null;
    if (typeof value === 'boolean') return value ? 1 : 0;
    if (value instanceof Date) return value.toISOString();
    return value;
  }

  // SQL ohne Kommentare und ohne Inhalt von '…' und "…" (nur zum Erkennen von Anweisungsart, Parametern und ;)
  private static plain(text: string): string {
    return text.replace(NOISE, (m) => (m.startsWith("'") || m.startsWith('"') ? m[0] + m[0] : ' '));
  }

  // 0/1 aus einer INTEGER-Spalte als boolean; null wird false
  static bool(value: number | bigint | boolean | null): boolean {
    return value !== null && Number(value) !== 0;
  }

  // ISO-Text als Date (wie SqlRunner.date); null bleibt null
  static date(iso: string | null): Date | null {
    return iso === null ? null : new Date(iso);
  }

  // Datumstext aus der DB als ISO-Text mit Millisekunden und Z; null bleibt null
  static iso(value: string | null): string | null {
    return value === null ? null : new Date(value).toISOString();
  }

  // GUIDs gelten im Harness kleingeschrieben (wie SqlRunner.guid)
  static guid(value: string): string {
    return value.toLowerCase();
  }
}
