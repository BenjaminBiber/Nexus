import { spawnSync } from 'node:child_process';
import { existsSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { BUILD_ARTIFACTS } from '../../../../vitest.process.setup.js';

export const ROOT = resolve(__dirname, '../../../..');

// Ausgabeordner des Builds: im Testlauf der lauf-eigene Ordner aus vitest.process.setup.ts,
// sonst (einzelner Aufruf ohne globalSetup) der Vorgabeordner des Repos
export const PLUGIN_OUT = process.env.NEXUS_PLUGIN_OUT ?? join(ROOT, 'packages/plugin/dist/nexus');

// Name des Binaries im gebauten Plugin (unter Windows mit Endung)
export const BINARY = process.platform === 'win32' ? 'nexus.exe' : 'nexus';

// Das Binary des gebauten Plugins: Hooks und `bin/harness` rufen genau diese Datei auf
export const PLUGIN_BIN = join(PLUGIN_OUT, 'bin', BINARY);

// Ordner, den es nicht gibt: Build ohne Anpassungen, unabhängig von custom/ im Repo
const NO_CUSTOM = process.env.NEXUS_NO_CUSTOM_DIR ?? join(tmpdir(), `nexus-no-custom-dir-${process.pid}`);

// Baut das Plugin (setzt gebauten Server und Oberfläche voraus; `npm run build`, dazu cargo für das
// Binary); `customDir` für Tests mit Anpassungen, `env` ergänzt die Umgebung des Builds
export function buildPlugin(customDir: string = NO_CUSTOM, env: NodeJS.ProcessEnv = {}): { ok: boolean; message: string } {
  for (const needed of BUILD_ARTIFACTS) {
    if (!existsSync(join(ROOT, needed))) return { ok: false, message: `Fehlt: ${needed}. Bitte zuerst npm run build ausführen.` };
  }
  const result = spawnSync(process.execPath, [join(ROOT, 'packages/plugin/scripts/build-plugin.mjs')], {
    cwd: ROOT, encoding: 'utf8', windowsHide: true,
    env: { ...process.env, NEXUS_CUSTOM_DIR: customDir, NEXUS_PLUGIN_OUT: PLUGIN_OUT, ...env },
  });
  return { ok: result.status === 0, message: `${result.stdout}${result.stderr}` };
}
