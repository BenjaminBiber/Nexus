import { HarnessError } from '@harness/core';

// Schema-Version, die dieser Stand des Harness erwartet (packages/db/sql/00N_….sql)
export const EXPECTED_SCHEMA_VERSION = 3;

// Additive Erweiterungen (board.SchemaFeatures), die dieser Stand verlangt, mit ihrem Skript. Sie erhöhen die Version nicht,
// damit ältere Stände mit der erweiterten DB weiterlaufen.
export const REQUIRED_SCHEMA_FEATURES: Readonly<Record<string, string>> = {
  'dispatch-runs': '004_dispatch_runs.sql',
  'note-structure': '005_note_structure.sql',
  'dispatch-run-details': '006_dispatch_run_details.sql',
  'target-branch': '007_target_branch.sql',
};

// Prüft die Schema-Version der DB. Bei MSSQL spielt der Nutzer Versionsskripte von Hand ein, bei SQLite legt der Harness
// sie beim Öffnen selbst an – die Meldungen unterscheiden sich deshalb je Anbieter, die Prüfung und die Fehlercodes nicht.
export class SchemaVersionGuard {
  // Wirft mit Hinweis auf das passende Skript (mssql) bzw. auf die Datei (sqlite), wenn die Version nicht passt.
  // Fehlende Erweiterungen werfen nicht, sondern werden zurückgegeben: sie sind additiv, die übrigen Befehle laufen
  // weiter und nur der Zugriff, der die neuen Spalten braucht, verweigert sich (siehe featureMessage).
  // database ist bei SQLite der Dateipfad.
  check(actual: number | null, database: string, features: string[], provider: 'mssql' | 'sqlite' = 'mssql'): string[] {
    const sqlite = provider === 'sqlite';
    if (actual === null) {
      throw new HarnessError('board-schema-missing', sqlite
        ? `In der SQLite-Datei ${database} fehlt das Board-Schema, obwohl der Harness es beim Öffnen selbst anlegt. Die Datei ist vermutlich keine Board-Datei – unter Einstellungen → Board-Datenbank einen anderen Pfad wählen.`
        : `In der Datenbank ${database} fehlt das Schema „board“. Skript packages/db/sql/001_initial.sql ausführen.`);
    }
    if (actual < EXPECTED_SCHEMA_VERSION) {
      const next = String(actual + 1).padStart(3, '0');
      throw new HarnessError('board-schema-outdated', sqlite
        ? `SQLite-Datei ${database} hat Schema-Version ${actual}, erwartet ${EXPECTED_SCHEMA_VERSION}. Der Harness spielt fehlende Stufen beim Öffnen selbst ein – Befehl bzw. Server neu starten; bleibt die Meldung, ist die Datei beschädigt.`
        : `Datenbank ${database} hat Schema-Version ${actual}, erwartet ${EXPECTED_SCHEMA_VERSION}. Skript packages/db/sql/${next}_….sql (und folgende) ausführen.`);
    }
    if (actual > EXPECTED_SCHEMA_VERSION) {
      throw new HarnessError('board-schema-newer', sqlite
        ? `SQLite-Datei ${database} hat Schema-Version ${actual}, dieser Harness kennt nur ${EXPECTED_SCHEMA_VERSION}. Im Harness-Ordner „npm run setup“ ausführen.`
        : `Datenbank ${database} hat Schema-Version ${actual}, dieser Harness kennt nur ${EXPECTED_SCHEMA_VERSION}. Im Harness-Ordner „npm run setup“ ausführen.`);
    }
    return SchemaVersionGuard.missing(features);
  }

  // Verlangte Erweiterungen, die in der DB fehlen – in der Reihenfolge von REQUIRED_SCHEMA_FEATURES (= Reihenfolge der Skripte)
  static missing(features: string[]): string[] {
    return Object.keys(REQUIRED_SCHEMA_FEATURES).filter((name) => !features.includes(name));
  }

  // Skript, das die Erweiterung einspielt; leer bei unbekanntem Namen
  static script(feature: string): string {
    return REQUIRED_SCHEMA_FEATURES[feature] ?? '';
  }

  // Meldung für einen Zugriff, der ohne die Erweiterung nicht geht. database ist bei SQLite der Dateipfad.
  static featureMessage(feature: string, database: string, provider: 'mssql' | 'sqlite' = 'mssql'): string {
    return provider === 'sqlite'
      ? `Dafür fehlt der SQLite-Datei ${database} die Erweiterung ${feature}. Der Harness spielt sie beim Öffnen selbst ein – Befehl bzw. Server neu starten; bleibt die Meldung, ist die Datei beschädigt.`
      : `Dafür fehlt der Datenbank ${database} die Erweiterung ${feature}. Skript packages/db/sql/${SchemaVersionGuard.script(feature)} ausführen (als Administrator). Die übrigen Board-Befehle laufen weiter.`;
  }
}
