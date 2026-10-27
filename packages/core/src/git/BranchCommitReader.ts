import { existsSync } from 'node:fs';
import { GitRunner } from './GitRunner.js';

export interface BranchCommit {
  hash: string;
  author: string;
  /** Autorzeit als ISO (UTC) */
  at: string;
  subject: string;
}

export interface BranchCommits {
  /** unavailable = kein Repo, Branch und Merge-Commit nicht (mehr) da; error = Git scheiterte unerwartet */
  status: 'ok' | 'unavailable' | 'error';
  commits: BranchCommit[];
  message?: string;
}

const HASH = /^[0-9a-f]{4,40}$/i;
const SEP = '\x1f';

// Liest die Commits eines Ticket-Branches: vor dem Merge Zielbranch..Branch (ohne Zielbranch HEAD..Branch), nach dem Merge über den Merge-Commit
// (Merge-Commit: zweiter Elternteil ohne ersten; Squash: nur der Commit selbst). Wirft nie.
export class BranchCommitReader {
  constructor(private readonly limit = 50, private readonly git = new GitRunner()) {}

  read(repoPath: string | null, source: { branch: string | null; mergeCommit: string | null; targetBranch?: string | null }): BranchCommits {
    if (!repoPath || !existsSync(repoPath)) return this.unavailable('Kein Repo-Pfad auf diesem Rechner.');
    if (!this.git.tryRun(repoPath, ['rev-parse', '--git-dir']).ok) return this.unavailable('Der Repo-Pfad ist kein Git-Repo.');
    const range = this.range(repoPath, source);
    if (!range) return this.unavailable('Branch und Merge-Commit sind in diesem Repo nicht (mehr) vorhanden.');
    const log = this.git.tryRun(repoPath, ['log', '--no-merges', `-n${this.limit}`, `--format=%H${SEP}%an${SEP}%aI${SEP}%s`, ...range, '--']);
    if (!log.ok) return { status: 'error', commits: [], message: log.err };
    return { status: 'ok', commits: log.out ? log.out.split('\n').map((line) => this.commit(line)) : [] };
  }

  // Bereich für git log; null, wenn weder Merge-Commit noch Branch auflösbar ist
  private range(repoPath: string, source: { branch: string | null; mergeCommit: string | null; targetBranch?: string | null }): string[] | null {
    const merge = source.mergeCommit && HASH.test(source.mergeCommit) ? this.resolve(repoPath, `${source.mergeCommit}^{commit}`) : null;
    if (merge) {
      const parents = this.git.tryRun(repoPath, ['rev-list', '--parents', '-n1', merge]).out.split(' ').length - 1;
      return parents >= 2 ? [`${merge}^1..${merge}^2`] : ['-n1', merge];
    }
    if (!source.branch || source.branch.startsWith('-')) return null;
    const branch = this.resolve(repoPath, `refs/heads/${source.branch}`);
    return branch ? [`${this.base(repoPath, source.targetBranch)}..${branch}`] : null;
  }

  // Beginn des Bereichs: lokaler Zielbranch des Tickets, wenn auflösbar, sonst HEAD
  private base(repoPath: string, target?: string | null): string {
    if (!target || target.startsWith('-')) return 'HEAD';
    return this.resolve(repoPath, `refs/heads/${target}`) ?? 'HEAD';
  }

  // Volle Commit-ID oder null
  private resolve(repoPath: string, ref: string): string | null {
    const r = this.git.tryRun(repoPath, ['rev-parse', '--verify', '--quiet', ref]);
    return r.ok && r.out ? r.out : null;
  }

  private commit(line: string): BranchCommit {
    const [hash, author, at, ...subject] = line.split(SEP);
    const date = new Date(at);
    return { hash, author, at: Number.isNaN(date.getTime()) ? at : date.toISOString(), subject: subject.join(SEP) };
  }

  private unavailable(message: string): BranchCommits {
    return { status: 'unavailable', commits: [], message };
  }
}
