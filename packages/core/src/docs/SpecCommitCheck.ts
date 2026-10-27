import { existsSync } from 'node:fs';
import { isAbsolute, join } from 'node:path';
import { GitRunner } from '../git/GitRunner.js';

// Prüft, ob das Spec-Dokument eines Tickets vollständig committet ist: weder geändert noch ungetrackt und im HEAD vorhanden.
// Worktrees und Agenten sehen nur den letzten Commit; die Prüfung liefert dafür eine Warnung (nie einen Abbruch).
export class SpecCommitCheck {
  constructor(private readonly git: Pick<GitRunner, 'tryRun'> = new GitRunner()) {}

  // Warnung zum Spec-Dokument (Pfad relativ zu `root`) oder null: alles committet, kein Git-Repo, leerer Pfad
  // oder die Datei gibt es nicht (z. B. ein abgeschnittener Pfad am Ticket)
  warningForDoc(root: string, docPath: string): string | null {
    if (!docPath) return null;
    if (!this.git.tryRun(root, ['rev-parse', '--is-inside-work-tree']).ok) return null;
    if (!existsSync(isAbsolute(docPath) ? docPath : join(root, docPath))) return null;
    return this.committed(root, docPath) ? null : SpecCommitCheck.message(docPath);
  }

  // Text der Warnung
  static message(docPath: string): string {
    return `Das Spec-Dokument ${docPath} ist nicht (vollständig) committet – Worktrees und Agenten sehen nur den letzten Commit. Erst committen.`;
  }

  // true, wenn das Dokument im HEAD liegt und weder geändert noch ungetrackt ist
  private committed(root: string, docPath: string): boolean {
    const status = this.git.tryRun(root, ['status', '--porcelain', '--untracked-files=all', '--', docPath]);
    if (!status.ok || status.out !== '') return false;
    const tree = this.git.tryRun(root, ['ls-tree', '--name-only', 'HEAD', '--', docPath]);
    return tree.ok && tree.out !== '';
  }
}
