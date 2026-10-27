import { HarnessHome } from '../registry/HarnessHome.js';
import { ProjectRule } from './ProjectRule.js';

// Ort der Rule im Projekt, relativ zur Wurzel (Hauptordner oder Worktree)
export const BOARD_RULE_PATH = '.claude/rules/ai-harness-board.md';

// Rule ohne Eingrenzung (immer geladen), die die zentrale Board-Regel (`~/.harness/rules/board.md`) lädt:
// Tickets, Board, Fragen und Notizen meinen immer das AI-Harness-Board
export class ProjectBoardRule extends ProjectRule {
  constructor(home: HarnessHome = new HarnessHome()) {
    super({ path: BOARD_RULE_PATH, importFile: 'board.md' }, home);
  }
}
