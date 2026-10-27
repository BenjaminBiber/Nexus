import { join } from 'node:path';
import { describe, expect, it } from 'vitest';
import { CustomizationPaths } from '../../../src/customization/CustomizationPaths.js';
describe('CustomizationPaths', () => {
  const root = join('/c');
  const paths = new CustomizationPaths(join(root, 'custom'), join(root, 'std'));
  it('nennt die Orte der Anpassungen in custom/', () => {
    expect(paths.customOf('workflow', 'workflow')).toBe(join(root, 'custom', 'workflow.ts'));
    expect(paths.customOf('skill', 'x')).toBe(join(root, 'custom', 'skills', 'x'));
    expect(paths.customOf('output-style', 'y')).toBe(join(root, 'custom', 'output-styles', 'y.md'));
  });
  it('nennt die Orte der Basis in custom/.base/', () => {
    expect(paths.baseOf('workflow', 'workflow')).toBe(join(root, 'custom', '.base', 'workflow.json'));
    expect(paths.baseOf('skill', 'x')).toBe(join(root, 'custom', '.base', 'skills', 'x'));
    expect(paths.baseOf('output-style', 'y')).toBe(join(root, 'custom', '.base', 'output-styles', 'y.md'));
  });
  it('nennt den Standard; den Workflow gibt es nur als Daten', () => {
    expect(paths.standardOf('skill', 'x')).toBe(join(root, 'std', 'skills', 'x'));
    expect(paths.standardOf('output-style', 'y')).toBe(join(root, 'std', 'output-styles', 'y.md'));
    expect(paths.standardOf('workflow', 'workflow')).toBeNull();
  });
  it('zeigt Pfade relativ zum Clone mit Schrägstrich', () => {
    expect(paths.display(join(root, 'custom', 'skills', 'x', 'SKILL.md'))).toBe('custom/skills/x/SKILL.md');
  });
});
