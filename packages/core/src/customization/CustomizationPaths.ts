import { dirname, join, relative, sep } from 'node:path';
import type { ExtensionPoint } from './CustomizationOverview.js';
// Orte der Anpassungen in custom/, ihrer Basis in custom/.base/ und des Standards (Manifest oder gebautes Plugin).
export class CustomizationPaths {
  constructor(
    readonly customDir: string,
    readonly standardDir: string,
  ) {}
  /** Datei des Workflows in custom/ */
  get workflowFile(): string { return join(this.customDir, 'workflow.ts'); }
  /** Ordner der Skills in custom/ */
  get skillsDir(): string { return join(this.customDir, 'skills'); }
  /** Ordner der Output-Styles in custom/ */
  get outputStylesDir(): string { return join(this.customDir, 'output-styles'); }
  /** Ordner der Basis in custom/.base/ */
  get baseDir(): string { return join(this.customDir, '.base'); }
  /** Ort der Anpassung in custom/ */
  customOf(point: ExtensionPoint, name: string): string {
    if (point === 'workflow') return this.workflowFile;
    return point === 'skill' ? join(this.skillsDir, name) : join(this.outputStylesDir, `${name}.md`);
  }
  // Basis hat den gleichen relativen Pfad; der Workflow liegt als JSON vor
  /** Ort der Basis in custom/.base/ */
  baseOf(point: ExtensionPoint, name: string): string {
    if (point === 'workflow') return join(this.baseDir, 'workflow.json');
    return point === 'skill' ? join(this.baseDir, 'skills', name) : join(this.baseDir, 'output-styles', `${name}.md`);
  }
  // Den Workflow gibt es nur als Daten (WORKFLOW_PHASES), daher kein Standard-Pfad
  /** Ort des Standards, null beim Workflow */
  standardOf(point: ExtensionPoint, name: string): string | null {
    if (point === 'workflow') return null;
    return point === 'skill' ? join(this.standardDir, 'skills', name) : join(this.standardDir, 'output-styles', `${name}.md`);
  }
  /** Pfad relativ zum Clone mit `/`, für Meldungen */
  display(file: string): string {
    // Windows liefert `\`, Meldungen brauchen immer `/`
    return relative(dirname(this.customDir), file).split(sep).join('/');
  }
}
