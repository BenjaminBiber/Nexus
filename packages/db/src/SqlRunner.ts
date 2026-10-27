import sql from 'mssql';

export type SqlValue = string | number | boolean | Date | null;

// Führt parametrisierte Abfragen innerhalb einer Transaktion aus.
export class SqlRunner {
  constructor(private readonly tx: sql.Transaction) {}

  // Bindet die Parameter (@name) und liefert Zeilen und Zahl der betroffenen Zeilen; `types` legt einzelne SQL-Typen fest
  // (z. B. bigint oder decimal statt der Ableitung aus dem Wert)
  async query<T>(text: string, params: Record<string, SqlValue> = {}, types: Record<string, sql.ISqlType | (() => sql.ISqlType)> = {}): Promise<{ rows: T[]; affected: number }> {
    const request = new sql.Request(this.tx);
    for (const [name, value] of Object.entries(params)) request.input(name, types[name] ?? this.typeOf(value), value);
    const result = await request.query<T>(text);
    return { rows: result.recordset ?? [], affected: result.rowsAffected.reduce((sum, n) => sum + n, 0) };
  }

  // SQL-Typ zum Wert; Texte immer als nvarchar(max), damit Umlaute erhalten bleiben
  private typeOf(value: SqlValue): sql.ISqlType | (() => sql.ISqlType) {
    if (typeof value === 'number') return Number.isInteger(value) ? sql.Int : sql.Float;
    if (typeof value === 'boolean') return sql.Bit;
    if (value instanceof Date) return sql.DateTime2(3);
    return sql.NVarChar(sql.MAX);
  }

  // ISO-Text als Date für datetime2-Spalten; null bleibt null
  static date(iso: string | null): Date | null {
    return iso === null ? null : new Date(iso);
  }

  // datetime2 aus der DB als ISO-Text; null bleibt null
  static iso(value: Date | null): string | null {
    return value === null ? null : value.toISOString();
  }

  // uniqueidentifier liefert SQL Server in Großbuchstaben; im Harness gilt Kleinschreibung
  static guid(value: string): string {
    return value.toLowerCase();
  }
}
