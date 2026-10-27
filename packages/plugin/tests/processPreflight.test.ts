import { describe, expect, it } from 'vitest';
import { join } from 'node:path';
import { BUILD_ARTIFACTS, ProcessPreflight } from '../../../vitest.process.setup.js';
import { PLUGIN_OUT, ROOT } from './helpers/pluginBuild.js';

// Vorbedingungen der Prozess-Suite: fehlende Build-Artefakte und eine nicht erreichbare Test-DB
// sollen den Lauf mit einer einzigen Meldung abbrechen statt verstreut rote Tests zu erzeugen.
describe('ProcessPreflight', () => {
  it('meldet die fehlenden Build-Artefakte', () => {
    const vorhanden = new Set([join('/repo', BUILD_ARTIFACTS[0])]);
    const fehlend = ProcessPreflight.missingBuildArtifacts('/repo', (pfad) => vorhanden.has(pfad));
    expect(fehlend).toEqual(BUILD_ARTIFACTS.slice(1));
  });

  it('meldet nichts, wenn alle Build-Artefakte da sind', () => {
    expect(ProcessPreflight.missingBuildArtifacts('/repo', () => true)).toEqual([]);
  });

  it('nennt alle drei Artefakte, die auch der Plugin-Build voraussetzt', () => {
    expect(BUILD_ARTIFACTS).toEqual(['packages/cli/dist/main.js', 'packages/server/dist/main.js', 'packages/web/dist/index.html']);
  });

  it('prüft die Test-DB nur, wenn HARNESS_TEST_DB gesetzt ist', async () => {
    expect(await ProcessPreflight.unreachableTestDb(undefined, async () => { throw new Error('darf nicht aufgerufen werden'); })).toBe(null);
  });

  it('meldet eine nicht erreichbare Test-DB mit Grund', async () => {
    const meldung = await ProcessPreflight.unreachableTestDb(
      JSON.stringify({ server: 'localhost', port: 14333, database: 'Harness_Test', user: 'sa', password: 'x' }),
      async () => { throw new Error('ECONNREFUSED 127.0.0.1:14333'); },
    );
    expect(meldung).toContain('Harness_Test');
    expect(meldung).toContain('ECONNREFUSED');
  });

  it('meldet nichts, wenn die Test-DB antwortet', async () => {
    const raw = JSON.stringify({ server: 'localhost', database: 'Harness_Test', user: 'sa', password: 'x' });
    expect(await ProcessPreflight.unreachableTestDb(raw, async () => undefined)).toBe(null);
  });

  it('meldet eine unbrauchbare HARNESS_TEST_DB statt abzustürzen', async () => {
    const meldung = await ProcessPreflight.unreachableTestDb('{kein json', async () => undefined);
    expect(meldung).toContain('HARNESS_TEST_DB');
  });

  it('baut das Plugin in einen lauf-eigenen Ordner, nicht in den geteilten Repo-Ordner', () => {
    expect(process.env.HARNESS_PLUGIN_OUT, 'globalSetup hat die Umgebung nicht an die Testdateien gereicht').toBeTruthy();
    expect(PLUGIN_OUT.startsWith(join(ROOT, 'packages/plugin/dist'))).toBe(false);
  });
});
