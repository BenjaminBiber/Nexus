import { execFileSync } from 'node:child_process';
import { existsSync, mkdirSync, rmSync, writeFileSync } from 'node:fs';
import { dirname, join, relative, resolve } from 'node:path';
import { GIT_IDENTITY_ENV } from './GitIdentity.mjs';

// Schritt-Art `commit`: schreibt und löscht Dateien in einem Ordner der Fixture-Repos und committet sie. Das ist die
// Arbeit, die im Vertragsmodus sonst niemand erledigt – es startet nie ein Worker, der Branch eines gestarteten Tickets
// bliebe also leer. Erst damit haben Review (Dateiliste, Patch) und Merge (Merge-Commit, Konflikt) echte Änderungen.
// Identität und Datum des Commits sind fest (GIT_IDENTITY_ENV), die Commit-Kennung also reproduzierbar.
export class CommitStep {
  // Eigene Felder der Art; der Lader meldet damit Tippfehler im Szenario
  get keys() {
    return ['cwd', 'files', 'remove', 'message'];
  }

  // Führt den Schritt aus; `context.reposDir` ist der Ordner der Fixture-Repos (nur dort wird geschrieben)
  async run(step, context) {
    const { cwd, files = {}, remove = [], message } = step.raw;
    if (typeof cwd !== 'string' || !cwd) throw new Error('cwd muss der Ordner sein, in dem committet wird (z. B. "{{REPO1}}").');
    if (files === null || typeof files !== 'object' || Array.isArray(files)) throw new Error('files muss die neuen Dateien als Pfad → Text nennen.');
    if (!Array.isArray(remove) || remove.some((path) => typeof path !== 'string' || !path)) throw new Error('remove muss eine Liste von Pfaden sein.');
    if (!Object.keys(files).length && !remove.length) throw new Error('Ein Commit braucht files oder remove.');
    if (typeof message !== 'string' || !message.trim()) throw new Error('message muss die Commit-Nachricht als Text sein.');
    const dir = resolve(context.variables.apply(cwd));
    this.assertInRepos(dir, context.reposDir);
    const written = this.write(dir, context.variables.apply(files));
    const removed = this.remove(dir, context.variables.apply(remove));
    // Nur die genannten Pfade aufnehmen: ein Worktree enthält auch Dateien, die der Harness beim Anlegen hineinschreibt
    // (Plugin-Einstellung, Rules) – die gehören nicht in den Commit des Agenten
    this.git(dir, ['add', '-A', '--', ...written, ...removed]);
    this.git(dir, ['commit', '-q', '-m', message]);
    return {
      request: { kind: 'commit', cwd: dir, files: written, ...(removed.length ? { remove: removed } : {}), message },
      response: { json: { branch: this.git(dir, ['rev-parse', '--abbrev-ref', 'HEAD']), commit: this.git(dir, ['rev-parse', '--short', 'HEAD']) } },
    };
  }

  // Schreibt die Dateien des Commits (Pfade mit `/`) und liefert sie zurück
  write(dir, files) {
    const written = [];
    for (const [path, content] of Object.entries(files)) {
      if (typeof content !== 'string') throw new Error(`files["${path}"] muss ein Text sein.`);
      const target = join(dir, ...path.split('/'));
      mkdirSync(dirname(target), { recursive: true });
      writeFileSync(target, content);
      written.push(path);
    }
    return written;
  }

  // Löscht die genannten Dateien; eine Datei, die es nicht gibt, ist ein Fehler im Szenario
  remove(dir, paths) {
    for (const path of paths) {
      const target = join(dir, ...path.split('/'));
      if (!existsSync(target)) throw new Error(`remove nennt ${path}; die Datei gibt es im Ordner nicht.`);
      rmSync(target, { force: true });
    }
    return [...paths];
  }

  // Nur innerhalb der Fixture-Repos committen; ein verschriebener Pfad soll nicht im echten Repo landen
  assertInRepos(dir, reposDir) {
    const inside = reposDir && !relative(reposDir, dir).startsWith('..');
    if (!inside) throw new Error(`cwd ${dir} liegt nicht in den Fixture-Repos – committet wird nur dort.`);
    if (!existsSync(dir)) throw new Error(`Den Ordner ${dir} gibt es nicht (Worktree schon angelegt?).`);
  }

  // Führt Git im Ordner aus und liefert die getrimmte Ausgabe
  git(cwd, args) {
    return execFileSync('git', args, {
      cwd,
      encoding: 'utf8',
      windowsHide: true,
      stdio: ['ignore', 'pipe', 'pipe'],
      env: { ...process.env, ...GIT_IDENTITY_ENV },
    }).trim();
  }
}
