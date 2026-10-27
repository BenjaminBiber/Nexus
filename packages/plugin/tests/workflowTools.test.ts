import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterAll, beforeAll, describe, expect, it } from 'vitest';
import { CloneWorkflowTools } from '../../core/src/customization/CloneWorkflowTools.js';
import { ROOT } from './helpers/pluginBuild.js';
const VALID = [
  "import { WORKFLOW_PHASES, type WorkflowPhase } from '@harness/core';",
  'export default WORKFLOW_PHASES.map((p) =>',
  "  p.id === 'umsetzen' ? { ...p, steps: [...p.steps, { step: 'Review', command: '/ai-harness:mein-review', when: 'Vorher.' }] } : p,",
  ') satisfies WorkflowPhase[];',
].join('\n');
const WRONG_TYPE = [
  "import type { WorkflowPhase } from '@harness/core';",
  'export default [',
  "  { id: 'planen', title: 'P', hint: 'h', steps: [] },",
  '  {',
  "    title: 1, id: 'umsetzen', hint: 'h', steps: [],",
  '  },',
  '] satisfies WorkflowPhase[];',
].join('\n');
describe('CloneWorkflowTools', () => {
  let dir: string;
  const tools = new CloneWorkflowTools(ROOT);
  const write = (name: string, text: string) => {
    const file = join(dir, 'custom', name);
    mkdirSync(join(dir, 'custom'), { recursive: true });
    writeFileSync(file, text);
    return file;
  };
  beforeAll(() => { dir = mkdtempSync(join(tmpdir(), 'harness-tools-')); });
  afterAll(() => rmSync(dir, { recursive: true, force: true }));
  it('prüft und lädt einen gültigen Workflow', async () => {
    const file = write('workflow.ts', VALID);
    expect(tools.typecheck(file)).toEqual([]);
    const value = (await tools.load(file)) as Array<{ id: string; steps: Array<{ step: string }> }>;
    expect(value.map((p) => p.id)).toEqual(['planen', 'umsetzen']);
    expect(value[1].steps.map((s) => s.step)).toContain('Review');
  });
  it('meldet Typfehler mit Zeile', () => {
    const problems = tools.typecheck(write('falsch.ts', WRONG_TYPE));
    expect(problems.length).toBeGreaterThan(0);
    expect(problems[0]).toMatchObject({ code: 'type-error', line: 5 });
    expect(problems[0].file).toMatch(/falsch\.ts$/);
  });
  it('verlangt einen Default-Export', () => {
    const problems = tools.typecheck(write('leer.ts', 'export const x = 1;\n'));
    expect(problems.map((p) => p.message).join(' ')).toContain('Default-Export');
  });
  it('lädt nur Daten', async () => {
    const value = await tools.load(write('fn.ts', "export default [{ id: 'planen', run: () => 1 }];\n"));
    expect(value).toEqual([{ id: 'planen' }]);
  });
});

describe('CloneWorkflowTools: Zeitgrenze', () => {
  it('meldet eine zu lange Typprüfung als Problem statt zu hängen', () => {
    // Clone-Attrappe, deren tsc nur wartet; die kurze Zeitgrenze löst ein Abbrechen aus
    const clone = mkdtempSync(join(tmpdir(), 'harness-slow-clone-'));
    try {
      mkdirSync(join(clone, 'node_modules', 'typescript', 'bin'), { recursive: true });
      writeFileSync(join(clone, 'node_modules', 'typescript', 'bin', 'tsc'), 'setTimeout(() => {}, 5000);\n');
      const file = join(clone, 'workflow.ts');
      writeFileSync(file, 'export default [];\n');
      const problems = new CloneWorkflowTools(clone, 300).typecheck(file);
      expect(problems).toEqual([{ code: 'type-error', message: 'Die Typprüfung hat zu lange gedauert (über 2 Minuten).', file }]);
    } finally {
      rmSync(clone, { recursive: true, force: true });
    }
  });
});
