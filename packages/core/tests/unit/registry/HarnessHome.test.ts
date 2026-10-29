import { mkdirSync, mkdtempSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';
import { HarnessHome } from '../../../src/registry/HarnessHome.js';
import * as core from '../../../src/index.js';

// Benutzerordner der Tests; `dirs` legt die genannten Unterordner an
function base(...dirs: string[]): string {
  const dir = mkdtempSync(join(tmpdir(), 'nexus-home-'));
  for (const name of dirs) mkdirSync(join(dir, name));
  return dir;
}

describe('HarnessHome', () => {
  it('nimmt NEXUS_HOME vor HARNESS_HOME', () => {
    const home = new HarnessHome({ NEXUS_HOME: join('x', 'n'), HARNESS_HOME: join('x', 'h') });
    expect(home.dir).toBe(join('x', 'n'));
    expect(home.source).toBe('env-nexus');
  });

  it('nimmt HARNESS_HOME als veralteten Rückfall', () => {
    const home = new HarnessHome({ HARNESS_HOME: join('x', 'h') });
    expect(home.dir).toBe(join('x', 'h'));
    expect(home.source).toBe('env-harness');
  });

  it('nimmt ohne Variablen ~/.nexus, wenn es den Ordner gibt', () => {
    const dir = base('.nexus', '.harness');
    const home = new HarnessHome({}, dir);
    expect(home.dir).toBe(join(dir, '.nexus'));
    expect(home.source).toBe('nexus');
  });

  it('fällt ohne ~/.nexus auf ~/.harness zurück', () => {
    const dir = base('.harness');
    const home = new HarnessHome({}, dir);
    expect(home.dir).toBe(join(dir, '.harness'));
    expect(home.source).toBe('harness-fallback');
  });

  it('nennt ohne beide Ordner ~/.nexus als Neuinstallation', () => {
    const dir = base();
    const home = new HarnessHome({}, dir);
    expect(home.dir).toBe(join(dir, '.nexus'));
    expect(home.source).toBe('new');
  });

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
