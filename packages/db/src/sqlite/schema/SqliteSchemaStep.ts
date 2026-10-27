/** Eine Schema-Stufe der SQLite-Datei; der SqliteSchemaRunner führt sie genau einmal in einer Transaktion aus */
export interface SqliteSchemaStep {
  /** Eindeutiger Name, gleich dem Dateinamen ohne Endung (z. B. '001_initial') */
  id: string;
  /** Schema-Version, die der Runner nach der Stufe in SchemaVersion einträgt (Stufen 001–003) */
  version?: number;
  /** Additive Erweiterung, die der Runner nach der Stufe in SchemaFeatures einträgt (z. B. 'dispatch-runs') */
  feature?: string;
  /** DDL, Anweisungen mit ; getrennt; ohne BEGIN/COMMIT und ohne Einträge in SchemaVersion/SchemaFeatures */
  sql: string;
}
