import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { BaseSnapshot } from '../../../src/customization/BaseSnapshot.js';
import { CustomizationApplier, type CustomizationInput } from '../../../src/customization/CustomizationApplier.js';
import type { WorkflowTools } from '../../../src/customization/WorkflowTools.js';
import { WORKFLOW_PHASES } from '../../../src/workflow/WorkflowPhases.js';

const tools = (value: unknown): WorkflowTools => ({ typecheck: () => [], load: async () => value });
const NOW = new Date('2026-10-14T10:00:00Z');

describe('CustomizationApplier', () => {
  let root: string;
  let plugin: string;
  let input: CustomizationInput;
  const applier = new CustomizationApplier();

  const withWorkflow = (value: unknown) => {
    mkdirSync(join(root, 'custom'), { recursive: true });
    writeFileSync(join(root, 'custom', 'workflow.ts'), '// ersetzt');
    input = { ...input, tools: tools(value) };
  };

  beforeEach(() => {
    root = mkdtempSync(join(tmpdir(), 'harness-apply-'));
    plugin = join(root, 'plugin');
    // Die vier Skills, auf die der Standard-Workflow verweist
    for (const name of ['harness-grill', 'harness-wayfinder', 'harness-tasks', 'harness-implement']) {
      mkdirSync(join(plugin, 'skills', name), { recursive: true });
      writeFileSync(join(plugin, 'skills', name, 'SKILL.md'), `---\nname: ${name}\ndescription: d\n---\n`);
    }
    input = { cloneDir: root, customDir: join(root, 'custom'), standardDir: plugin, harnessVersion: '0.2.0', tools: tools(null), now: NOW };
  });

  afterEach(() => rmSync(root, { recursive: true, force: true }));

  it('meldet ohne custom/ den Standard und schreibt nur die Übersicht', async () => {
    const overview = await applier.apply(input, plugin);

    expect(overview).toEqual({ version: 1, createdAt: NOW.toISOString(), cloneDir: root, customDir: join(root, 'custom'), harnessVersion: '0.2.0', workflow: 'standard', entries: [] });
    expect(JSON.parse(readFileSync(join(plugin, 'customizations', 'overview.json'), 'utf8'))).toEqual(overview);
    expect(existsSync(join(plugin, 'customizations', 'workflow.json'))).toBe(false);
  });

  it('schreibt einen gültigen eigenen Workflow', async () => {
    const phases = WORKFLOW_PHASES.map((p) => (p.id === 'umsetzen' ? { ...p, steps: [...p.steps, { step: 'Grill', command: '/ai-harness:harness-grill', when: 'x' }] } : p));
    withWorkflow(phases);

    const overview = await applier.apply(input, plugin);

    expect(overview.workflow).toBe('custom');
    expect(overview.entries[0].warnings.map((w) => w.code)).toEqual(['base-unknown']);
    expect(readFileSync(join(plugin, 'customizations', 'workflow.json'), 'utf8')).toBe(BaseSnapshot.formatJson(phases));
  });

  it('fällt bei ungültigem Workflow zurück und ersetzt alte Ergebnisse', async () => {
    mkdirSync(join(plugin, 'customizations'), { recursive: true });
    writeFileSync(join(plugin, 'customizations', 'workflow.json'), '[]');
    withWorkflow({});

    const overview = await applier.apply(input, plugin);

    expect(overview.workflow).toBe('fallback');
    expect(existsSync(join(plugin, 'customizations', 'workflow.json'))).toBe(false);
  });

  it('prüft ohne zu schreiben', async () => {
    withWorkflow(WORKFLOW_PHASES);

    const { overview, phases } = await applier.check(input);

    expect(overview.workflow).toBe('custom');
    expect(phases).toEqual(WORKFLOW_PHASES);
    expect(existsSync(join(plugin, 'customizations'))).toBe(false);
  });

  it('zählt aktive eigene Skills bei den Verweisen des Workflows', async () => {
    const phases = WORKFLOW_PHASES.map((p) => (p.id === 'umsetzen' ? { ...p, steps: [...p.steps, { step: 'Review', command: '/ai-harness:mein-review', when: 'x' }] } : p));
    withWorkflow(phases);
    mkdirSync(join(root, 'custom', 'skills', 'mein-review'), { recursive: true });
    writeFileSync(join(root, 'custom', 'skills', 'mein-review', 'SKILL.md'), '---\ndescription: Neu\n---\n');
    const ok = (await applier.check(input)).overview;
    expect(ok.entries.map((e) => `${e.point}:${e.name}`)).toEqual(['workflow:workflow', 'skill:mein-review']);
    expect(ok.entries[0].warnings.map((w) => w.code)).toEqual(['base-unknown']);
    writeFileSync(join(root, 'custom', 'skills', 'mein-review', 'SKILL.md'), 'ohne Kopf');
    const missing = (await applier.check(input)).overview;
    expect(missing.entries[0].warnings.map((w) => w.code)).toContain('skill-ref-missing');
  });

  it('legt Skills und Output-Styles über das Plugin', async () => {
    mkdirSync(join(root, 'custom', 'skills', 'harness-grill'), { recursive: true });
    writeFileSync(join(root, 'custom', 'skills', 'harness-grill', 'SKILL.md'), '---\ndescription: Eigene Fassung\n---\n');
    mkdirSync(join(root, 'custom', 'output-styles'), { recursive: true });
    writeFileSync(join(root, 'custom', 'output-styles', 'kurz.md'), '---\ndescription: Kurz\n---\n');
    const overview = await applier.apply(input, plugin);
    expect(overview.entries.map((e) => `${e.point}:${e.name}:${e.kind}`)).toEqual(['skill:harness-grill:replace', 'output-style:kurz:new']);
    expect(readFileSync(join(plugin, 'skills', 'harness-grill', 'SKILL.md'), 'utf8')).toContain('Eigene Fassung');
    expect(existsSync(join(plugin, 'output-styles', 'kurz.md'))).toBe(true);
  });
});
