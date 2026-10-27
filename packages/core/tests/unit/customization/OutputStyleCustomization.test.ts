import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { BaseSnapshot } from '../../../src/customization/BaseSnapshot.js';
import { CustomizationPaths } from '../../../src/customization/CustomizationPaths.js';
import { OutputStyleCustomization } from '../../../src/customization/OutputStyleCustomization.js';

describe('OutputStyleCustomization', () => {
  let root: string;
  let paths: CustomizationPaths;

  const style = (dir: string, file: string, text: string) => {
    mkdirSync(dir, { recursive: true });
    writeFileSync(join(dir, file), text);
  };

  beforeEach(() => {
    root = mkdtempSync(join(tmpdir(), 'harness-styles-'));
    paths = new CustomizationPaths(join(root, 'custom'), join(root, 'std'));
    style(join(root, 'std', 'output-styles'), 'software-engineer.md', '---\nname: software-engineer\ndescription: Standard\n---\n');
    const own = join(root, 'custom', 'output-styles');
    style(own, 'software-engineer.md', '---\nname: software-engineer\ndescription: Eigen\n---\nEigener Stil');
    style(own, 'kurz.md', '---\ndescription: Kurz\n---\n');
    style(own, 'leer.md', '---\nname: leer\n---\n');
    style(own, 'anders.md', '---\nname: x\ndescription: d\n---\n');
    style(own, 'notiz.txt', 'egal');
  });

  afterEach(() => rmSync(root, { recursive: true, force: true }));

  it('prüft jeden eigenen Output-Style', () => {
    const summary = new OutputStyleCustomization(paths).check().map((e) => [e.name, e.kind, e.state, e.base, e.problems.map((p) => p.code).join()]);
    expect(summary).toEqual([
      ['anders', 'new', 'fallback', 'none', 'name-mismatch'],
      ['kurz', 'new', 'active', 'none', ''],
      ['leer', 'new', 'fallback', 'none', 'description-missing'],
      ['software-engineer', 'replace', 'active', 'unknown', ''],
    ]);
  });

  it('meldet ein kaputtes YAML im Kopf als solches statt als fehlende description', () => {
    style(join(root, 'custom', 'output-styles'), 'yaml-kaputt.md', '---\ndescription: Review: prüft vor dem Abschluss\n---\n');
    const entry = new OutputStyleCustomization(paths).check().find((e) => e.name === 'yaml-kaputt')!;
    expect(entry.state).toBe('fallback');
    expect(entry.problems).toHaveLength(1);
    expect(entry.problems[0]).toMatchObject({ code: 'front-matter-invalid', file: 'custom/output-styles/yaml-kaputt.md' });
    expect(entry.problems[0].message).toMatch(/^Der Kopf ist kein gültiges YAML: .+/);
    expect(entry.problems[0].message).not.toContain('\n');
  });

  it('lässt einen unerwarteten Fehler in einem Output-Style nicht die anderen verwerfen', () => {
    // x.md als Ordner: wird als Output-Style gelistet, das Lesen wirft
    mkdirSync(join(root, 'custom', 'output-styles', 'ordner-stil.md'), { recursive: true });
    const entries = new OutputStyleCustomization(paths).check();
    const broken = entries.find((e) => e.name === 'ordner-stil')!;
    expect(broken).toMatchObject({ point: 'output-style', kind: 'new', state: 'fallback', base: 'none', warnings: [] });
    expect(broken.problems).toHaveLength(1);
    expect(broken.problems[0].code).toBe('check-failed');
    expect(broken.problems[0].message).toMatch(/^Prüfung fehlgeschlagen: .+/);
    expect(broken.problems[0].file).toBe('custom/output-styles/ordner-stil.md');
    // Die übrigen Output-Styles werden weiter bewertet
    expect(entries.find((e) => e.name === 'kurz')).toMatchObject({ state: 'active' });
    expect(entries.find((e) => e.name === 'software-engineer')).toMatchObject({ kind: 'replace', state: 'active' });
    expect(entries.find((e) => e.name === 'anders')!.problems.map((p) => p.code)).toEqual(['name-mismatch']);
  });

  it('legt gültige Output-Styles über das Plugin', () => {
    const plugin = join(root, 'plugin');
    style(join(plugin, 'output-styles'), 'software-engineer.md', 'Standard');
    const custom = new OutputStyleCustomization(paths);
    custom.apply(custom.check(), plugin);
    expect(readFileSync(join(plugin, 'output-styles', 'software-engineer.md'), 'utf8')).toContain('Eigener Stil');
    expect(existsSync(join(plugin, 'output-styles', 'kurz.md'))).toBe(true);
    expect(existsSync(join(plugin, 'output-styles', 'leer.md'))).toBe(false);
  });

  it('vergleicht die Basis ersetzter Output-Styles', () => {
    const find = () => new OutputStyleCustomization(paths).check().find((e) => e.name === 'software-engineer')!;
    const snapshot = new BaseSnapshot();
    const standard = join(root, 'std', 'output-styles', 'software-engineer.md');
    // Ohne Basis: unbekannt mit Hinweis
    const fresh = find();
    expect(fresh.base).toBe('unknown');
    expect(fresh.warnings.map((w) => w.code)).toEqual(['base-unknown']);
    expect(fresh.warnings[0].file).toBe('custom/output-styles/software-engineer.md');
    // Basis als Kopie des Standards: aktuell, ohne Warnung und ohne Diff
    snapshot.save(standard, paths.baseOf('output-style', 'software-engineer'));
    const current = find();
    expect(current.base).toBe('current');
    expect(current.warnings).toEqual([]);
    expect(current.diff).toBeUndefined();
    // Standard geändert: veraltet mit Warnung, Ort und Diff
    writeFileSync(standard, '---\nname: software-engineer\ndescription: Standard neu\n---\n');
    const outdated = find();
    expect(outdated.base).toBe('outdated');
    expect(outdated.warnings.map((w) => w.code)).toEqual(['base-outdated']);
    expect(outdated.warnings[0].file).toBe('custom/output-styles/software-engineer.md');
    expect(outdated.diff).toContain('+description: Standard neu');
  });
});
