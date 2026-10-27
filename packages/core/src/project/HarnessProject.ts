import { existsSync } from 'node:fs';
import { mkdir, writeFile } from 'node:fs/promises';
import { GitPaths } from '../git/GitPaths.js';
import { HarnessError } from '../errors/HarnessError.js';
import { Counter } from '../counter/Counter.js';
import { SessionNumbers } from '../sessions/SessionNumbers.js';
import { HarnessLayout } from './HarnessLayout.js';
import { TokenStore } from './TokenStore.js';

// Ein eingerichtetes Git-Repo: Harness-Ordner im gemeinsamen Git-Verzeichnis mit Token, Zählern und Session-Nummern.
// Tickets und Fragen liegen in der Board-Datenbank, nicht hier.
export class HarnessProject {
  // Gemeinsames Git-Verzeichnis je Ordner: ändert sich praktisch nie, ein Git-Start kostet unter Windows einige 100 ms.
  // Gemerkt werden nur Erfolge; ob der Harness eingerichtet ist, prüft open trotzdem jedes Mal.
  private static readonly commonDirs = new Map<string, string>();

  readonly counter: Counter;
  readonly sessionNumbers: SessionNumbers;

  private constructor(
    readonly layout: HarnessLayout,
    readonly root: string,
  ) {
    this.counter = new Counter(layout);
    this.sessionNumbers = new SessionNumbers(layout, this.counter);
  }

  // Richtet das Projekt ein; wiederholbar, vorhandene Daten bleiben unverändert
  static async init(cwd: string): Promise<HarnessProject> {
    const project = HarnessProject.locate(cwd);
    await mkdir(project.layout.dir, { recursive: true });
    if (!existsSync(project.layout.countersFile)) {
      await writeFile(project.layout.countersFile, JSON.stringify({ session: 0 }), { flag: 'wx' }).catch((error) => {
        if ((error as NodeJS.ErrnoException).code !== 'EEXIST') throw error;
      });
    }
    await new TokenStore(project.layout).ensure();
    return project;
  }

  // Öffnet ein eingerichtetes Projekt; ohne `harness init` gibt es einen Hinweis
  static async open(cwd: string): Promise<HarnessProject> {
    const project = await HarnessProject.locateCached(cwd);
    if (!existsSync(project.layout.countersFile) && !existsSync(project.layout.tokenFile)) {
      throw new HarnessError('not-initialized', 'Dieses Projekt ist nicht eingerichtet. Bitte zuerst `harness init` ausführen.');
    }
    return project;
  }

  // Ermittelt Harness-Ordner und Projektwurzel aus einem beliebigen Ordner (auch Worktree)
  private static locate(cwd: string): HarnessProject {
    const git = new GitPaths();
    const commonDir = git.commonDir(cwd);
    HarnessProject.commonDirs.set(cwd, commonDir);
    return new HarnessProject(new HarnessLayout(commonDir), git.rootOf(commonDir));
  }

  // Wie locate, aber nicht blockierend und mit gemerktem Git-Verzeichnis
  private static async locateCached(cwd: string): Promise<HarnessProject> {
    const git = new GitPaths();
    let commonDir = HarnessProject.commonDirs.get(cwd);
    if (commonDir === undefined) {
      commonDir = await git.commonDirAsync(cwd);
      HarnessProject.commonDirs.set(cwd, commonDir);
    }
    return new HarnessProject(new HarnessLayout(commonDir), git.rootOf(commonDir));
  }
}
