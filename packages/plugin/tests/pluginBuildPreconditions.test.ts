import { describe, expect, it } from 'vitest';
import { join } from 'node:path';
import { BUILD_ARTIFACTS } from '../../../vitest.globalSetup.js';
import { PLUGIN_OUT, ROOT } from './helpers/pluginBuild.js';

// Vorbedingungen des Plugin-Baus: das Artefakt, das er braucht, und der lauf-eigene Ordner,
// in den er schreibt.
describe('Vorbedingungen des Plugin-Baus', () => {
  it('nennt die gebaute Oberfläche als einziges Artefakt', () => {
    expect(BUILD_ARTIFACTS).toEqual(['packages/web/dist/index.html']);
  });

  it('baut das Plugin in einen lauf-eigenen Ordner, nicht in den geteilten Repo-Ordner', () => {
    expect(process.env.NEXUS_PLUGIN_RUN_DIR, 'globalSetup hat die Umgebung nicht an die Testdateien gereicht').toBeTruthy();
    expect(PLUGIN_OUT.startsWith(join(ROOT, 'packages/plugin/dist'))).toBe(false);
  });
});
