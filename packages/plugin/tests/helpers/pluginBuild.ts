import { spawnSync } from 'node:child_process';
import { existsSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { BUILD_ARTIFACTS } from '../../../../vitest.process.setup.js';

export const ROOT = resolve(__dirname, '../../../..');

// Ausgabeordner des Builds: im Testlauf der lauf-eigene Ordner aus vitest.process.setup.ts,
// sonst (einzelner Aufruf ohne globalSetup) der Vorgabeordner des Repos
export const PLUGIN_OUT = process.env.HARNESS_PLUGIN_OUT ?? join(ROOT, 'packages/plugin/dist/ai-harness');

// Ordner, den es nicht gibt: Build ohne Anpassungen, unabhängig von custom/ im Repo
const NO_CUSTOM = process.env.HARNESS_NO_CUSTOM_DIR ?? join(tmpdir(), `harness-no-custom-dir-${process.pid}`);

// Baut das Plugin (setzt gebautes CLI, Server und Oberfläche voraus; `npm run build`); `customDir` für Tests mit Anpassungen, `env` ergänzt die Umgebung des Builds
export function buildPlugin(customDir: string = NO_CUSTOM, env: NodeJS.ProcessEnv = {}): { ok: boolean; message: string } {
  for (const needed of BUILD_ARTIFACTS) {
    if (!existsSync(join(ROOT, needed))) return { ok: false, message: `Fehlt: ${needed}. Bitte zuerst npm run build ausführen.` };
  }
  const result = spawnSync(process.execPath, [join(ROOT, 'packages/plugin/scripts/build-plugin.mjs')], {
    cwd: ROOT, encoding: 'utf8', windowsHide: true,
    env: { ...process.env, HARNESS_CUSTOM_DIR: customDir, HARNESS_PLUGIN_OUT: PLUGIN_OUT, ...env },
  });
  return { ok: result.status === 0, message: `${result.stdout}${result.stderr}` };
}
