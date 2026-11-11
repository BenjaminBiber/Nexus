import { execFileSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, realpathSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { GIT_IDENTITY_ENV } from './GitIdentity.mjs';

// Ordnername mit Leerzeichen und Umlaut: Pfade, die auf allen drei Plattformen zu Fehlern führen, fallen früh auf
const REPO_DIR = 'Mein Projekt ä';

// Hauptbranch der Fixture-Repos
const MAIN = 'main';

// Legt die temporären Git-Repos eines Szenarios an (Dateien, erster Commit, Arbeitsbranches, auf Wunsch `nexus init`).
export class RepoFixture {
  // `home`: HARNESS_HOME des Szenarios, `claudeConfig`: leere Claude-Code-Konfiguration,
  // `cli`: Befehl der CLI als Liste (z. B. ['node', '…/cli/dist/main.js'])
  constructor(options) {
    this.home = options.home;
    this.claudeConfig = options.claudeConfig;
    this.cli = options.cli;
    // Aufgelöster Pfad: Git und die CLI melden die Projektwurzel ohne Verknüpfungen (unter macOS /private/var
    // statt /var) und unter Windows mit langen Namen statt 8.3-Kurznamen wie RUNNER~1; die löst nur
    // `realpathSync.native` auf. Nur so ist `{{REPO1}}` derselbe Text, den der Server als Projektwurzel führt –
    // sonst fände ein Board, dessen Repo-Pfad auf `{{REPO1}}` zeigt, sein Projekt nicht und die Golden-Dateien
    // wären je Plattform verschieden.
    this.base = realpathSync.native(mkdtempSync(join(tmpdir(), 'harness-contract-repos-')));
  }

  // Legt alle Repos an und liefert Name → Pfad
  create(repos) {
    const paths = {};
    for (const repo of repos) paths[repo.name] = this.one(repo);
    return paths;
  }

  // Legt ein Repo an und liefert seinen Pfad
  one(repo) {
    const path = join(this.base, repo.name, REPO_DIR);
    mkdirSync(path, { recursive: true });
    this.write(path, repo.files);
    if (repo.git) {
      this.git(path, ['init', '-q', '-b', MAIN]);
      this.commit(path, 'init');
      for (const branch of repo.branches ?? []) this.branch(path, branch);
    }
    if (repo.harnessInit) this.harnessInit(path);
    // Aufgelöster Pfad: Git meldet die Projektwurzel ohne Verknüpfungen (unter macOS `/private/var/…` statt
    // `/var/…`, unter Windows mit langen Namen). Nur so passt `{{REPO1}}` zu dem Pfad, den der Server selbst
    // führt – etwa beim Repo-Pfad eines Boards. Der Normalisierer kennt alle Schreibweisen und schreibt in
    // jedem Fall `<REPO1>`.
    return realpathSync.native(path);
  }

  // Legt einen Arbeitsbranch mit einem Commit an und wechselt zurück auf den Hauptbranch: der Branch steht damit vor
  // dem Hauptbranch, wie nach der Arbeit eines Agenten
  branch(cwd, branch) {
    this.git(cwd, ['checkout', '-q', '-b', branch.name]);
    this.write(cwd, branch.files);
    this.commit(cwd, branch.message);
    this.git(cwd, ['checkout', '-q', MAIN]);
  }

  // Schreibt die Dateien eines Repos oder eines Branches (Pfade mit `/`)
  write(cwd, files) {
    for (const [file, content] of Object.entries(files ?? {})) {
      const target = join(cwd, ...file.split('/'));
      mkdirSync(dirname(target), { recursive: true });
      writeFileSync(target, content);
    }
  }

  // Nimmt alles auf und committet es
  commit(cwd, message) {
    this.git(cwd, ['add', '-A']);
    this.git(cwd, ['commit', '--allow-empty', '-q', '-m', message]);
  }

  // Führt Git im Repo aus; die feste Identität und das feste Datum aus GIT_IDENTITY_ENV machen die Commits reproduzierbar
  git(cwd, args) {
    execFileSync('git', args, {
      cwd,
      encoding: 'utf8',
      windowsHide: true,
      stdio: ['ignore', 'pipe', 'pipe'],
      env: { ...process.env, ...GIT_IDENTITY_ENV },
    });
  }

  // Richtet den Harness im Repo ein (`<cli> init`) – mit dem Home des Szenarios und leerer Claude-Konfiguration
  harnessInit(cwd) {
    const [command, ...args] = this.cli;
    try {
      execFileSync(command, [...args, 'init'], {
        cwd,
        encoding: 'utf8',
        windowsHide: true,
        stdio: ['ignore', 'pipe', 'pipe'],
        env: { ...process.env, ...GIT_IDENTITY_ENV, HARNESS_HOME: this.home, CLAUDE_CONFIG_DIR: this.claudeConfig },
      });
    } catch (error) {
      const output = `${error.stdout ?? ''}${error.stderr ?? ''}`.trim();
      throw new Error(`\`${this.cli.join(' ')} init\` ist in ${cwd} fehlgeschlagen: ${output || error.message}`);
    }
  }

  // Räumt alle Repos auf. Unter Windows hält oft noch ein anderes Programm (Virenscanner, Indexer) kurz eine
  // Datei offen; das Löschen scheitert dann mit EPERM und gelingt bei einem späteren Versuch.
  dispose() {
    rmSync(this.base, { recursive: true, force: true, maxRetries: 10, retryDelay: 100 });
  }
}
