import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { REQUIRED_SCHEMA_FEATURES, SchemaVersionGuard } from './SchemaVersionGuard.js';

// Nur das, was der Runner vom Verbindungspool braucht – so lässt er sich ohne SQL Server prüfen
export interface SchemaBatchRunner {
  request(): { batch(text: string): Promise<unknown> };
}

// „Erweiterung ist schon da“ – zwei Sessions haben gleichzeitig eingespielt, das Ergebnis stimmt trotzdem
const ALREADY_APPLIED = 50002;
// „Vorbedingung fehlt“ – das vorige Skript ist noch nicht eingespielt
const PRECONDITION = 50001;
// Fehlendes ALTER/CREATE-Recht auf dem Schema board (SQL Server meldet je nach Anweisung eine andere Nummer)
const PERMISSION = new Set([229, 230, 262, 2760, 15247]);

// Spielt fehlende additive Erweiterungen (004, 005, …) selbst ein, wenn der Dienst-Login DDL auf board darf.
// Der Verbindungsaufbau darf daran nie scheitern: jeder Fehler wird gemeldet, nicht geworfen – fehlt die Erweiterung
// danach weiter, verweigert sich nur der Zugriff, der sie wirklich braucht.
export class MssqlSchemaRunner {
  constructor(
    private readonly pool: SchemaBatchRunner,
    private readonly warn: (message: string) => void = (m) => process.stderr.write(`${m}\n`),
  ) {}

  // Spielt die fehlenden Erweiterungen in Skript-Reihenfolge ein und liefert die eingespielten Namen
  async apply(missing: string[]): Promise<string[]> {
    const applied: string[] = [];
    for (const feature of Object.keys(REQUIRED_SCHEMA_FEATURES)) {
      if (!missing.includes(feature)) continue;
      const text = this.read(feature);
      if (text === null) continue;
      if (await this.run(feature, text)) applied.push(feature);
    }
    return applied;
  }

  // Skripttext aus packages/db/sql; aus dist/ wie aus src/ liegt der Ordner eine Ebene höher
  private read(feature: string): string | null {
    try {
      const text = readFileSync(fileURLToPath(new URL(`../sql/${SchemaVersionGuard.script(feature)}`, import.meta.url)), 'utf8');
      // Die Skripte enden mit genau einem GO; mssql führt immer einen einzelnen Batch aus und kennt GO nicht
      return text.replace(/^\s*GO\s*$/m, '');
    } catch {
      return null;
    }
  }

  // true, wenn die Erweiterung danach eingespielt ist
  private async run(feature: string, text: string): Promise<boolean> {
    try {
      await this.pool.request().batch(text);
      return true;
    } catch (error) {
      const number = (error as { number?: number }).number;
      if (number === ALREADY_APPLIED) return true;
      const message = (error as Error).message ?? '';
      if ((number !== undefined && PERMISSION.has(number)) || /permission|Berechtigung/i.test(message)) {
        this.warn(`Erweiterung ${feature} konnte nicht eingespielt werden, es fehlt das Recht ALTER auf dem Schema board. Skript packages/db/sql/${SchemaVersionGuard.script(feature)} als Administrator ausführen.`);
        return false;
      }
      this.warn(number === PRECONDITION
        ? `Erweiterung ${feature} konnte nicht eingespielt werden, eine Vorbedingung fehlt: ${message}`
        : `Erweiterung ${feature} konnte nicht eingespielt werden: ${message}`);
      return false;
    }
  }
}
