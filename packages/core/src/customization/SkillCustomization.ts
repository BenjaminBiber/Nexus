import { cpSync, existsSync, readdirSync, readFileSync, rmSync } from 'node:fs';
import { join } from 'node:path';
import { BaseSnapshot } from './BaseSnapshot.js';
import type { CustomizationEntry, Problem } from './CustomizationOverview.js';
import type { CustomizationPaths } from './CustomizationPaths.js';
import { FrontMatter } from './FrontMatter.js';
import { NameRules } from './NameRules.js';

// Eigene Skills in custom/skills/<name>/: prüfen und über die Skills des gebauten Plugins legen.
export class SkillCustomization {
  constructor(
    private readonly paths: CustomizationPaths,
    private readonly snapshot = new BaseSnapshot(),
    private readonly rules = new NameRules(),
    private readonly front = new FrontMatter(),
  ) {}

  // Ein Eintrag je Ordner in custom/skills, nach Name sortiert
  check(): CustomizationEntry[] {
    if (!existsSync(this.paths.skillsDir)) return [];
    return readdirSync(this.paths.skillsDir, { withFileTypes: true })
      .filter((e) => e.isDirectory())
      .map((e) => e.name)
      .sort()
      .map((name) => this.safeEntry(name));
  }

  // Aktive Skills ersetzen den gleichnamigen Standard-Ordner vollständig oder kommen neu dazu
  apply(entries: CustomizationEntry[], pluginDir: string): void {
    for (const entry of entries.filter((e) => e.point === 'skill' && e.state === 'active')) {
      const target = join(pluginDir, 'skills', entry.name);
      rmSync(target, { recursive: true, force: true });
      cpSync(this.paths.customOf('skill', entry.name), target, { recursive: true });
    }
  }

  // Wie entry, aber ein unerwarteter Fehler betrifft nur diesen Skill und führt zum Rückfall
  private safeEntry(name: string): CustomizationEntry {
    try {
      return this.entry(name);
    } catch (error) {
      const file = this.paths.display(this.paths.customOf('skill', name));
      const problem: Problem = { code: 'check-failed', message: `Prüfung fehlgeschlagen: ${(error as Error).message}`, file };
      return { point: 'skill', name, kind: this.kind(name), state: 'fallback', base: 'none', problems: [problem], warnings: [] };
    }
  }

  // Ein Standard-Skill gleichen Namens wird ersetzt, sonst ist der Skill neu
  private kind(name: string): 'replace' | 'new' {
    return existsSync(join(this.paths.standardOf('skill', name)!, 'SKILL.md')) ? 'replace' : 'new';
  }

  // Bewertet einen Skill: Rückfall bei Problemen, sonst aktiv; ersetzte Skills mit Basisvergleich
  private entry(name: string): CustomizationEntry {
    const standard = this.paths.standardOf('skill', name)!;
    const kind = this.kind(name);
    const problems = this.problems(name);
    const entry: CustomizationEntry = { point: 'skill', name, kind, state: problems.length ? 'fallback' : 'active', base: 'none', problems, warnings: [] };
    if (problems.length || kind === 'new') return entry;
    const base = this.snapshot.check(this.paths.baseOf('skill', name), standard);
    const file = this.paths.display(this.paths.customOf('skill', name));
    return { ...entry, base: base.state, warnings: base.warnings.map((w) => ({ ...w, file })), ...(base.diff ? { diff: base.diff } : {}) };
  }

  // Name zulässig, SKILL.md vorhanden, Kopf lesbar, description gesetzt, name im Kopf gleich dem Ordner
  private problems(name: string): Problem[] {
    const dir = this.paths.customOf('skill', name);
    const file = join(dir, 'SKILL.md');
    if (!this.rules.isValid(name)) {
      return [{ code: 'name-invalid', message: `Der Name ${name} ist nicht zulässig (Kleinbuchstaben, Ziffern, einzelne Bindestriche, höchstens 64 Zeichen).`, file: this.paths.display(dir) }];
    }
    if (!existsSync(file)) return [{ code: 'skill-file-missing', message: 'SKILL.md fehlt.', file: this.paths.display(dir) }];
    const { fields, error } = this.front.parse(readFileSync(file, 'utf8'));
    const at = this.paths.display(file);
    // Ein kaputtes YAML nicht als fehlende description ausgeben, sondern als das, was es ist
    if (error) return [{ code: 'front-matter-invalid', message: `Der Kopf ist kein gültiges YAML: ${error}`, file: at }];
    const problems: Problem[] = [];
    if (!this.front.text(fields, 'description')) problems.push({ code: 'description-missing', message: 'Im Kopf fehlt description.', file: at });
    const declared = this.front.text(fields, 'name');
    if (declared && declared !== name) problems.push({ code: 'name-mismatch', message: `name im Kopf (${declared}) passt nicht zum Ordner ${name}.`, file: at });
    return problems;
  }
}
