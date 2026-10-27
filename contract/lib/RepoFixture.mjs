import { execFileSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';

// Ordnername mit Leerzeichen und Umlaut: Pfade, die auf allen drei Plattformen zu Fehlern führen, fallen früh auf
const REPO_DIR = 'Mein Projekt ä';

// Legt die temporären Git-Repos eines Szenarios an (Dateien, erster Commit, auf Wunsch `harness init`).
export class RepoFixture {
  // `home`: HARNESS_HOME des Szenarios, `claudeConfig`: leere Claude-Code-Konfiguration,
  // `cli`: Befehl der CLI als Liste (z. B. ['node', '…/cli/dist/main.js'])
  constructor(options) {
    this.home = options.home;
    this.claudeConfig = options.claudeConfig;
    this.cli = options.cli;
    this.base = mkdtempSync(join(tmpdir(), 'harness-contract-repos-'));
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
    for (const [file, content] of Object.entries(repo.files)) {
      const target = join(path, ...file.split('/'));
      mkdirSync(dirname(target), { recursive: true });
      writeFileSync(target, content);
    }
    if (repo.git) {
      this.git(path, ['init', '-q', '-b', 'main']);
      this.git(path, ['add', '-A']);
      this.git(path, ['-c', 'user.name=Vertrag', '-c', 'user.email=vertrag@example.invalid', 'commit', '--allow-empty', '-q', '-m', 'init']);
    }
    if (repo.harnessInit) this.harnessInit(path);
    return path;
  }

  // Führt Git im Repo aus
  git(cwd, args) {
    execFileSync('git', args, { cwd, encoding: 'utf8', windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'] });
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
        env: { ...process.env, HARNESS_HOME: this.home, CLAUDE_CONFIG_DIR: this.claudeConfig },
      });
    } catch (error) {
      const output = `${error.stdout ?? ''}${error.stderr ?? ''}`.trim();
      throw new Error(`\`${this.cli.join(' ')} init\` ist in ${cwd} fehlgeschlagen: ${output || error.message}`);
    }
  }

  // Räumt alle Repos auf
  dispose() {
    rmSync(this.base, { recursive: true, force: true });
  }
}
