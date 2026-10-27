// Typen der Übersicht über die Anpassungen (data-model.md); Grundlage für Ausgabe, Panel und Skill.
export type ExtensionPoint = 'workflow' | 'skill' | 'output-style';
export type BaseState = 'current' | 'outdated' | 'unknown';
export interface Problem {
  code: string;
  message: string;
  /** Pfad relativ zum Clone, mit `/` */
  file?: string;
  line?: number;
}
export interface CustomizationEntry {
  point: ExtensionPoint;
  name: string;
  kind: 'replace' | 'new';
  state: 'active' | 'fallback';
  base: BaseState | 'none';
  /** Gründe für den Rückfall auf den Standard */
  problems: Problem[];
  /** Hinweise ohne Rückfall */
  warnings: Problem[];
  /** Unterschiede Basis → Standard, nur bei veralteter Basis */
  diff?: string;
}
export interface CustomizationOverview {
  version: 1;
  createdAt: string;
  cloneDir: string;
  customDir: string;
  harnessVersion: string;
  workflow: 'standard' | 'custom' | 'fallback';
  entries: CustomizationEntry[];
}
export const CUSTOMIZE_COMMAND = '/ai-harness:harness-customize';
