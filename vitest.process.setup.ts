import { existsSync, mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

// Build-Artefakte, ohne die weder der Plugin-Build noch die Prozess-Suite laufen können.
// Einzige Liste im Repo: `packages/plugin/tests/helpers/pluginBuild.ts` holt sie von hier.
export const BUILD_ARTIFACTS = ['packages/cli/dist/main.js', 'packages/server/dist/main.js', 'packages/web/dist/index.html'];

// Verbindungsdaten der Test-DB aus HARNESS_TEST_DB (Teilmenge von BoardConfig)
type TestDbConfig = { server: string; port?: number; database: string; user: string; password: string; encrypt?: boolean; trustServerCertificate?: boolean };

const ROOT = dirname(fileURLToPath(import.meta.url));

// Vorbedingungen eines Prozess-Testlaufs: fehlt etwas, bricht der Lauf mit einer einzigen Meldung ab,
// statt in vielen Dateien gleichzeitig rot zu werden.
export class ProcessPreflight {
  // Build-Artefakte, die unter `root` fehlen (`exists` nur für Tests austauschbar)
  static missingBuildArtifacts(root: string, exists: (pfad: string) => boolean = existsSync): string[] {
    return BUILD_ARTIFACTS.filter((artefakt) => !exists(join(root, artefakt)));
  }

  // Grund, warum die Test-DB nicht nutzbar ist, oder null (ohne HARNESS_TEST_DB immer null)
  static async unreachableTestDb(
    raw: string | undefined,
    connect: (config: TestDbConfig) => Promise<unknown> = ProcessPreflight.connect,
  ): Promise<string | null> {
    if (!raw) return null;
    let config: TestDbConfig;
    try {
      config = JSON.parse(raw) as TestDbConfig;
    } catch (fehler) {
      return `HARNESS_TEST_DB ist kein gültiges JSON: ${ProcessPreflight.text(fehler)}`;
    }
    if (!config?.database) return 'HARNESS_TEST_DB enthält keine Datenbank (Feld "database").';
    try {
      await connect(config);
      return null;
    } catch (fehler) {
      return `Test-DB ${config.database} auf ${config.server}:${config.port ?? 1433} ist nicht erreichbar: ${ProcessPreflight.text(fehler)}`;
    }
  }

  // Eine kurze Verbindung zur Test-DB; wird nur bei gesetztem HARNESS_TEST_DB geladen
  private static async connect(config: TestDbConfig): Promise<void> {
    const { default: sql } = await import('mssql');
    const pool = await new sql.ConnectionPool({
      server: config.server, port: config.port ?? 1433, database: config.database, user: config.user, password: config.password,
      options: { encrypt: config.encrypt ?? true, trustServerCertificate: config.trustServerCertificate ?? true },
      connectionTimeout: 5000, requestTimeout: 5000,
    }).connect();
    try {
      await pool.request().query('SELECT 1');
    } finally {
      await pool.close();
    }
  }

  // Text eines Fehlers für Menschen
  private static text(fehler: unknown): string {
    return fehler instanceof Error ? fehler.message : String(fehler);
  }
}

// Ordner dieses Laufs; wird am Ende wieder entfernt
let laufOrdner: string | null = null;

// globalSetup: prüft die Vorbedingungen und gibt allen Testdateien lauf-eigene Ordner mit
export async function setup(): Promise<void> {
  const fehlend = ProcessPreflight.missingBuildArtifacts(ROOT);
  if (fehlend.length) throw new Error(`Fehlt: ${fehlend.join(', ')} – bitte zuerst "npm run build" ausführen.`);
  const dbGrund = await ProcessPreflight.unreachableTestDb(process.env.HARNESS_TEST_DB);
  if (dbGrund) throw new Error(dbGrund);
  // Eigener Plugin-Ausgabeordner je Lauf: weder `npm run build:plugin` noch eine Claude-Session
  // können den Tests den geteilten Ordner packages/plugin/dist/nexus unter den Füßen wegziehen.
  laufOrdner = mkdtempSync(join(tmpdir(), 'harness-prozesslauf-'));
  process.env.NEXUS_PLUGIN_OUT = join(laufOrdner, 'nexus');
  process.env.NEXUS_NO_CUSTOM_DIR = join(laufOrdner, 'keine-anpassungen');
}

// globalSetup: räumt den Ordner dieses Laufs wieder weg
export async function teardown(): Promise<void> {
  if (!laufOrdner) return;
  rmSync(laufOrdner, { recursive: true, force: true });
  laufOrdner = null;
}
