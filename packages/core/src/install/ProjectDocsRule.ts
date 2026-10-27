import { HarnessHome } from '../registry/HarnessHome.js';
import { ProjectRule } from './ProjectRule.js';

// Ort der Rule im Projekt, relativ zur Wurzel (Hauptordner oder Worktree)
export const DOCS_RULE_PATH = '.claude/rules/ai-harness-docs.md';

// Rule, die beim Zugriff auf einen `docs/`-Ordner die zentralen Doku-Regeln (`~/.harness/rules/docs.md`) lädt
export class ProjectDocsRule extends ProjectRule {
  constructor(home: HarnessHome = new HarnessHome()) {
    super({ path: DOCS_RULE_PATH, importFile: 'docs.md', paths: ['**/docs/**'] }, home);
  }
}
