import { join } from 'node:path';
import { describe, expect, it } from 'vitest';
import { HarnessHome } from '../../../src/registry/HarnessHome.js';
import * as core from '../../../src/index.js';

describe('HarnessHome', () => {
  it('nennt den Ordner der angewendeten Anpassungen', () => {
    const home = new HarnessHome({ HARNESS_HOME: join('x', 'h') });
    expect(home.customizationsDir).toBe(join('x', 'h', 'customizations'));
  });

  it('nennt die Standarddatei der Board-Datenbank', () => {
    const home = new HarnessHome({ HARNESS_HOME: join('x', 'h') });
    expect(home.boardDbFile).toBe(join('x', 'h', 'board.db'));
  });

  it('exportiert den Vertrag des Workflows', () => {
    expect(core.WORKFLOW_PHASES.map((p) => p.id)).toEqual(['planen', 'umsetzen']);
  });
});
