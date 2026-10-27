import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { BaseSnapshot } from '../../../src/customization/BaseSnapshot.js';
import { CustomizationPaths } from '../../../src/customization/CustomizationPaths.js';
import { SkillCustomization } from '../../../src/customization/SkillCustomization.js';

describe('SkillCustomization', () => {
  let root: string;
  let paths: CustomizationPaths;
  const skill = (base: string, name: string, head: string | null) => {
    mkdirSync(join(base, name), { recursive: true });
    if (head !== null) writeFileSync(join(base, name, 'SKILL.md'), `---\n${head}\n---\nText von ${name}`);
  };

  beforeEach(() => {
    root = mkdtempSync(join(tmpdir(), 'harness-skills-'));
    paths = new CustomizationPaths(join(root, 'custom'), join(root, 'std'));
    skill(join(root, 'std', 'skills'), 'harness-grill', 'name: harness-grill\ndescription: Standard');
    writeFileSync(join(root, 'std', 'skills', 'harness-grill', 'alt.md'), 'alt');
    const own = join(root, 'custom', 'skills');
    skill(own, 'harness-grill', 'name: harness-grill\ndescription: Eigen');
    skill(own, 'mein-review', 'description: Neu');
    skill(own, 'kaputt', null);
    skill(own, 'Gross', 'description: x');
    skill(own, 'ohne-text', 'name: ohne-text');
    skill(own, 'falsch', 'name: anders\ndescription: x');
  });

  afterEach(() => rmSync(root, { recursive: true, force: true }));

  it('prüft jeden eigenen Skill', () => {
    const entries = new SkillCustomization(paths).check();
    const summary = entries.map((e) => [e.name, e.kind, e.state, e.base, e.problems.map((p) => p.code).join()]);
    expect(summary).toEqual([
      ['Gross', 'new', 'fallback', 'none', 'name-invalid'],
      ['falsch', 'new', 'fallback', 'none', 'name-mismatch'],
      ['harness-grill', 'replace', 'active', 'unknown', ''],
      ['kaputt', 'new', 'fallback', 'none', 'skill-file-missing'],
      ['mein-review', 'new', 'active', 'none', ''],
      ['ohne-text', 'new', 'fallback', 'none', 'description-missing'],
    ]);
    expect(entries.find((e) => e.name === 'kaputt')!.problems[0].file).toBe('custom/skills/kaputt');
    expect(entries.find((e) => e.name === 'mein-review')!.warnings).toEqual([]);
  });

  it('meldet ein kaputtes YAML im Kopf als solches statt als fehlende description', () => {
    skill(join(root, 'custom', 'skills'), 'yaml-kaputt', 'description: Review: prüft vor dem Abschluss');
    const entry = new SkillCustomization(paths).check().find((e) => e.name === 'yaml-kaputt')!;
    expect(entry.state).toBe('fallback');
    expect(entry.problems).toHaveLength(1);
    expect(entry.problems[0]).toMatchObject({ code: 'front-matter-invalid', file: 'custom/skills/yaml-kaputt/SKILL.md' });
    expect(entry.problems[0].message).toMatch(/^Der Kopf ist kein gültiges YAML: .+/);
    expect(entry.problems[0].message).not.toContain('\n');
  });

  it('lässt einen unerwarteten Fehler in einem Skill nicht die anderen verwerfen', () => {
    // SKILL.md als Ordner: existsSync ist wahr, das Lesen wirft
    mkdirSync(join(root, 'custom', 'skills', 'ordner-skill', 'SKILL.md'), { recursive: true });
    const entries = new SkillCustomization(paths).check();
    const broken = entries.find((e) => e.name === 'ordner-skill')!;
    expect(broken).toMatchObject({ point: 'skill', kind: 'new', state: 'fallback', base: 'none', warnings: [] });
    expect(broken.problems).toHaveLength(1);
    expect(broken.problems[0].code).toBe('check-failed');
    expect(broken.problems[0].message).toMatch(/^Prüfung fehlgeschlagen: .+/);
    expect(broken.problems[0].file).toBe('custom/skills/ordner-skill');
    // Die übrigen Skills werden weiter bewertet
    expect(entries.find((e) => e.name === 'mein-review')).toMatchObject({ state: 'active' });
    expect(entries.find((e) => e.name === 'harness-grill')).toMatchObject({ kind: 'replace', state: 'active' });
    expect(entries.find((e) => e.name === 'falsch')!.problems.map((p) => p.code)).toEqual(['name-mismatch']);
  });

  it('legt gültige Skills über das Plugin und übergeht ungültige', () => {
    const plugin = join(root, 'plugin');
    skill(join(plugin, 'skills'), 'harness-grill', 'description: Standard');
    writeFileSync(join(plugin, 'skills', 'harness-grill', 'alt.md'), 'alt');
    const custom = new SkillCustomization(paths);
    custom.apply(custom.check(), plugin);
    expect(readFileSync(join(plugin, 'skills', 'harness-grill', 'SKILL.md'), 'utf8')).toContain('Eigen');
    expect(existsSync(join(plugin, 'skills', 'harness-grill', 'alt.md'))).toBe(false);
    expect(existsSync(join(plugin, 'skills', 'mein-review', 'SKILL.md'))).toBe(true);
    expect(existsSync(join(plugin, 'skills', 'kaputt'))).toBe(false);
  });

  it('liefert ohne Ordner nichts', () => {
    rmSync(join(root, 'custom'), { recursive: true });
    expect(new SkillCustomization(paths).check()).toEqual([]);
  });

  it('vergleicht die Basis ersetzter Skills', () => {
    const find = () => new SkillCustomization(paths).check().find((e) => e.name === 'harness-grill')!;
    const snapshot = new BaseSnapshot();
    // Ohne Basis: unbekannt mit Hinweis
    const fresh = find();
    expect(fresh.base).toBe('unknown');
    expect(fresh.warnings.map((w) => w.code)).toEqual(['base-unknown']);
    expect(fresh.warnings[0].file).toBe('custom/skills/harness-grill');
    // Basis als Kopie des Standards: aktuell, ohne Warnung und ohne Diff
    snapshot.save(join(root, 'std', 'skills', 'harness-grill'), paths.baseOf('skill', 'harness-grill'));
    const current = find();
    expect(current.base).toBe('current');
    expect(current.warnings).toEqual([]);
    expect(current.diff).toBeUndefined();
    // Standard geändert: veraltet mit Warnung, Ort und Diff
    writeFileSync(join(root, 'std', 'skills', 'harness-grill', 'SKILL.md'), '---\nname: harness-grill\ndescription: Standard neu\n---\n');
    const outdated = find();
    expect(outdated.base).toBe('outdated');
    expect(outdated.warnings.map((w) => w.code)).toEqual(['base-outdated']);
    expect(outdated.warnings[0].file).toBe('custom/skills/harness-grill');
    expect(outdated.diff).toContain('+description: Standard neu');
  });
});
