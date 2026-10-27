import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { BaseSnapshot } from '../../../src/customization/BaseSnapshot.js';
import { CustomizationPaths } from '../../../src/customization/CustomizationPaths.js';
import { WorkflowCustomization } from '../../../src/customization/WorkflowCustomization.js';
import type { WorkflowTools } from '../../../src/customization/WorkflowTools.js';
import type { Problem } from '../../../src/customization/CustomizationOverview.js';
import { WORKFLOW_PHASES } from '../../../src/workflow/WorkflowPhases.js';

const SKILLS = ['harness-grill', 'harness-wayfinder', 'harness-tasks', 'harness-implement'];

// Werkzeuge ohne tsc und esbuild: liefern vorgegebene Typprobleme und Werte
function tools(value: unknown, typeProblems: Problem[] = [], loadError?: string): WorkflowTools {
  return {
    typecheck: () => typeProblems,
    load: async () => { if (loadError) throw new Error(loadError); return value; },
  };
}

describe('WorkflowCustomization', () => {
  let root: string;
  let paths: CustomizationPaths;
  const custom = () => new WorkflowCustomization(paths, tools(WORKFLOW_PHASES));

  beforeEach(() => {
    root = mkdtempSync(join(tmpdir(), 'harness-wf-'));
    paths = new CustomizationPaths(join(root, 'custom'), join(root, 'std'));
    mkdirSync(join(root, 'custom'), { recursive: true });
    writeFileSync(paths.workflowFile, '// Inhalt egal, die Werkzeuge sind ersetzt');
  });

  afterEach(() => rmSync(root, { recursive: true, force: true }));

  it('gibt ohne Datei keinen Eintrag und den Standard', async () => {
    rmSync(paths.workflowFile);
    expect(await custom().check(SKILLS)).toEqual({ entry: null, phases: null });
  });

  it('fällt bei Typfehlern zurück und nennt Datei und Zeile relativ zum Clone', async () => {
    const typeProblem = { code: 'type-error', message: 'TS2322: x', file: paths.workflowFile, line: 3 };
    const result = await new WorkflowCustomization(paths, tools(null, [typeProblem])).check(SKILLS);
    expect(result.phases).toBeNull();
    expect(result.entry).toMatchObject({ point: 'workflow', name: 'workflow', kind: 'replace', state: 'fallback', base: 'none' });
    expect(result.entry!.problems).toEqual([{ ...typeProblem, file: 'custom/workflow.ts' }]);
  });

  it('fällt zurück, wenn das Laden scheitert', async () => {
    const result = await new WorkflowCustomization(paths, tools(null, [], 'kaputt')).check(SKILLS);
    expect(result.entry!.problems[0].code).toBe('load-failed');
    expect(result.entry!.problems[0].message).toContain('kaputt');
  });

  it('fällt zurück, wenn die Prüfung selbst unerwartet scheitert', async () => {
    const throwing: WorkflowTools = { typecheck: () => { throw new Error('tsc nicht da'); }, load: async () => WORKFLOW_PHASES };
    const result = await new WorkflowCustomization(paths, throwing).check(SKILLS);
    expect(result.phases).toBeNull();
    expect(result.entry).toMatchObject({ state: 'fallback', base: 'none' });
    expect(result.entry!.problems).toEqual([{ code: 'check-failed', message: 'Prüfung fehlgeschlagen: tsc nicht da', file: 'custom/workflow.ts' }]);
  });

  it('fällt zurück, wenn der Basisvergleich unerwartet scheitert', async () => {
    const snapshot = Object.assign(new BaseSnapshot(), { check: () => { throw new Error('Basis nicht lesbar'); } });
    const result = await new WorkflowCustomization(paths, tools(WORKFLOW_PHASES), undefined, snapshot).check(SKILLS);
    expect(result.phases).toBeNull();
    expect(result.entry!.state).toBe('fallback');
    expect(result.entry!.problems).toEqual([{ code: 'check-failed', message: 'Prüfung fehlgeschlagen: Basis nicht lesbar', file: 'custom/workflow.ts' }]);
  });

  it('fällt ohne Pflicht-Phase zurück', async () => {
    const result = await new WorkflowCustomization(paths, tools(WORKFLOW_PHASES.slice(0, 1))).check(SKILLS);
    expect(result.entry!.state).toBe('fallback');
    expect(result.entry!.problems).toEqual([{ code: 'required-phase-missing', message: 'Die Pflicht-Phase umsetzen fehlt.', file: 'custom/workflow.ts' }]);
  });

  it('ist gültig aktiv; ohne Basis mit Hinweis', async () => {
    const result = await custom().check(SKILLS);
    expect(result.phases).toEqual(WORKFLOW_PHASES);
    expect(result.entry).toMatchObject({ state: 'active', base: 'unknown' });
    expect(result.entry!.warnings.map((w) => w.code)).toEqual(['base-unknown']);
  });

  it('erkennt aktuelle und veraltete Basis', async () => {
    const base = paths.baseOf('workflow', 'workflow');
    new BaseSnapshot().saveText(BaseSnapshot.formatJson(WORKFLOW_PHASES), base);
    expect((await custom().check(SKILLS)).entry).toMatchObject({ base: 'current', warnings: [] });

    new BaseSnapshot().saveText('[]\n', base);
    const outdated = (await custom().check(SKILLS)).entry!;
    expect(outdated.base).toBe('outdated');
    expect(outdated.warnings.map((w) => w.code)).toEqual(['base-outdated']);
    expect(outdated.diff).toBeTruthy();
  });
});
