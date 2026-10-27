import { CUSTOMIZE_COMMAND, type CustomizationEntry, type CustomizationOverview, type ExtensionPoint, type Problem } from './CustomizationOverview.js';

const WORKFLOW_STATE = { standard: 'Standard', custom: 'angepasst', fallback: 'Rückfall auf Standard' } as const;

// Text der Übersicht für setup und `npm run customize -- check` (Format: contracts/customize-script.md).
export class OverviewFormatter {
  // Anzeigename eines Erweiterungspunkts mit Name, z. B. „Skill harness-grill“
  static label(point: ExtensionPoint, name: string): string {
    if (point === 'workflow') return 'Workflow';
    return `${point === 'skill' ? 'Skill' : 'Output-Style'} ${name}`;
  }

  // Ganze Übersicht; Diffs je Warnung auf maxDiffLines Zeilen gekürzt (Standard 40)
  format(overview: CustomizationOverview, options: { maxDiffLines?: number } = {}): string {
    if (!overview.entries.length) return `Keine Anpassungen, überall gilt der Standard.\nÄndern: ${CUSTOMIZE_COMMAND}`;
    const max = options.maxDiffLines ?? 40;
    const lines = [`Anpassungen (custom/ in ${overview.customDir})`, ...this.summary(overview)];
    const warned = overview.entries.filter((e) => e.warnings.length);
    if (warned.length) lines.push('', 'Warnungen', ...warned.flatMap((e) => this.warnings(e, max)));
    const fallbacks = overview.entries.filter((e) => e.state === 'fallback');
    if (fallbacks.length) lines.push('', 'Rückfälle auf den Standard', ...fallbacks.flatMap((e) => e.problems.map((p) => `  ✗ ${this.name(e)}: ${p.message}${this.where(p)}`)));
    lines.push('', `Ändern: ${CUSTOMIZE_COMMAND}`);
    return lines.join('\n');
  }

  // Je Erweiterungspunkt eine Zeile
  private summary(overview: CustomizationOverview): string[] {
    return [
      `  Workflow        ${WORKFLOW_STATE[overview.workflow]}`,
      `  Skills          ${this.names(overview, 'skill')}`,
      `  Output-Styles   ${this.names(overview, 'output-style')}`,
    ];
  }

  // Aktive Anpassungen eines Punkts mit Art, sonst „Standard“
  private names(overview: CustomizationOverview, point: ExtensionPoint): string {
    const active = overview.entries.filter((e) => e.point === point && e.state === 'active');
    return active.length ? active.map((e) => `${e.name} (${e.kind === 'replace' ? 'ersetzt' : 'neu'})`).join(', ') : 'Standard';
  }

  // Warnungen eines Eintrags, darunter der Diff eingerückt und gekürzt
  private warnings(entry: CustomizationEntry, max: number): string[] {
    const lines = entry.warnings.map((w) => `  ! ${this.name(entry)}: ${w.message}`);
    if (!entry.diff) return lines;
    const diff = entry.diff.split('\n');
    lines.push(...diff.slice(0, max).map((line) => `      ${line}`));
    if (diff.length > max) lines.push(`      … ${diff.length - max} weitere Zeilen: npm run customize -- check`);
    return lines;
  }

  // Anzeigename eines Eintrags
  private name(entry: CustomizationEntry): string {
    return OverviewFormatter.label(entry.point, entry.name);
  }

  // Ort in Klammern, mit Zeile falls bekannt
  private where(problem: Problem): string {
    if (!problem.file) return '';
    return ` (${problem.file}${problem.line ? `:${problem.line}` : ''})`;
  }
}
