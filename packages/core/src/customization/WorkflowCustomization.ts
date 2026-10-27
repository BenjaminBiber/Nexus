import { existsSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { WORKFLOW_PHASES, type WorkflowPhase } from '../workflow/WorkflowPhases.js';
import { BaseSnapshot, type BaseCheck } from './BaseSnapshot.js';
import type { CustomizationEntry, Problem } from './CustomizationOverview.js';
import type { CustomizationPaths } from './CustomizationPaths.js';
import { WorkflowValidator } from './WorkflowValidator.js';
import type { WorkflowTools } from './WorkflowTools.js';

export interface WorkflowResult {
  /** null, wenn es keinen eigenen Workflow gibt */
  entry: CustomizationEntry | null;
  /** Geladene Phasen bei aktivem eigenen Workflow, sonst null (Standard gilt) */
  phases: WorkflowPhase[] | null;
}

// Bewertet custom/workflow.ts: Typprüfung, Laden, Regeln, Basis; aktiv oder Rückfall auf den Standard.
export class WorkflowCustomization {
  constructor(
    private readonly paths: CustomizationPaths,
    private readonly tools: WorkflowTools,
    private readonly validator = new WorkflowValidator(),
    private readonly snapshot = new BaseSnapshot(),
  ) {}

  // Ohne Datei kein Eintrag; ein unerwarteter Fehler in der Prüfung führt zum Rückfall statt zum Abbruch
  async check(skills: string[]): Promise<WorkflowResult> {
    if (!existsSync(this.paths.workflowFile)) return { entry: null, phases: null };
    try {
      return await this.evaluate(skills);
    } catch (error) {
      return this.fallback([{ code: 'check-failed', message: `Prüfung fehlgeschlagen: ${(error as Error).message}` }]);
    }
  }

  // Prüft in Reihenfolge; das erste Problem einer Stufe führt zum Rückfall
  private async evaluate(skills: string[]): Promise<WorkflowResult> {
    const file = this.paths.workflowFile;
    const typeProblems = this.tools.typecheck(file);
    if (typeProblems.length) return this.fallback(typeProblems);

    let value: unknown;
    try {
      value = await this.tools.load(file);
    } catch (error) {
      return this.fallback([{ code: 'load-failed', message: `Der Workflow ließ sich nicht laden: ${(error as Error).message}` }]);
    }

    const { problems, warnings } = this.validator.validate(value, skills);
    if (problems.length) return this.fallback(problems);

    // Basis nur prüfen, wenn der Workflow gültig ist
    const base = this.base();
    const entry: CustomizationEntry = { ...this.entry('active', []), base: base.state, warnings: this.located([...warnings, ...base.warnings]) };
    if (base.diff) entry.diff = base.diff;
    return { entry, phases: value as WorkflowPhase[] };
  }

  // Rückfall: Standard gilt, Gründe mit Ort
  private fallback(problems: Problem[]): WorkflowResult {
    return { entry: this.entry('fallback', this.located(problems)), phases: null };
  }

  // Grundgerüst des Eintrags; ein eigener Workflow ersetzt immer den Standard
  private entry(state: 'active' | 'fallback', problems: Problem[]): CustomizationEntry {
    return { point: 'workflow', name: 'workflow', kind: 'replace', state, base: 'none', problems, warnings: [] };
  }

  // Ort relativ zum Clone; ohne Ort gilt custom/workflow.ts
  private located(problems: Problem[]): Problem[] {
    return problems.map((p) => ({ ...p, file: this.paths.display(p.file ?? this.paths.workflowFile) }));
  }

  // Vergleicht die Basis mit dem aktuellen Standard (als formatiertes JSON in einer temporären Datei)
  private base(): BaseCheck {
    const dir = mkdtempSync(join(tmpdir(), 'harness-workflow-std-'));
    try {
      const standard = join(dir, 'workflow.json');
      writeFileSync(standard, BaseSnapshot.formatJson(WORKFLOW_PHASES));
      return this.snapshot.check(this.paths.baseOf('workflow', 'workflow'), standard);
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  }
}
