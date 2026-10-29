import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

// Build-Artefakte, ohne die der Plugin-Build nicht läuft.
// Einzige Liste im Repo: `packages/plugin/tests/helpers/pluginBuild.ts` holt sie von hier.
export const BUILD_ARTIFACTS = ['packages/web/dist/index.html'];

// Ordner dieses Laufs; wird am Ende wieder entfernt
let laufOrdner: string | null = null;

// globalSetup: gibt allen Testdateien einen lauf-eigenen Ordner für den Plugin-Bau mit.
// So kann weder `npm run build:plugin` noch eine Claude-Session den Tests den geteilten
// Ordner packages/plugin/dist/nexus unter den Füßen wegziehen.
export async function setup(): Promise<void> {
  laufOrdner = mkdtempSync(join(tmpdir(), 'harness-pluginlauf-'));
  process.env.NEXUS_PLUGIN_RUN_DIR = laufOrdner;
  process.env.NEXUS_NO_CUSTOM_DIR = join(laufOrdner, 'keine-anpassungen');
}

// globalSetup: räumt den Ordner dieses Laufs wieder weg
export async function teardown(): Promise<void> {
  if (!laufOrdner) return;
  rmSync(laufOrdner, { recursive: true, force: true });
  laufOrdner = null;
}
