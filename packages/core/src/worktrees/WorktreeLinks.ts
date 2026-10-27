import { lstatSync, readdirSync, rmdirSync, unlinkSync } from 'node:fs';
import { join } from 'node:path';

export interface LinkRemoval {
  /** Entfernte Links (nur der Link selbst, nie sein Ziel) */
  removed: string[];
  /** Links, die sich nicht entfernen ließen – dann darf der Ordner nicht gelöscht werden */
  failed: string[];
}

// Findet Symlinks und Junctions (Windows-Reparse-Points) in einem Worktree, ohne ihnen zu folgen, und entfernt nur die
// Links selbst. Muss vor jedem `git worktree remove` laufen: Git löscht sonst womöglich durch eine Junction hindurch den
// Inhalt des Ziels (z. B. node_modules des Hauptordners).
export class WorktreeLinks {
  // Alle Links unterhalb von `root` (rekursiv, aber nie in einen Link hinein); `root` selbst als Link zählt mit
  find(root: string, unreadable: string[] = []): string[] {
    const found: string[] = [];
    if (this.isLink(root)) return [root];
    const visit = (dir: string): void => {
      let names: string[];
      try {
        names = readdirSync(dir);
      } catch {
        // Nicht lesbar: darin könnte ein Link stecken – der Aufrufer darf dann nicht löschen
        unreadable.push(dir);
        return;
      }
      for (const name of names) {
        const path = join(dir, name);
        let stat;
        try {
          stat = lstatSync(path);
        } catch {
          unreadable.push(path);
          continue;
        }
        if (stat.isSymbolicLink()) found.push(path);
        else if (stat.isDirectory()) visit(path);
      }
    };
    visit(root);
    return found;
  }

  // Entfernt alle Links unterhalb von `root` (nie rekursiv, das Ziel bleibt); `root` selbst als Link wird nie angefasst
  removeAll(root: string): LinkRemoval {
    const result: LinkRemoval = { removed: [], failed: [] };
    if (this.isLink(root)) return { removed: [], failed: [root] };
    const unreadable: string[] = [];
    for (const link of this.find(root, unreadable)) {
      if (this.removeLink(link)) result.removed.push(link);
      else result.failed.push(link);
    }
    // Nicht durchsuchbare Ordner zählen als gescheitert, damit der Worktree stehen bleibt
    result.failed.push(...unreadable);
    return result;
  }

  // Entfernt genau den Link: Datei-Symlink per unlink, Junction/Verzeichnis-Link per rmdir (ohne recursive)
  private removeLink(path: string): boolean {
    try {
      unlinkSync(path);
    } catch {
      try {
        rmdirSync(path);
      } catch {
        return false;
      }
    }
    try {
      lstatSync(path);
      return false;
    } catch {
      return true;
    }
  }

  // Ist der Pfad selbst ein Link (Symlink oder Junction)?
  private isLink(path: string): boolean {
    try {
      return lstatSync(path).isSymbolicLink();
    } catch {
      return false;
    }
  }
}
