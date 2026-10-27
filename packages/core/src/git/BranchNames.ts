import { GitRunner } from './GitRunner.js';
import { TargetBranchRule } from './TargetBranchRule.js';

// Liest die Branch-Namen eines Repos für die Vorschlagsliste des Zielbranches: lokale Branches und die von origin
// (ohne Präfix zusammengeführt), gefiltert auf das, was als Zielbranch gespeichert werden darf. Wirft nie.
export class BranchNames {
  constructor(private readonly limit = 500, private readonly git = new GitRunner(), private readonly rule = new TargetBranchRule()) {}

  list(root: string): string[] {
    const result = this.git.tryRun(root, ['for-each-ref', '--format=%(refname:short)', 'refs/heads', 'refs/remotes/origin']);
    if (!result.ok || !result.out) return [];
    const names = new Set<string>();
    for (const line of result.out.split('\n')) {
      // Git kürzt refs/remotes/origin/HEAD zu „origin“ – der Zeiger auf den Standardbranch ist kein eigener Branch
      const short = line.trim();
      if (short === 'origin' || short === 'origin/HEAD') continue;
      const name = short.replace(/^origin\//, '');
      if (name === '' || name === 'HEAD') continue;
      if (this.allowed(name)) names.add(name);
    }
    return [...names].sort((a, b) => a.localeCompare(b)).slice(0, this.limit);
  }

  // Darf der Name als Zielbranch gespeichert werden? `normalize` wirft bei ungültigen Namen und dem Namensraum der Tickets
  private allowed(name: string): boolean {
    try {
      return this.rule.normalize(name) !== null;
    } catch {
      return false;
    }
  }
}
