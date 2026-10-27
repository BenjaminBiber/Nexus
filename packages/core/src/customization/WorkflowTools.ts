import type { Problem } from './CustomizationOverview.js';
// Werkzeuge zum Prüfen und Laden von custom/workflow.ts; austauschbar, damit Tests ohne tsc und esbuild auskommen.
export interface WorkflowTools {
  /** Typfehler der Datei gegen WorkflowPhase[]; leer, wenn alles passt */
  typecheck(file: string): Problem[];
  /** Default-Export der Datei als reine Daten */
  load(file: string): Promise<unknown>;
}
