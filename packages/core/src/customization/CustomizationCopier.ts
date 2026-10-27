import { cpSync, existsSync, mkdirSync, rmSync, writeFileSync } from 'node:fs';
import { dirname } from 'node:path';
import { HarnessError } from '../errors/HarnessError.js';
import { WORKFLOW_PHASES } from '../workflow/WorkflowPhases.js';
import { BaseSnapshot } from './BaseSnapshot.js';
import type { ExtensionPoint } from './CustomizationOverview.js';
import type { CustomizationPaths } from './CustomizationPaths.js';
import { NameRules } from './NameRules.js';
import { OverviewFormatter } from './OverviewFormatter.js';

export interface CopyTarget {
  point: ExtensionPoint;
  name: string;
}

// Vorgänge des Skills: Standard als Ausgangspunkt kopieren (mit Basis), nur die Basis erneuern, Anpassung löschen.
export class CustomizationCopier {
  constructor(
    private readonly paths: CustomizationPaths,
    private readonly snapshot = new BaseSnapshot(),
    private readonly rules = new NameRules(),
  ) {}

  // Kopiert den aktuellen Standard nach custom/ und vermerkt die Basis; mit rebase nur die Basis erneuern
  copy(target: CopyTarget, options: { rebase?: boolean } = {}): string {
    this.checkName(target);
    const custom = this.paths.customOf(target.point, target.name);
    const label = OverviewFormatter.label(target.point, target.name);
    // Nur Basis erneuern: die Anpassung bleibt unverändert
    if (options.rebase) {
      if (!existsSync(custom)) throw new HarnessError('customization-missing', `${label} ist nicht angepasst.`);
      this.saveBase(target);
      return `Basis von ${label} auf den aktuellen Standard gesetzt. Wirkt nach npm run setup.`;
    }
    if (existsSync(custom)) {
      throw new HarnessError('customization-exists', `${label} ist schon angepasst (${this.paths.display(custom)}). Nur die Basis erneuern: copy mit --rebase.`);
    }
    // Standard zuerst prüfen, damit ein abgelehnter Vorgang keinen leeren Ordner hinterlässt
    const source = target.point === 'workflow' ? null : this.standard(target);
    mkdirSync(dirname(custom), { recursive: true });
    if (source === null) writeFileSync(custom, this.workflowSource());
    else cpSync(source, custom, { recursive: true });
    this.saveBase(target);
    return `${label} nach ${this.paths.display(custom)} kopiert. Wirkt nach npm run setup.`;
  }

  // Löscht Anpassung und Basis; danach gilt nach npm run setup wieder der Standard
  reset(target: CopyTarget): string {
    this.checkName(target);
    const custom = this.paths.customOf(target.point, target.name);
    const label = OverviewFormatter.label(target.point, target.name);
    if (!existsSync(custom)) throw new HarnessError('customization-missing', `${label} ist nicht angepasst.`);
    rmSync(custom, { recursive: true, force: true });
    rmSync(this.paths.baseOf(target.point, target.name), { recursive: true, force: true });
    return `${label} gelöscht. Wirkt nach npm run setup: dann gilt wieder der Standard.`;
  }

  // Verhindert Pfadausbrüche: Skill- und Output-Style-Namen müssen den Namensregeln genügen (Workflow ist fest)
  private checkName(target: CopyTarget): void {
    if (target.point === 'workflow' || this.rules.isValid(target.name)) return;
    throw new HarnessError('customization-invalid-name', `Der Name ${target.name} ist nicht zulässig (Kleinbuchstaben, Ziffern, einzelne Bindestriche, höchstens 64 Zeichen).`);
  }

  // Basis: beim Workflow der Standard als JSON, sonst eine Kopie der Standard-Datei bzw. des Ordners
  private saveBase(target: CopyTarget): void {
    const base = this.paths.baseOf(target.point, target.name);
    if (target.point === 'workflow') this.snapshot.saveText(BaseSnapshot.formatJson(WORKFLOW_PHASES), base);
    else this.snapshot.save(this.standard(target), base);
  }

  // Ort des Standards; fehlt er, ist es kein Ersatz, sondern etwas Neues
  private standard(target: CopyTarget): string {
    const standard = this.paths.standardOf(target.point, target.name);
    if (!standard || !existsSync(standard)) {
      const custom = this.paths.display(this.paths.customOf(target.point, target.name));
      throw new HarnessError('customization-no-standard', `Einen mitgelieferten ${OverviewFormatter.label(target.point, target.name)} gibt es nicht. Neue legst du direkt in ${custom} an.`);
    }
    return standard;
  }

  // Ausgangspunkt für custom/workflow.ts: der Standard als Literal mit Typprüfung
  private workflowSource(): string {
    return [
      "import type { WorkflowPhase } from '@harness/core';",
      '',
      '// Eigener Workflow: ersetzt den Standard vollständig. Pflicht-Phasen: planen, umsetzen. Reserviert: fragen, nebenbei. Wirkt nach npm run setup.',
      `export default ${JSON.stringify(WORKFLOW_PHASES, null, 2)} satisfies WorkflowPhase[];`,
      '',
    ].join('\n');
  }
}
