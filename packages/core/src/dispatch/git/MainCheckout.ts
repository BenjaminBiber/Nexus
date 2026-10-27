import { existsSync } from 'node:fs';
import { isAbsolute, join } from 'node:path';
import { GitRunner } from '../../git/GitRunner.js';
import { PROJECT_RULE_PATHS } from '../../install/ProjectRules.js';

// Lokale Dateien des Harness (Plugin-Einstellung, Rules für Doku- und Board-Regeln); zählen nie als Änderung
const HARNESS_FILES: ReadonlySet<string> = new Set(['.claude/settings.local.json', ...PROJECT_RULE_PATHS]);
// Zeiger auf die aktuelle Spec aus der Spec-Kit-Zeit; bleibt in der Ausnahmeliste, damit alte Arbeitsordner
// keinen Merge blockieren – ungestaged kein Hindernis
const SPEC_POINTER = '.specify/feature.json';

// Zustände, in denen nicht gemergt werden darf (Datei im Git-Verzeichnis → laufende Aktion)
const BUSY: Array<[string, string]> = [
  ['MERGE_HEAD', 'ein Merge'],
  ['rebase-merge', 'ein Rebase'],
  ['rebase-apply', 'ein Rebase'],
  ['CHERRY_PICK_HEAD', 'ein Cherry-Pick'],
  ['REVERT_HEAD', 'ein Revert'],
];

export interface CheckoutState {
  /** Ausgecheckter Branch; null bei Detached HEAD */
  branch: string | null;
  /** Grund, warum gerade nicht gemergt werden darf, oder null */
  busy: string | null;
  /** Geänderte oder neue Dateien, die keine lokale Datei des Harness sind (sortiert) */
  blockingFiles: string[];
  /** Davon die im Index vorgemerkten (gestagten) Dateien – mit ihnen weist Git jeden Merge ab (sortiert) */
  stagedFiles: string[];
}

// Prüft den Hauptordner eines Repos, bevor der Dispatcher hineinmergt.
export class MainCheckout {
  constructor(private readonly git: GitRunner = new GitRunner()) {}

  // Branch, laufende Git-Aktion und störende lokale Änderungen
  inspect(root: string): CheckoutState {
    const symbolic = this.git.tryRun(root, ['symbolic-ref', '--quiet', '--short', 'HEAD']);
    const running = this.running(root);
    const blocking = this.blocking(root);
    return {
      branch: symbolic.ok ? symbolic.out : null,
      busy: running ? `Im Hauptordner läuft gerade ${running}.` : null,
      blockingFiles: blocking.map((e) => e.file),
      stagedFiles: blocking.filter((e) => e.staged).map((e) => e.file),
    };
  }

  // Läuft gerade ein Merge, Rebase, Cherry-Pick oder Revert? Liefert z. B. „ein Merge“ oder null
  running(root: string): string | null {
    for (const [name, what] of BUSY) {
      const path = this.git.run(root, ['rev-parse', '--git-path', name]);
      if (existsSync(isAbsolute(path) ? path : join(root, path))) return what;
    }
    return null;
  }

  // Alle lokal geänderten oder neuen Dateien des Ordners (ohne Ausnahmen); bei Umbenennung/Kopie beide Pfade
  changed(root: string): string[] {
    return [...new Set(this.status(root).flatMap((e) => e.paths))];
  }

  // Lokal geänderte oder neue Dateien aus `git status` außer denen des Harness; der Spec-Zeiger zählt nur gestagt.
  // Bei Umbenennung/Kopie zählen beide Pfade.
  private blocking(root: string): Array<{ file: string; staged: boolean }> {
    const files: Array<{ file: string; staged: boolean }> = [];
    for (const { xy, paths } of this.status(root)) {
      const unstagedOnly = xy[0] === ' ' || xy === '??';
      const allowed = (f: string) => HARNESS_FILES.has(f) || (unstagedOnly && f === SPEC_POINTER);
      if (!paths.every(allowed)) files.push({ file: paths[0], staged: !unstagedOnly });
    }
    return files.sort((a, b) => a.file.localeCompare(b.file));
  }

  // Einträge aus `git status --porcelain=v1 -z`: Statusspalten und Pfade (bei Umbenennung/Kopie neuer, dann alter Pfad)
  private status(root: string): Array<{ xy: string; paths: string[] }> {
    const out = this.git.raw(root, ['-c', 'core.quotePath=false', 'status', '--porcelain=v1', '-z', '--untracked-files=all']);
    const entries = out.split('\0');
    const result: Array<{ xy: string; paths: string[] }> = [];
    for (let i = 0; i < entries.length; i += 1) {
      const entry = entries[i];
      if (entry.length < 4) continue;
      const xy = entry.slice(0, 2);
      const paths = [entry.slice(3)];
      // Umbenennung/Kopie: der nächste Eintrag ist der alte Pfad
      if ('RC'.includes(xy[0]) || 'RC'.includes(xy[1])) {
        i += 1;
        if (entries[i]) paths.push(entries[i]);
      }
      result.push({ xy, paths });
    }
    return result;
  }
}
