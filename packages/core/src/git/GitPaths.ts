import { dirname } from 'node:path';
import { GitRunner } from './GitRunner.js';
import { HarnessError } from '../errors/HarnessError.js';

// Ermittelt die für den Harness wichtigen Git-Pfade (immer mit Schrägstrichen).
export class GitPaths {
  constructor(private readonly git: GitRunner = new GitRunner()) {}

  // Gemeinsames Git-Verzeichnis, für Hauptordner und alle Worktrees identisch
  commonDir(cwd: string): string {
    try {
      const out = this.git.run(cwd, ['rev-parse', '--path-format=absolute', '--git-common-dir']);
      return this.normalize(out);
    } catch {
      throw new HarnessError(
        'not-a-git-project',
        'Hier ist kein Git-Projekt. `harness init` muss in einem Git-Repository laufen.',
      );
    }
  }

  // Wie commonDir, aber ohne den Prozess zu blockieren
  async commonDirAsync(cwd: string): Promise<string> {
    try {
      return this.normalize(await this.git.runAsync(cwd, ['rev-parse', '--path-format=absolute', '--git-common-dir']));
    } catch {
      throw new HarnessError(
        'not-a-git-project',
        'Hier ist kein Git-Projekt. `harness init` muss in einem Git-Repository laufen.',
      );
    }
  }

  // Wurzel des Hauptordners (nicht des Worktrees), abgeleitet aus dem gemeinsamen Verzeichnis
  mainRoot(cwd: string): string {
    return this.rootOf(this.commonDir(cwd));
  }

  // Wurzel des Hauptordners zu einem schon ermittelten gemeinsamen Verzeichnis (ohne Git-Aufruf)
  rootOf(commonDir: string): string {
    return this.normalize(dirname(commonDir));
  }

  // Wurzel des aktuellen Arbeitsordners (im Worktree dessen eigene Wurzel)
  topLevel(cwd: string): string {
    try {
      return this.normalize(this.git.run(cwd, ['rev-parse', '--show-toplevel']));
    } catch {
      throw new HarnessError('not-a-git-project', 'Hier ist kein Git-Projekt.');
    }
  }

  // Vereinheitlicht Pfade auf Schrägstriche, damit Windows und Linux gleich vergleichbar sind
  private normalize(path: string): string {
    return path.replace(/\\/g, '/');
  }
}
