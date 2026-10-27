import { existsSync } from 'node:fs';
import { basename, dirname, join } from 'node:path';
import { GitPaths } from '../../git/GitPaths.js';
import { PathKey } from '../../git/PathKey.js';
import { GitRunner } from '../../git/GitRunner.js';
import { LONG_PATHS_ARGS, LONG_PATHS_NOTICE, LongPaths } from '../../git/LongPaths.js';
import { HarnessError } from '../../errors/HarnessError.js';
import { ClaudeImportApproval } from '../../install/ClaudeImportApproval.js';
import { ProjectPluginSwitch } from '../../install/ProjectPluginSwitch.js';
import { ProjectRules } from '../../install/ProjectRules.js';
import { WorktreeLinks } from '../../worktrees/WorktreeLinks.js';

export interface ResearchWorktreeResult {
  path: string;
  /** Plugin oder Rules konnten nicht eingerichtet werden (nur Hinweis) */
  warning?: string;
}

// Losgelöster Worktree (`git worktree add --detach … HEAD`) für einen Recherche-Lauf zu einer Frage: der Hauptordner
// des Nutzers bleibt unberührt, kein Branch entsteht. Liegt neben den Ticket-Worktrees unter <repo>-worktrees/research-<ID>.
// Mit Präfix „plan“ derselbe Worktree für einen Planungslauf an einem Ticket (<repo>-worktrees/plan-<ID>).
export class ResearchWorktree {
  private readonly git = new GitRunner();
  private readonly longPaths = new LongPaths();
  private readonly pluginSwitch = new ProjectPluginSwitch();
  private readonly rules = new ProjectRules();
  private readonly importApproval = new ClaudeImportApproval();
  private readonly links = new WorktreeLinks();
  private readonly keys = new PathKey();
  /** Bezeichnung in Meldungen */
  private readonly label: string;

  constructor(private readonly prefix: 'research' | 'plan' = 'research') {
    this.label = prefix === 'plan' ? 'Planungs-Worktree' : 'Recherche-Worktree';
  }

  // Pfad des Worktrees einer Frage (Recherche) bzw. eines Tickets (Planung)
  path(root: string, questionId: string): string {
    return join(dirname(root), `${basename(root)}-worktrees`, `${this.prefix}-${questionId}`).replace(/\\/g, '/');
  }

  // Legt den Worktree an (ein Rest eines früheren Laufs wird vorher entfernt) und schaltet Plugin und Rules ein
  async create(root: string, questionId: string): Promise<ResearchWorktreeResult> {
    const path = this.path(root, questionId);
    if (existsSync(path)) this.remove(root, path);
    if (existsSync(path)) {
      throw new HarnessError(`${this.prefix}-worktree-busy`, `Der Ordner ${path} existiert schon und ist kein entfernbarer Worktree – bitte von Hand prüfen.`);
    }
    const warnings: string[] = [];
    if (this.longPaths.ensure(root)) warnings.push(LONG_PATHS_NOTICE);
    this.git.run(root, [...LONG_PATHS_ARGS, 'worktree', 'add', '--detach', path, 'HEAD']);
    try {
      await this.pluginSwitch.enable(path);
      await this.rules.ensure(path);
      await this.importApproval.approve(path);
    } catch (error) {
      warnings.push(`Plugin oder Rules im ${this.label} nicht eingerichtet: ${(error as Error).message}`);
    }
    return { path, warning: warnings.join(' ') || undefined };
  }

  // Entfernt genau diesen Worktree (mit --force: er enthält nur Recherche-Ausgaben) und räumt die Git-Verwaltung auf;
  // liefert eine Fehlermeldung oder null. Entfernt wird nur ein Ordner research-<ID>, der genau am erwarteten Platz unter
  // <repo>-worktrees liegt – andere Ordner nie
  remove(root: string, path: string): string | null {
    if (!this.isResearchPath(root, path)) return `${path} ist kein ${this.label} – nicht entfernt.`;
    // Links (Symlinks/Junctions) zuerst entfernen, nie ihr Ziel; bleibt einer stehen, wird nichts gelöscht
    const links = existsSync(path) ? this.links.removeAll(path) : { removed: [], failed: [] };
    if (links.failed.length) return `${this.label} ${path} nicht entfernt: Links ließen sich nicht entfernen (${links.failed.join(', ')}).`;
    const attempt = this.git.tryRun(root, ['worktree', 'remove', '--force', path]);
    this.git.tryRun(root, ['worktree', 'prune']);
    return attempt.ok || !existsSync(path) ? null : `${this.label} ${path} nicht entfernt: ${attempt.err}`;
  }

  // Entfernt verwaiste Recherche-Worktrees eines Repos (`repoPath` darf auch ein Unterordner sein): eingetragene Worktrees
  // research-<ID> am erwarteten Platz, zu denen kein Lauf gehört (`active`: Ordner laufender Recherchen). Liefert je
  // Worktree eine Meldung (entfernt oder Fehler); ist der Ordner kein Git-Repo, nichts
  removeOrphans(repoPath: string, active: string[]): string[] {
    let root: string;
    try {
      root = new GitPaths().mainRoot(repoPath);
    } catch {
      return [];
    }
    const list = this.git.tryRun(root, ['worktree', 'list', '--porcelain']);
    if (!list.ok) return [];
    const busy = new Set(active.map((p) => this.keys.of(p)));
    const notes: string[] = [];
    for (const line of list.out.split(/\r?\n/)) {
      if (!line.startsWith('worktree ')) continue;
      const path = line.slice('worktree '.length).trim();
      if (!this.isResearchPath(root, path) || busy.has(this.keys.of(path))) continue;
      notes.push(this.remove(root, path) ?? `verwaister ${this.label} ${path} entfernt`);
    }
    return notes;
  }

  // Liegt `path` genau dort, wo der Recherche-Worktree der Frage aus seinem Namen research-<ID> läge?
  private isResearchPath(root: string, path: string): boolean {
    const match = new RegExp(`^${this.prefix}-([A-Za-z0-9-]+)$`).exec(basename(path));
    return match !== null && this.keys.same(path, this.path(root, match[1]));
  }
}
