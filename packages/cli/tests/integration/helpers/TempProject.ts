import { mkdtempSync, mkdirSync, rmSync, realpathSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { execFileSync, spawnSync } from 'node:child_process';
import { pathToFileURL } from 'node:url';

const ROOT = resolve(__dirname, '../../../../..');
const MAIN = join(ROOT, 'packages/cli/src/main.ts');
const DEV_TSCONFIG = join(ROOT, 'tsconfig.dev.json');
// Absoluter Pfad zum tsx-Lader, damit er auch aus fremden Arbeitsordnern gefunden wird
const TSX_LOADER = pathToFileURL(join(ROOT, 'node_modules/tsx/dist/esm/index.mjs')).href;

export interface RunResult {
  code: number;
  stdout: string;
  stderr: string;
}

// Temporäres Git-Projekt mit erstem Commit; führt das CLI per tsx im Projekt aus.
export class TempProject {
  readonly base: string;
  readonly path: string;

  // Legt Ordner (mit Leerzeichen und Umlaut) und Git-Projekt an
  constructor(withCommit = true) {
    // realpathSync.native löst auch 8.3-Kurznamen auf (TEMP steht unter Windows oft als C:\Users\BENJAM~1.BIB\…);
    // git meldet den Langpfad, sonst passt die Repo-Zuordnung des Boards nicht zum Ordner.
    this.base = realpathSync.native(mkdtempSync(join(tmpdir(), 'harness-cli-')));
    this.path = join(this.base, 'Demo Projekt ä');
    mkdirSync(this.path);
    this.git('init', '-q');
    if (withCommit) this.git('-c', 'user.name=t', '-c', 'user.email=t@t', 'commit', '--allow-empty', '-q', '-m', 'init');
  }

  // Führt Git im Projekt aus
  git(...args: string[]): string {
    return execFileSync('git', args, { cwd: this.path, encoding: 'utf8', windowsHide: true }).trim();
  }

  // Führt `harness <args>` aus; `cwd` erlaubt den Aufruf aus einem Worktree
  run(args: string[], cwd: string = this.path, env: Record<string, string> = {}, input?: string): RunResult {
    // HARNESS_SESSION und HARNESS_TICKET werden nicht geerbt, sonst gilt der Testlauf innerhalb einer Session als „session“
    // bzw. als einem Ticket zugeordnet
    const { HARNESS_SESSION: _session, HARNESS_TICKET: _ticket, ...inherited } = process.env;
    const result = spawnSync(process.execPath, ['--import', TSX_LOADER, MAIN, ...args], {
      cwd,
      encoding: 'utf8',
      // Eigenes HARNESS_HOME je Testprojekt, damit kein Test die echte board.yaml (und damit AiHarness) erreicht
      env: { ...inherited, HARNESS_HOME: join(this.base, 'home'), TSX_TSCONFIG_PATH: DEV_TSCONFIG, ...env },
      input,
      windowsHide: true,
    });
    return { code: result.status ?? -1, stdout: result.stdout, stderr: result.stderr };
  }

  // Räumt das temporäre Verzeichnis auf
  dispose(): void {
    rmSync(this.base, { recursive: true, force: true });
  }
}
