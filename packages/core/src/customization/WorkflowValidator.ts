import type { Problem } from './CustomizationOverview.js';

export interface WorkflowValidation {
  /** Führen zum Rückfall auf den Standard */
  problems: Problem[];
  /** Hinweise, der Workflow bleibt aktiv */
  warnings: Problem[];
}

interface Step {
  step: string;
  command?: string;
  options?: Array<{ command: string }>;
}

interface Phase {
  id: string;
  steps: Step[];
}

const PREFIX = '/ai-harness:';

// Prüft einen geladenen Workflow ohne Dateizugriff: Aufbau, Pflicht-Phasen (Leitplanke), Verweise auf Harness-Skills.
export class WorkflowValidator {
  static readonly REQUIRED = ['planen', 'umsetzen'];
  /** Phasen-IDs, die die Anleitung im Board selbst belegt (EXTRA_PHASES in guideContent.ts) */
  static readonly RESERVED = ['fragen', 'nebenbei'];

  // Aufbau zuerst; nur ein korrekt aufgebauter Workflow wird auf IDs und Verweise geprüft
  validate(value: unknown, skills: string[]): WorkflowValidation {
    const problems = this.shape(value);
    if (problems.length) return { problems, warnings: [] };
    const phases = value as Phase[];
    return { problems: [...this.duplicates(phases), ...this.reserved(phases), ...this.required(phases)], warnings: this.references(phases, skills) };
  }

  // Liste von Phasen mit id, title, hint und steps; Ort im Wert als Pfad wie phases[1].steps[0].when
  private shape(value: unknown): Problem[] {
    if (!Array.isArray(value)) return [this.problem('shape-invalid', 'Der Workflow muss eine Liste von Phasen sein.')];
    const problems: Problem[] = [];
    for (const [p, phase] of value.entries()) {
      const at = `phases[${p}]`;
      problems.push(...this.texts(phase, at, ['id', 'title', 'hint'], []));
      const steps = (phase as { steps?: unknown } | null)?.steps;
      if (!Array.isArray(steps)) {
        problems.push(this.problem('shape-invalid', `${at}.steps muss eine Liste sein.`));
        continue;
      }
      for (const [s, step] of steps.entries()) problems.push(...this.step(step, `${at}.steps[${s}]`));
    }
    return problems;
  }

  // Ein Schritt: step und when Pflicht, command und place optional, options als Liste von {label, command, when}
  private step(step: unknown, at: string): Problem[] {
    const problems = this.texts(step, at, ['step', 'when'], ['command', 'place']);
    const options = (step as { options?: unknown } | null)?.options;
    if (options === undefined) return problems;
    if (!Array.isArray(options)) return [...problems, this.problem('shape-invalid', `${at}.options muss eine Liste sein.`)];
    for (const [o, option] of options.entries()) problems.push(...this.texts(option, `${at}.options[${o}]`, ['label', 'command', 'when'], []));
    return problems;
  }

  // Pflichtfelder müssen Texte sein, optionale Felder Texte oder fehlen
  private texts(value: unknown, at: string, required: string[], optional: string[]): Problem[] {
    if (!value || typeof value !== 'object') return [this.problem('shape-invalid', `${at} muss ein Objekt sein.`)];
    const record = value as Record<string, unknown>;
    const problems: Problem[] = [];
    for (const key of required) {
      if (typeof record[key] !== 'string') problems.push(this.problem('shape-invalid', `${at}.${key} muss ein Text sein.`));
    }
    for (const key of optional) {
      if (record[key] !== undefined && typeof record[key] !== 'string') problems.push(this.problem('shape-invalid', `${at}.${key} muss ein Text sein.`));
    }
    return problems;
  }

  // Jede Phasen-ID darf nur einmal vorkommen
  private duplicates(phases: Phase[]): Problem[] {
    const seen = new Set<string>();
    const problems: Problem[] = [];
    for (const { id } of phases) {
      if (seen.has(id)) problems.push(this.problem('phase-id-duplicate', `Die Phase ${id} kommt mehrfach vor.`));
      seen.add(id);
    }
    return problems;
  }

  // Die IDs der Anleitung im Board dürfen nicht für eigene Phasen verwendet werden
  private reserved(phases: Phase[]): Problem[] {
    return phases
      .filter((p) => WorkflowValidator.RESERVED.includes(p.id))
      .map((p) => this.problem('phase-id-reserved', `Die Phasen-ID ${p.id} ist für die Anleitung reserviert.`));
  }

  // Leitplanke: Pflicht-Phasen planen und umsetzen
  private required(phases: Phase[]): Problem[] {
    const ids = new Set(phases.map((p) => p.id));
    return WorkflowValidator.REQUIRED.filter((id) => !ids.has(id)).map((id) => this.problem('required-phase-missing', `Die Pflicht-Phase ${id} fehlt.`));
  }

  // /ai-harness:<name> muss einen verfügbaren Skill nennen; je Schritt und Name eine Warnung; andere Befehle ungeprüft
  private references(phases: Phase[], skills: string[]): Problem[] {
    const known = new Set(skills);
    const warnings: Problem[] = [];
    for (const step of phases.flatMap((p) => p.steps)) {
      const names = new Set([step.command, ...(step.options ?? []).map((o) => o.command)].map((c) => this.skillName(c)));
      for (const name of names) {
        if (name && !known.has(name)) warnings.push(this.problem('skill-ref-missing', `/ai-harness:${name} gibt es nicht (Schritt „${step.step}“).`));
      }
    }
    return warnings;
  }

  // Name des Harness-Skills in einem Befehl, sonst null
  private skillName(command: string | undefined): string | null {
    if (!command?.startsWith(PREFIX)) return null;
    return command.slice(PREFIX.length).split(/\s/)[0] || null;
  }

  // Baut ein Problem ohne Ort (den Ort ergänzt der Aufrufer)
  private problem(code: string, message: string): Problem {
    return { code, message };
  }
}
