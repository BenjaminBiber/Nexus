import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { OverviewFile } from '../../../src/customization/OverviewFile.js';
import { HarnessHome } from '../../../src/registry/HarnessHome.js';

describe('OverviewFile', () => {
  let dir: string;
  let home: HarnessHome;

  beforeEach(() => {
    dir = mkdtempSync(join(tmpdir(), 'harness-ovf-'));
    home = new HarnessHome({ HARNESS_HOME: dir });
  });

  afterEach(() => rmSync(dir, { recursive: true, force: true }));

  it('liest die Übersicht oder null', () => {
    expect(new OverviewFile(home).read()).toBeNull();

    mkdirSync(home.customizationsDir, { recursive: true });
    writeFileSync(join(home.customizationsDir, 'overview.json'), 'kaputt');
    expect(new OverviewFile(home).read()).toBeNull();

    writeFileSync(join(home.customizationsDir, 'overview.json'), '{"version":1,"entries":[]}');
    expect(new OverviewFile(home).read()).toEqual({ version: 1, entries: [] });
  });

  it('liefert null bei gültigem JSON mit falscher Form', () => {
    mkdirSync(home.customizationsDir, { recursive: true });
    for (const content of ['{}', '[]', '{"version":1,"entries":null}', '{"version":1}']) {
      writeFileSync(join(home.customizationsDir, 'overview.json'), content);
      expect(new OverviewFile(home).read()).toBeNull();
    }
  });
});
