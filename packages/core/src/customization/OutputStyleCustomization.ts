import { cpSync, existsSync, mkdirSync, readdirSync, readFileSync, rmSync } from 'node:fs';
import { basename, join } from 'node:path';
import { BaseSnapshot } from './BaseSnapshot.js';
import type { CustomizationEntry, Problem } from './CustomizationOverview.js';
import type { CustomizationPaths } from './CustomizationPaths.js';
import { FrontMatter } from './FrontMatter.js';
import { NameRules } from './NameRules.js';

// Eigene Output-Styles in custom/output-styles/<name>.md: prüfen und über die des gebauten Plugins legen.
export class OutputStyleCustomization {
  constructor(
    private readonly paths: CustomizationPaths,
    private readonly snapshot = new BaseSnapshot(),
    private readonly rules = new NameRules(),
    private readonly front = new FrontMatter(),
  ) {}

  // Ein Eintrag je .md-Datei, nach Name sortiert; andere Dateien zählen nicht
  check(): CustomizationEntry[] {
    if (!existsSync(this.paths.outputStylesDir)) return [];
    return readdirSync(this.paths.outputStylesDir)
      .filter((file) => file.endsWith('.md'))
      .map((file) => basename(file, '.md'))
      .sort()
      .map((name) => this.safeEntry(name));
  }

  // Aktive Output-Styles ersetzen die gleichnamige Datei oder kommen neu dazu
  apply(entries: CustomizationEntry[], pluginDir: string): void {
    for (const entry of entries.filter((e) => e.point === 'output-style' && e.state === 'active')) {
      const target = join(pluginDir, 'output-styles', `${entry.name}.md`);
      mkdirSync(join(pluginDir, 'output-styles'), { recursive: true });
      rmSync(target, { force: true });
      cpSync(this.paths.customOf('output-style', entry.name), target);
    }
  }

  // Wie entry, aber ein unerwarteter Fehler betrifft nur diesen Output-Style und führt zum Rückfall
  private safeEntry(name: string): CustomizationEntry {
    try {
      return this.entry(name);
    } catch (error) {
      const file = this.paths.display(this.paths.customOf('output-style', name));
      const problem: Problem = { code: 'check-failed', message: `Prüfung fehlgeschlagen: ${(error as Error).message}`, file };
      return { point: 'output-style', name, kind: this.kind(name), state: 'fallback', base: 'none', problems: [problem], warnings: [] };
    }
  }

  // Ein Standard-Output-Style gleichen Namens wird ersetzt, sonst ist er neu
  private kind(name: string): 'replace' | 'new' {
    return existsSync(this.paths.standardOf('output-style', name)!) ? 'replace' : 'new';
  }

  // Bewertet einen Output-Style: Rückfall bei Problemen, sonst aktiv; ersetzte mit Basisvergleich
  private entry(name: string): CustomizationEntry {
    const standard = this.paths.standardOf('output-style', name)!;
    const kind = this.kind(name);
    const problems = this.problems(name);
    const entry: CustomizationEntry = { point: 'output-style', name, kind, state: problems.length ? 'fallback' : 'active', base: 'none', problems, warnings: [] };
    if (problems.length || kind === 'new') return entry;
    const base = this.snapshot.check(this.paths.baseOf('output-style', name), standard);
    const file = this.paths.display(this.paths.customOf('output-style', name));
    return { ...entry, base: base.state, warnings: base.warnings.map((w) => ({ ...w, file })), ...(base.diff ? { diff: base.diff } : {}) };
  }

  // Name zulässig, Kopf lesbar, description gesetzt, name im Kopf gleich dem Dateinamen
  private problems(name: string): Problem[] {
    const file = this.paths.customOf('output-style', name);
    const at = this.paths.display(file);
    if (!this.rules.isValid(name)) {
      return [{ code: 'name-invalid', message: `Der Name ${name} ist nicht zulässig (Kleinbuchstaben, Ziffern, einzelne Bindestriche, höchstens 64 Zeichen).`, file: at }];
    }
    const { fields, error } = this.front.parse(readFileSync(file, 'utf8'));
    // Ein kaputtes YAML nicht als fehlende description ausgeben, sondern als das, was es ist
    if (error) return [{ code: 'front-matter-invalid', message: `Der Kopf ist kein gültiges YAML: ${error}`, file: at }];
    const problems: Problem[] = [];
    if (!this.front.text(fields, 'description')) problems.push({ code: 'description-missing', message: 'Im Kopf fehlt description.', file: at });
    const declared = this.front.text(fields, 'name');
    if (declared && declared !== name) problems.push({ code: 'name-mismatch', message: `name im Kopf (${declared}) passt nicht zum Dateinamen ${name}.`, file: at });
    return problems;
  }
}
