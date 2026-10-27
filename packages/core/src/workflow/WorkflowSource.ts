import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import type { HarnessHome } from '../registry/HarnessHome.js';
import { WORKFLOW_PHASES, type WorkflowPhase } from './WorkflowPhases.js';

export interface WorkflowSelection {
  source: 'standard' | 'custom';
  phases: WorkflowPhase[];
}

// Liefert den angewendeten Workflow aus `~/.harness/customizations/workflow.json`, sonst den Standard.
export class WorkflowSource {
  constructor(private readonly home: HarnessHome) {}

  // Fehlende, unlesbare oder leere Datei ergibt den Standard (geprüft wurde schon beim Anwenden)
  read(): WorkflowSelection {
    try {
      const phases: unknown = JSON.parse(readFileSync(join(this.home.customizationsDir, 'workflow.json'), 'utf8'));
      if (Array.isArray(phases) && phases.length) return { source: 'custom', phases: phases as WorkflowPhase[] };
    } catch {
      /* Standard */
    }
    return { source: 'standard', phases: WORKFLOW_PHASES };
  }
}
