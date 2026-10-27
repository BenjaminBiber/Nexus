import { HarnessError } from '../../errors/HarnessError.js';
import { GitRunner } from '../../git/GitRunner.js';

/** Eine geänderte Datei des Ticket-Branches */
export interface ReviewFile {
  path: string;
  /** Bisheriger Pfad bei Umbenennung oder Kopie */
  oldPath?: string;
  /** A, M, D, R, C, T … (erster Buchstabe von --name-status) */
  status: string;
  additions: number;
  deletions: number;
  binary: boolean;
}

/** Dateiliste des Ticket-Branches gegen den Zielbranch des Tickets, sonst gegen den ausgecheckten Branch des Hauptordners */
export interface ReviewFileList {
  /** Zielbranch des Tickets, sonst ausgecheckter Branch des Hauptordners; „HEAD“ bei Detached HEAD */
  target: string;
  /** Merge-Basis (Commit) */
  base: string;
  files: ReviewFile[];
}

/** Patch einer Datei, notfalls gekürzt */
export interface ReviewPatch {
  patch: string;
  truncated: boolean;
}

/** Größter gezeigter Patch je Datei in Zeichen */
const MAX_PATCH = 200_000;

// Diff eines Ticket-Branches gegen das Ziel des Merges (Three-Dot: ab der Merge-Basis, fremde Änderungen am Ziel fehlen).
// Ziel ist der Zielbranch des Tickets, wenn er lokal existiert, sonst der ausgecheckte Branch des Hauptordners (HEAD).
export class ReviewDiff {
  constructor(private readonly git: GitRunner = new GitRunner()) {}

  // Ziel, Merge-Basis und geänderte Dateien mit Zeilenzahlen
  files(root: string, branch: string, target?: string | null): ReviewFileList {
    this.checkBranch(branch);
    const ref = this.baseRef(root, target);
    const symbolic = this.git.tryRun(root, ['symbolic-ref', '--quiet', '--short', 'HEAD']);
    const base = this.git.run(root, ['merge-base', ref.ref, branch]);
    const range = `${ref.ref}...${branch}`;
    const stats = this.numstat(this.git.raw(root, ['diff', '-M', '--numstat', '-z', range]));
    const files = this.nameStatus(this.git.raw(root, ['diff', '-M', '--name-status', '-z', range])).map((f) => {
      const s = stats.get(f.path);
      return { ...f, additions: s?.additions ?? 0, deletions: s?.deletions ?? 0, binary: s?.binary ?? false };
    });
    return { target: ref.name ?? (symbolic.ok ? symbolic.out : 'HEAD'), base, files };
  }

  // Patch einer Datei aus der Dateiliste; andere Pfade → HarnessError „review-file-unknown“
  file(root: string, branch: string, path: string, target?: string | null): ReviewPatch {
    const entry = this.files(root, branch, target).files.find((f) => f.path === path);
    if (!entry) throw new HarnessError('review-file-unknown', `Die Datei „${path}“ gehört nicht zu den Änderungen des Branches ${branch}.`);
    const paths = entry.oldPath ? [entry.oldPath, entry.path] : [entry.path];
    const patch = this.git.raw(root, ['diff', '-M', `${this.baseRef(root, target).ref}...${branch}`, '--', ...paths]);
    return patch.length > MAX_PATCH ? { patch: patch.slice(0, MAX_PATCH), truncated: true } : { patch, truncated: false };
  }

  // Branch-Namen, die Git als Option lesen würde, sind ausgeschlossen
  private checkBranch(branch: string): void {
    if (!branch || branch.startsWith('-')) throw new HarnessError('review-branch-invalid', `Ungültiger Branch „${branch}“.`);
  }

  // Basis des Vergleichs: lokaler Zielbranch, wenn auflösbar, sonst HEAD (z. B. Zielbranch noch nicht angelegt)
  private baseRef(root: string, target?: string | null): { ref: string; name: string | null } {
    if (!target) return { ref: 'HEAD', name: null };
    this.checkBranch(target);
    const ref = `refs/heads/${target}`;
    return this.git.tryRun(root, ['rev-parse', '--verify', '--quiet', ref]).ok ? { ref, name: target } : { ref: 'HEAD', name: null };
  }

  // Ausgabe von --name-status -z: Status, Pfad; bei R/C Status, alter Pfad, neuer Pfad
  private nameStatus(out: string): Array<Pick<ReviewFile, 'path' | 'oldPath' | 'status'>> {
    const parts = out.split('\0').filter((p, i, all) => p !== '' || i < all.length - 1);
    const list: Array<Pick<ReviewFile, 'path' | 'oldPath' | 'status'>> = [];
    for (let i = 0; i < parts.length; i++) {
      const status = parts[i].charAt(0);
      if (!status) continue;
      if (status === 'R' || status === 'C') {
        list.push({ status, oldPath: parts[i + 1], path: parts[i + 2] });
        i += 2;
      } else {
        list.push({ status, path: parts[i + 1] });
        i += 1;
      }
    }
    return list;
  }

  // Ausgabe von --numstat -z: „plus\tminus\tpfad“ oder bei Umbenennung „plus\tminus\t“, alter, neuer Pfad; „-“ = binär
  private numstat(out: string): Map<string, { additions: number; deletions: number; binary: boolean }> {
    const parts = out.split('\0');
    const map = new Map<string, { additions: number; deletions: number; binary: boolean }>();
    for (let i = 0; i < parts.length; i++) {
      const m = /^(-|\d+)\t(-|\d+)\t(.*)$/s.exec(parts[i]);
      if (!m) continue;
      let path = m[3];
      if (path === '') {
        path = parts[i + 2] ?? '';
        i += 2;
      }
      const binary = m[1] === '-' && m[2] === '-';
      map.set(path, { additions: binary ? 0 : Number(m[1]), deletions: binary ? 0 : Number(m[2]), binary });
    }
    return map;
  }
}
