import { existsSync } from 'node:fs';
import { isAbsolute, join } from 'node:path';
import { HarnessError } from '../../errors/HarnessError.js';
import { GitRunner, type GitAttempt } from '../../git/GitRunner.js';
import type { MergeStrategy } from '../DispatchConfig.js';
import { MainCheckout } from './MainCheckout.js';

/** Trailer eines Squash-Commits: `Harness-Squashed: <branch> <volle Spitze des Branches>` */
export const SQUASH_TRAILER = 'Harness-Squashed';

// Höchstens so viele Commits listet ein Squash-Commit auf
const MAX_SQUASH_LIST = 50;

export type MergeOutcome =
  | { kind: 'merged'; commit: string }
  | { kind: 'conflict'; files: string[] }
  | { kind: 'refused'; message: string };

// Mergt einen Ticket-Branch in den ausgecheckten Branch des Hauptordners; bei Problemen bleibt der Ordner wie vorher.
// Fremde Git-Zustände (laufender Merge, Rebase, Cherry-Pick, Revert) werden nie angefasst.
export class BranchMerger {
  private readonly checkout: MainCheckout;

  constructor(private readonly git: GitRunner = new GitRunner()) {
    this.checkout = new MainCheckout(git);
  }

  // Strategie „merge“: `git merge --no-ff --no-edit [-m <message>] refs/heads/<branch>` (ohne Nachricht die
  // Git-Standardnachricht). Strategie „squash“: `git merge --squash refs/heads/<branch>` und ein Commit mit der Nachricht,
  // der Liste der zusammengefassten Commits und dem Trailer `Harness-Squashed: <branch> <Spitze>` (auch leer, damit
  // `integrated` den Branch erkennt). Ist der Branch schon enthalten oder schon zusammengefasst → kein neuer Commit.
  // Läuft schon eine Git-Aktion → Abweisung ohne Eingriff, Konflikt → Dateien und abort (nur für den eigenen Merge),
  // sonstige Abweisung → Git-Meldung
  merge(root: string, branch: string, message?: string, strategy: MergeStrategy = 'merge'): MergeOutcome {
    const running = this.checkout.running(root);
    if (running) return { kind: 'refused', message: `Im Hauptordner läuft bereits ${running}; es wird nicht gemergt.` };
    if (this.squashedInto(root, branch, 'HEAD')) return this.merged(root);
    if (strategy === 'squash') return this.squash(root, branch, message);
    const text = message ? ['-m', message] : [];
    const attempt = this.git.tryRun(root, ['-c', 'core.quotePath=false', 'merge', '--no-ff', '--no-edit', ...text, `refs/heads/${branch}`]);
    if (attempt.ok) return this.merged(root);
    // Vorher lief keine Aktion: ein MERGE_HEAD jetzt stammt von diesem Aufruf
    const started = this.git.tryRun(root, ['rev-parse', '--verify', '--quiet', 'MERGE_HEAD']).ok;
    return this.afterFailure(root, attempt, started);
  }

  // Ist der Branch im Ziel angekommen – als Vorfahre (Merge) oder per Squash-Commit mit Trailer für seine aktuelle Spitze?
  // Ein Branch, den es nicht gibt, gilt als nicht angekommen.
  integrated(root: string, branch: string, target = 'HEAD'): boolean {
    if (!this.git.tryRun(root, ['rev-parse', '--verify', '--quiet', `refs/heads/${branch}^{commit}`]).ok) return false;
    return this.contains(root, `refs/heads/${branch}`, target) || this.squashedInto(root, branch, target);
  }

  // Den Branch gibt es nicht mehr, aber `target` enthält schon seinen Merge – z. B. nach einem Abbruch zwischen Merge und
  // Aufräumen. Als Merge zählt nur ein Merge-Commit (zwei Eltern) mit Betreff „… (<branch>)“ oder ein Commit mit dem
  // Squash-Trailer für den Branch; ein Revert oder ein Commit, der den Branch nur erwähnt, zählt nicht.
  // Liefert die Kurzkennung dieses Commits, sonst null
  mergedEarlier(root: string, branch: string, target = 'HEAD'): string | null {
    if (this.git.tryRun(root, ['rev-parse', '--verify', '--quiet', `refs/heads/${branch}^{commit}`]).ok) return null;
    const patterns = [`(${branch})`, `${SQUASH_TRAILER}: ${branch} `].map((p) => `--grep=${p}`);
    // Je Kandidat: Kurzkennung, Eltern, Betreff und die Werte des Squash-Trailers (Felder mit \x1f, Commits mit \x1e getrennt)
    const format = `--format=%h%x1f%p%x1f%s%x1f%(trailers:key=${SQUASH_TRAILER},valueonly)%x1e`;
    const found = this.git.tryRun(root, ['log', format, '--fixed-strings', ...patterns, target]);
    if (!found.ok) return null;
    for (const record of found.out.split('\x1e')) {
      const [hash, parents, subject, trailers] = record.replace(/^\s+/, '').split('\x1f');
      if (!hash) continue;
      const isMerge = (parents ?? '').trim().split(/\s+/).length >= 2 && (subject ?? '').endsWith(`(${branch})`);
      const isSquash = (trailers ?? '').split(/\r?\n/).some((value) => value.trim().startsWith(`${branch} `));
      if (isMerge || isSquash) return hash;
    }
    return null;
  }

  // Trägt ein Commit in `target` den Trailer `Harness-Squashed: <branch> <aktuelle Spitze des Branches>`?
  squashedInto(root: string, branch: string, target = 'HEAD'): boolean {
    const tip = this.git.tryRun(root, ['rev-parse', '--verify', '--quiet', `refs/heads/${branch}^{commit}`]);
    if (!tip.ok || !tip.out) return false;
    const found = this.git.tryRun(root, ['log', '-1', '--format=%H', '--fixed-strings', `--grep=${SQUASH_TRAILER}: ${branch} ${tip.out}`, target]);
    return found.ok && found.out !== '';
  }

  // Squash: ist der Branch schon Vorfahre, gibt es nichts zu tun; sonst `merge --squash` und ein eigener Commit.
  // Bei --squash gibt es kein MERGE_HEAD: begonnen hat der Aufruf, wenn danach SQUASH_MSG oder Konfliktdateien da sind.
  private squash(root: string, branch: string, message?: string): MergeOutcome {
    const ref = `refs/heads/${branch}`;
    if (this.contains(root, ref, 'HEAD')) return this.merged(root);
    // Vorgemerkte Änderungen würde ein Squash (beim Vorspulen prüft Git den Index nicht) mit in den Commit nehmen
    if (!this.git.tryRun(root, ['diff', '--cached', '--quiet']).ok) {
      return { kind: 'refused', message: 'Im Hauptordner sind Änderungen vorgemerkt (git add); ein Squash würde sie mit übernehmen. Bitte committen oder aus dem Index nehmen.' };
    }
    // --no-verify wie beim Merge-Commit von `git merge`, der keinen pre-commit-Hook ausführt
    const commit = ['commit', '-q', '--allow-empty', '--no-verify', ...this.squashMessage(root, branch, message)];
    const pendingBefore = this.squashPending(root);
    const attempt = this.git.tryRun(root, ['-c', 'core.quotePath=false', 'merge', '--squash', ref]);
    if (attempt.ok) {
      const done = this.git.tryRun(root, commit);
      if (done.ok) return this.merged(root);
      this.abort(root, 'squash');
      return { kind: 'refused', message: `Der Squash-Commit scheiterte: ${[done.err, done.out].filter(Boolean).join('\n')}` };
    }
    const unmerged = this.git.tryRun(root, ['ls-files', '-u']);
    const started = (!pendingBefore && this.squashPending(root)) || (unmerged.ok && unmerged.out !== '');
    return this.afterFailure(root, attempt, started, 'squash');
  }

  // Gescheiterter Merge oder Squash: Konfliktdateien ermitteln, den eigenen Versuch zurücksetzen und
  // Konflikt bzw. Abweisung melden
  private afterFailure(root: string, attempt: GitAttempt, started: boolean, strategy: MergeStrategy = 'merge'): MergeOutcome {
    let conflicted: string[] = [];
    try {
      const listed = this.git.tryRun(root, ['-c', 'core.quotePath=false', 'diff', '--name-only', '--diff-filter=U']);
      if (listed.ok) conflicted = listed.out.split('\n').map((l) => l.trim()).filter(Boolean);
    } finally {
      if (started) this.abort(root, strategy);
    }
    if (started || conflicted.length) return { kind: 'conflict', files: conflicted };
    return { kind: 'refused', message: [attempt.err, attempt.out].filter(Boolean).join('\n') || 'Git hat den Merge abgelehnt.' };
  }

  // Nachricht des Squash-Commits als `-m`-Absätze: Betreff, Liste der zusammengefassten Commits (älteste zuerst,
  // höchstens MAX_SQUASH_LIST), Trailer mit der vollen Spitze des Branches
  private squashMessage(root: string, branch: string, message?: string): string[] {
    const ref = `refs/heads/${branch}`;
    const tip = this.git.run(root, ['rev-parse', `${ref}^{commit}`]);
    const subjects = this.git.run(root, ['log', '--reverse', '--format=- %s', `HEAD..${ref}`]).split('\n').filter(Boolean);
    const shown = subjects.slice(0, MAX_SQUASH_LIST);
    if (subjects.length > shown.length) shown.push(`(… und ${subjects.length - shown.length} weitere Commits)`);
    const paragraphs = [message || `Squash ${branch}`, ...(shown.length ? [shown.join('\n')] : []), `${SQUASH_TRAILER}: ${branch} ${tip}`];
    return paragraphs.flatMap((p) => ['-m', p]);
  }

  // Liegt eine SQUASH_MSG im Git-Verzeichnis (ein begonnener, nicht abgeschlossener Squash)?
  private squashPending(root: string): boolean {
    const path = this.git.tryRun(root, ['rev-parse', '--git-path', 'SQUASH_MSG']);
    return path.ok && existsSync(isAbsolute(path.out) ? path.out : join(root, path.out));
  }

  // Ergebnis „gemergt“ mit der Kurzkennung von HEAD
  private merged(root: string): MergeOutcome {
    return { kind: 'merged', commit: this.git.run(root, ['rev-parse', '--short', 'HEAD']) };
  }

  // Dateien, die der Branch seit dem gemeinsamen Vorgänger mit HEAD ändert und die im Hauptordner lokal geändert
  // oder neu sind (sprachunabhängig, ohne Ausnahmen, sortiert) – die Ursache, wenn Git den Merge abweist
  overlapping(root: string, branch: string): string[] {
    const local = new Set(this.checkout.changed(root));
    const diff = this.git.raw(root, ['diff', '--name-only', '-z', '--no-renames', `HEAD...refs/heads/${branch}`]);
    const files = diff.split('\0').filter((f) => f && local.has(f));
    return [...new Set(files)].sort((a, b) => a.localeCompare(b));
  }

  // Commit-Kennung einer Referenz
  head(root: string, ref: string): string {
    return this.git.run(root, ['rev-parse', ref]);
  }

  // Ist `ancestor` in `branch` enthalten?
  contains(root: string, ancestor: string, branch: string): boolean {
    return this.git.tryRun(root, ['merge-base', '--is-ancestor', ancestor, branch]).ok;
  }

  // Löscht einen gemergten Branch mit `-d`. Mit `squashedTip` (per Squash zusammengefasst, Git hält ihn für ungemergt):
  // nur, wenn er noch genau auf dieser Spitze steht und in keinem Worktree ausgecheckt ist (`update-ref -d` mit altem Wert,
  // damit ein Commit nach der Prüfung nicht verloren geht). Liefert die Git-Meldung, wenn es nicht geht
  deleteBranch(root: string, branch: string, squashedTip?: string): string | null {
    if (!squashedTip) {
      const attempt = this.git.tryRun(root, ['branch', '-d', branch]);
      return attempt.ok ? null : `Branch ${branch} nicht gelöscht: ${attempt.err}`;
    }
    const ref = `refs/heads/${branch}`;
    const checkedOut = this.git.tryRun(root, ['for-each-ref', '--format=%(worktreepath)', ref]);
    if (checkedOut.ok && checkedOut.out.trim()) return `Branch ${branch} nicht gelöscht: er ist in ${checkedOut.out.trim()} ausgecheckt.`;
    const attempt = this.git.tryRun(root, ['update-ref', '-d', ref, squashedTip]);
    if (!attempt.ok) return `Branch ${branch} nicht gelöscht: ${attempt.err || 'er hat inzwischen neue Commits.'}`;
    this.git.tryRun(root, ['config', '--remove-section', `branch.${branch}`]);
    return null;
  }

  // Bricht den eigenen Merge ab; scheitert das, steht der Ordner noch im Merge-Zustand → Fehler statt Konfliktmeldung.
  // Ein Squash hat kein MERGE_HEAD: `git reset --merge` setzt Index und vom Squash geänderte Dateien zurück, lässt aber
  // nicht gestagte lokale Änderungen stehen (wie `merge --abort`) und entfernt SQUASH_MSG.
  private abort(root: string, strategy: MergeStrategy = 'merge'): void {
    const command = strategy === 'squash' ? ['reset', '-q', '--merge'] : ['merge', '--abort'];
    const aborted = this.git.tryRun(root, command);
    if (aborted.ok) return;
    const git = aborted.err || aborted.out;
    const manual = `git ${command.filter((a) => a !== '-q').join(' ')}`;
    // details: Ordner, Git-Meldung und Befehl, damit ein Aufrufer außerhalb des Hauptordners den richtigen Ordner nennt
    throw new HarnessError(
      'merge-abort-failed',
      `Der Hauptordner steht noch im Merge-Zustand: ${git}. Bitte von Hand aufräumen (${manual}).`,
      { root, git, command: manual },
    );
  }
}
