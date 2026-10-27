import { existsSync, mkdirSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import type { WorkflowPhase } from '../workflow/WorkflowPhases.js';
import { BaseSnapshot } from './BaseSnapshot.js';
import type { CustomizationEntry, CustomizationOverview } from './CustomizationOverview.js';
import { CustomizationPaths } from './CustomizationPaths.js';
import { OutputStyleCustomization } from './OutputStyleCustomization.js';
import { SkillCustomization } from './SkillCustomization.js';
import { WorkflowCustomization } from './WorkflowCustomization.js';
import type { WorkflowTools } from './WorkflowTools.js';

export interface CustomizationInput {
  cloneDir: string;
  customDir: string;
  /** Ordner mit skills/ und output-styles/ des Standards (Manifest oder frisch gebautes Plugin) */
  standardDir: string;
  harnessVersion: string;
  tools: WorkflowTools;
  now?: Date;
}

export interface CheckResult {
  overview: CustomizationOverview;
  /** Phasen des aktiven eigenen Workflows, sonst null */
  phases: WorkflowPhase[] | null;
}

// Ablauf für setup und `npm run customize -- check`: alle Anpassungen prüfen, Übersicht bauen, ins Plugin schreiben.
export class CustomizationApplier {
  // Prüft alle Anpassungen, ohne etwas zu schreiben; aktive eigene Skills gelten für die Verweise im Workflow
  async check(input: CustomizationInput): Promise<CheckResult> {
    const paths = new CustomizationPaths(input.customDir, input.standardDir);
    const skills = new SkillCustomization(paths).check();
    const styles = new OutputStyleCustomization(paths).check();
    const available = [...this.standardSkills(input.standardDir), ...skills.filter((s) => s.state === 'active').map((s) => s.name)];
    const workflow = await new WorkflowCustomization(paths, input.tools).check(available);
    const entries = [...(workflow.entry ? [workflow.entry] : []), ...skills, ...styles];
    return { overview: this.overview(input, entries), phases: workflow.phases };
  }

  // Wendet die Anpassungen auf das gebaute Plugin an und schreibt customizations/ neu
  async apply(input: CustomizationInput, pluginDir: string): Promise<CustomizationOverview> {
    const { overview, phases } = await this.check(input);
    // Gültige Skills und Output-Styles überlagern das gebaute Plugin
    const paths = new CustomizationPaths(input.customDir, input.standardDir);
    new SkillCustomization(paths).apply(overview.entries, pluginDir);
    new OutputStyleCustomization(paths).apply(overview.entries, pluginDir);
    // Alter Ordner aus einem früheren Lauf wird vollständig ersetzt
    const dir = join(pluginDir, 'customizations');
    rmSync(dir, { recursive: true, force: true });
    mkdirSync(dir, { recursive: true });
    if (phases) writeFileSync(join(dir, 'workflow.json'), BaseSnapshot.formatJson(phases));
    writeFileSync(join(dir, 'overview.json'), BaseSnapshot.formatJson(overview));
    return overview;
  }

  // Namen der mitgelieferten Skills (Ordner mit SKILL.md)
  private standardSkills(standardDir: string): string[] {
    const dir = join(standardDir, 'skills');
    if (!existsSync(dir)) return [];
    return readdirSync(dir).filter((name) => existsSync(join(dir, name, 'SKILL.md')));
  }

  // Übersicht mit Zustand des Workflows
  private overview(input: CustomizationInput, entries: CustomizationEntry[]): CustomizationOverview {
    const workflow = entries.find((e) => e.point === 'workflow');
    return {
      version: 1,
      createdAt: (input.now ?? new Date()).toISOString(),
      cloneDir: input.cloneDir,
      customDir: input.customDir,
      harnessVersion: input.harnessVersion,
      workflow: !workflow ? 'standard' : workflow.state === 'active' ? 'custom' : 'fallback',
      entries,
    };
  }
}
