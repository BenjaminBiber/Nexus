import { describe, expect, it } from 'vitest';
import type { CustomizationEntry, CustomizationOverview } from '../../../src/customization/CustomizationOverview.js';
import { OverviewFormatter } from '../../../src/customization/OverviewFormatter.js';

const base: CustomizationOverview = { version: 1, createdAt: '2026-10-14T10:00:00.000Z', cloneDir: '/h', customDir: '/h/custom', harnessVersion: '0.2.0', workflow: 'standard', entries: [] };
const entry = (extra: Partial<CustomizationEntry>): CustomizationEntry => ({ point: 'skill', name: 'x', kind: 'new', state: 'active', base: 'none', problems: [], warnings: [], ...extra });
const formatter = new OverviewFormatter();

describe('OverviewFormatter', () => {
  it('meldet, wenn es keine Anpassungen gibt', () => {
    expect(formatter.format(base)).toBe('Keine Anpassungen, überall gilt der Standard.\nÄndern: /ai-harness:harness-customize');
  });

  it('zeigt je Erweiterungspunkt eine Zeile, Warnungen mit gekürztem Diff und Rückfälle mit Ort', () => {
    const diff = Array.from({ length: 50 }, (_, i) => `+zeile ${i}`).join('\n');
    const text = formatter.format({
      ...base,
      workflow: 'custom',
      entries: [
        entry({ point: 'workflow', name: 'workflow', kind: 'replace', base: 'current' }),
        entry({ name: 'harness-implement', kind: 'replace', base: 'outdated', warnings: [{ code: 'base-outdated', message: 'Der Standard hat sich seit deiner Kopie geändert.' }], diff }),
        entry({ name: 'mein-review' }),
        entry({ point: 'output-style', name: 'kurz', state: 'fallback', problems: [{ code: 'description-missing', message: 'Im Kopf fehlt description.', file: 'custom/output-styles/kurz.md' }] }),
      ],
    }, { maxDiffLines: 40 });
    expect(text).toContain('Anpassungen (custom/ in /h/custom)');
    expect(text).toContain('  Workflow        angepasst');
    expect(text).toContain('  Skills          harness-implement (ersetzt), mein-review (neu)');
    expect(text).toContain('  Output-Styles   Standard');
    expect(text).toContain('  ! Skill harness-implement: Der Standard hat sich seit deiner Kopie geändert.');
    expect(text).toContain('      +zeile 39');
    expect(text).not.toContain('+zeile 40');
    expect(text).toContain('      … 10 weitere Zeilen: npm run customize -- check');
    expect(text).toContain('  ✗ Output-Style kurz: Im Kopf fehlt description. (custom/output-styles/kurz.md)');
    expect(text.endsWith('Ändern: /ai-harness:harness-customize')).toBe(true);
  });

  it('nennt die Zeile bei Typfehlern und benennt Erweiterungspunkte', () => {
    const text = formatter.format({ ...base, workflow: 'fallback', entries: [entry({ point: 'workflow', name: 'workflow', kind: 'replace', state: 'fallback', problems: [{ code: 'type-error', message: 'TS2322: x', file: 'custom/workflow.ts', line: 3 }] })] });
    expect(text).toContain('  Workflow        Rückfall auf Standard');
    expect(text).toContain('  ✗ Workflow: TS2322: x (custom/workflow.ts:3)');
    expect(OverviewFormatter.label('skill', 'x')).toBe('Skill x');
    expect(OverviewFormatter.label('output-style', 'y')).toBe('Output-Style y');
    expect(OverviewFormatter.label('workflow', 'workflow')).toBe('Workflow');
  });
});
