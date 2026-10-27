import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { HarnessHome } from '../../../src/registry/HarnessHome.js';
import { WorkflowSource } from '../../../src/workflow/WorkflowSource.js';
import { WORKFLOW_PHASES } from '../../../src/workflow/WorkflowPhases.js';

describe('WorkflowSource', () => {
  let dir: string;
  let home: HarnessHome;
  const write = (text: string) => {
    mkdirSync(home.customizationsDir, { recursive: true });
    writeFileSync(join(home.customizationsDir, 'workflow.json'), text);
  };

  beforeEach(() => {
    dir = mkdtempSync(join(tmpdir(), 'harness-wfsrc-'));
    home = new HarnessHome({ HARNESS_HOME: dir });
  });
  afterEach(() => rmSync(dir, { recursive: true, force: true }));

  it('liefert ohne Datei den Standard', () => {
    expect(new WorkflowSource(home).read()).toEqual({ source: 'standard', phases: WORKFLOW_PHASES });
  });

  it('liefert den angewendeten Workflow', () => {
    const phases = [
      { id: 'planen', title: 'P', hint: 'h', steps: [] },
      { id: 'umsetzen', title: 'U', hint: 'h', steps: [] },
    ];
    write(JSON.stringify(phases));
    expect(new WorkflowSource(home).read()).toEqual({ source: 'custom', phases });
  });

  it('fällt bei kaputter oder leerer Datei auf den Standard zurück', () => {
    write('kaputt');
    expect(new WorkflowSource(home).read().source).toBe('standard');
    write('[]');
    expect(new WorkflowSource(home).read().source).toBe('standard');
  });
});
