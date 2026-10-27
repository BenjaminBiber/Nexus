import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { BaseSnapshot } from '../../../src/customization/BaseSnapshot.js';
import { CustomizationCopier } from '../../../src/customization/CustomizationCopier.js';
import { CustomizationPaths } from '../../../src/customization/CustomizationPaths.js';
import { WORKFLOW_PHASES } from '../../../src/workflow/WorkflowPhases.js';

describe('CustomizationCopier', () => {
  let root: string;
  let paths: CustomizationPaths;
  let copier: CustomizationCopier;
  const grill = { point: 'skill' as const, name: 'harness-grill' };
  const workflow = { point: 'workflow' as const, name: 'workflow' };
  const code = (fn: () => unknown) => { try { fn(); return null; } catch (e) { return (e as { code?: string }).code; } };

  beforeEach(() => {
    root = mkdtempSync(join(tmpdir(), 'harness-copy-'));
    paths = new CustomizationPaths(join(root, 'custom'), join(root, 'std'));
    mkdirSync(join(root, 'std', 'skills', 'harness-grill'), { recursive: true });
    writeFileSync(join(root, 'std', 'skills', 'harness-grill', 'SKILL.md'), 'Standard');
    copier = new CustomizationCopier(paths);
  });
  afterEach(() => rmSync(root, { recursive: true, force: true }));

  it('kopiert den Workflow als TypeScript mit Basis', () => {
    expect(copier.copy(workflow)).toContain('Wirkt nach npm run setup');
    const source = readFileSync(paths.workflowFile, 'utf8');
    expect(source).toContain("import type { WorkflowPhase } from '@harness/core';");
    expect(source).toContain('satisfies WorkflowPhase[]');
    expect(readFileSync(paths.baseOf('workflow', 'workflow'), 'utf8')).toBe(BaseSnapshot.formatJson(WORKFLOW_PHASES));
    expect(code(() => copier.copy(workflow))).toBe('customization-exists');
  });

  it('kopiert einen Skill mit Basis und verweigert Unbekanntes', () => {
    copier.copy(grill);
    expect(readFileSync(join(paths.customOf('skill', 'harness-grill'), 'SKILL.md'), 'utf8')).toBe('Standard');
    expect(existsSync(join(paths.baseOf('skill', 'harness-grill'), 'SKILL.md'))).toBe(true);
    expect(code(() => copier.copy({ point: 'skill', name: 'gibt-es-nicht' }))).toBe('customization-no-standard');
  });

  it('erneuert nur die Basis', () => {
    copier.copy(grill);
    writeFileSync(join(paths.customOf('skill', 'harness-grill'), 'SKILL.md'), 'Eigen');
    writeFileSync(join(root, 'std', 'skills', 'harness-grill', 'SKILL.md'), 'Standard neu');
    const snapshot = new BaseSnapshot();
    expect(snapshot.state(paths.baseOf('skill', 'harness-grill'), paths.standardOf('skill', 'harness-grill')!)).toBe('outdated');
    copier.copy(grill, { rebase: true });
    expect(snapshot.state(paths.baseOf('skill', 'harness-grill'), paths.standardOf('skill', 'harness-grill')!)).toBe('current');
    expect(readFileSync(join(paths.customOf('skill', 'harness-grill'), 'SKILL.md'), 'utf8')).toBe('Eigen');
    expect(code(() => copier.copy({ point: 'output-style', name: 'x' }, { rebase: true }))).toBe('customization-missing');
  });

  it('setzt auf den Standard zurück', () => {
    copier.copy(grill);
    expect(copier.reset(grill)).toContain('npm run setup');
    expect(existsSync(paths.customOf('skill', 'harness-grill'))).toBe(false);
    expect(existsSync(paths.baseOf('skill', 'harness-grill'))).toBe(false);
    expect(code(() => copier.reset(grill))).toBe('customization-missing');
  });

  it('lehnt unzulässige Namen ab, bevor etwas gelöscht wird', () => {
    copier.copy(grill);
    writeFileSync(join(root, 'marker.txt'), 'x');
    expect(code(() => copier.reset({ point: 'skill', name: '../..' }))).toBe('customization-invalid-name');
    expect(code(() => copier.reset({ point: 'skill', name: '..' }))).toBe('customization-invalid-name');
    expect(code(() => copier.copy({ point: 'output-style', name: '../x' }))).toBe('customization-invalid-name');
    expect(existsSync(join(root, 'marker.txt'))).toBe(true);
    expect(existsSync(paths.customOf('skill', 'harness-grill'))).toBe(true);
  });

  it('legt bei unbekanntem Standard keinen Ordner an', () => {
    expect(code(() => copier.copy({ point: 'skill', name: 'gibt-es-nicht' }))).toBe('customization-no-standard');
    expect(existsSync(paths.skillsDir)).toBe(false);
  });
});
