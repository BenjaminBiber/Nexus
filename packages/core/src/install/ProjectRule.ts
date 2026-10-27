import { existsSync } from 'node:fs';
import { mkdir, readFile } from 'node:fs/promises';
import { homedir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { GitExclude } from '../git/GitExclude.js';
import { GitPaths } from '../git/GitPaths.js';
import { AtomicWriter } from '../locking/AtomicWriter.js';
import { HarnessHome } from '../registry/HarnessHome.js';

export interface ProjectRuleSpec {
  /** Ort der Rule im Projekt, relativ zur Wurzel (Hauptordner oder Worktree), mit Schrägstrichen */
  path: string;
  /** Datei unter `~/.harness/rules`, die die Rule importiert (z. B. `docs.md`) */
  importFile: string;
  /** Optionale Eingrenzung (`paths` im Frontmatter); ohne sie lädt Claude Code die Rule immer */
  paths?: string[];
}

// Eine Rule im Projekt, die nur einen Import auf eine zentrale Regeldatei unter `~/.harness/rules` enthält.
// Den Inhalt dort aktualisiert `npm run setup`. Die Rule ist lokal wie die Plugin-Einstellung und wird über
// `info/exclude` aus Git herausgenommen.
export class ProjectRule {
  private readonly writer = new AtomicWriter();
  private readonly exclude = new GitExclude();
  private readonly git = new GitPaths();

  constructor(
    readonly spec: ProjectRuleSpec,
    private readonly home: HarnessHome = new HarnessHome(),
  ) {}

  // Ort der Rule relativ zur Projektwurzel
  get path(): string {
    return this.spec.path;
  }

  // Datei der Rule im Projekt
  file(projectDir: string): string {
    return join(projectDir, ...this.spec.path.split('/'));
  }

  // Importziel: beim Standardordner `~/…`, sonst der absolute Pfad mit Schrägstrichen und maskierten Leerzeichen
  importTarget(): string {
    if (resolve(this.home.dir) === resolve(homedir(), '.harness')) return `~/.harness/rules/${this.spec.importFile}`;
    return join(this.home.rulesDir, this.spec.importFile).replace(/\\/g, '/').replace(/ /g, '\\ ');
  }

  // Inhalt der Rule: Frontmatter (mit Eingrenzung, falls angegeben), dann der Import
  content(): string {
    const paths = this.spec.paths ?? [];
    return [
      '---',
      '# Angelegt von `harness init`, nicht versioniert. Die Regeln liegen zentral und kommen mit `npm run setup`.',
      ...(paths.length > 0 ? ['paths:', ...paths.map((p) => `  - "${p}"`)] : []),
      '---',
      `@${this.importTarget()}`,
      '',
    ].join('\n');
  }

  // Schreibt die Rule, falls sie fehlt oder veraltet ist, und trägt sie in `info/exclude` ein.
  // Liefert true, wenn die Rule neu geschrieben wurde.
  async ensure(projectDir: string): Promise<boolean> {
    await this.exclude.add(this.git.commonDir(projectDir), `/${this.spec.path}`);
    const file = this.file(projectDir);
    const content = this.content();
    if (existsSync(file) && (await readFile(file, 'utf8')) === content) return false;
    await mkdir(dirname(file), { recursive: true });
    await this.writer.write(file, content);
    return true;
  }
}
