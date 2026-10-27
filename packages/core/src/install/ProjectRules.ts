import { HarnessHome } from '../registry/HarnessHome.js';
import { BOARD_RULE_PATH, ProjectBoardRule } from './ProjectBoardRule.js';
import { DOCS_RULE_PATH, ProjectDocsRule } from './ProjectDocsRule.js';
import type { ProjectRule } from './ProjectRule.js';

// Orte aller Rules, die der Harness im Projekt anlegt; sie zählen bei Sauberkeitsprüfungen nie als Änderung
export const PROJECT_RULE_PATHS: readonly string[] = [DOCS_RULE_PATH, BOARD_RULE_PATH];

// Alle Rules, die `harness init` und das Anlegen eines Worktrees ins Projekt schreiben
export class ProjectRules {
  readonly docs: ProjectDocsRule;
  readonly board: ProjectBoardRule;

  constructor(home: HarnessHome = new HarnessHome()) {
    this.docs = new ProjectDocsRule(home);
    this.board = new ProjectBoardRule(home);
  }

  // Alle Rules in fester Reihenfolge
  all(): ProjectRule[] {
    return [this.docs, this.board];
  }

  // Schreibt alle fehlenden oder veralteten Rules; liefert die neu geschriebenen
  async ensure(projectDir: string): Promise<ProjectRule[]> {
    const written: ProjectRule[] = [];
    for (const rule of this.all()) if (await rule.ensure(projectDir)) written.push(rule);
    return written;
  }
}
