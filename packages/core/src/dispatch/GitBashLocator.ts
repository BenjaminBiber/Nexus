import { execFileSync } from 'node:child_process';
import { existsSync } from 'node:fs';
import { win32 } from 'node:path';

// Sucht die bash.exe von Git for Windows; Dateisystem und Befehlssuche sind für Tests austauschbar.
export class GitBashLocator {
  constructor(
    private readonly exists: (path: string) => boolean = (p) => existsSync(p),
    private readonly which: (command: string) => string | null = (c) => GitBashLocator.where(c),
  ) {}

  // Pfad der Git Bash oder null (nicht unter Windows oder nichts gefunden)
  locate(env: Record<string, string | undefined>, platform: string = process.platform): string | null {
    if (platform !== 'win32') return null;
    const given = env.CLAUDE_CODE_GIT_BASH_PATH?.trim();
    if (given && this.exists(given)) return given;
    const fromGit = this.fromGit();
    if (fromGit) return fromGit;
    const roots = [env.ProgramFiles, env['ProgramFiles(x86)'], env.LOCALAPPDATA && win32.join(env.LOCALAPPDATA, 'Programs')];
    for (const root of roots) {
      if (!root) continue;
      const candidate = win32.join(root, 'Git', 'bin', 'bash.exe');
      if (this.exists(candidate)) return candidate;
    }
    return null;
  }

  // bash.exe neben dem im PATH gefundenen git (…\cmd, …\mingw64\bin oder …\bin → Git-Wurzel)
  private fromGit(): string | null {
    const git = this.which('git');
    if (!git) return null;
    let dir = win32.dirname(git);
    const leaf = (p: string) => win32.basename(p).toLowerCase();
    if (leaf(dir) === 'bin' && ['mingw64', 'mingw32', 'usr'].includes(leaf(win32.dirname(dir)))) dir = win32.dirname(dir);
    const root = win32.dirname(dir);
    const candidate = win32.join(root, 'bin', 'bash.exe');
    return this.exists(candidate) ? candidate : null;
  }

  // Erster Treffer von `where <Befehl>`; null, wenn es den Befehl nicht gibt
  private static where(command: string): string | null {
    try {
      const out = execFileSync('where', [command], { encoding: 'utf8', stdio: ['ignore', 'pipe', 'ignore'], windowsHide: true });
      return out.split(/\r?\n/).map((l) => l.trim()).find(Boolean) ?? null;
    } catch {
      return null;
    }
  }
}
