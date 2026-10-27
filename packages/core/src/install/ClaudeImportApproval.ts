import { existsSync } from 'node:fs';
import { readFile } from 'node:fs/promises';
import { homedir } from 'node:os';
import { join } from 'node:path';
import { HarnessError } from '../errors/HarnessError.js';
import { AtomicWriter } from '../locking/AtomicWriter.js';

const APPROVED = 'hasClaudeMdExternalIncludesApproved';
const SHOWN = 'hasClaudeMdExternalIncludesWarningShown';

export interface ApprovalResult {
  /** `approved`: gesetzt; `unchanged`: war schon gesetzt; `no-config`: Claude Code lief auf diesem Rechner noch nie */
  status: 'approved' | 'unchanged' | 'no-config';
  /** Konfigurationsdatei von Claude Code, die geprüft oder geändert wurde */
  file: string;
  /** Projektschlüssel, deren Freigabe jetzt gesetzt ist */
  keys: string[];
}

type Projects = Record<string, Record<string, unknown>>;

// Gibt in der Konfiguration von Claude Code die Importe von außerhalb des Projekts frei, damit die Rules
// `@~/.harness/rules/docs.md` und `@~/.harness/rules/board.md` laden dürfen. Die Freigabe gilt je Projekt für alle
// Importe. Claude Code fragt danach sonst nur beim Start einer interaktiven Session, und das nie, wenn der Import
// erst beim Zugriff auf `docs/` geladen wird.
// Die beiden Werte sind nicht dokumentiert; Claude Code liest sie im Eintrag des Git-Stamms, auch beim Start in einem Unterordner.
export class ClaudeImportApproval {
  private readonly writer = new AtomicWriter();

  constructor(
    private readonly env: Record<string, string | undefined> = process.env,
    private readonly home: string = homedir(),
    private readonly platform: NodeJS.Platform = process.platform,
  ) {}

  // Datei wie bei Claude Code: ältere `~/.claude/.config.json`, sonst `.claude.json` in CLAUDE_CONFIG_DIR oder im Benutzerordner
  configFile(): string {
    const legacy = join(this.env.CLAUDE_CONFIG_DIR || join(this.home, '.claude'), '.config.json');
    if (existsSync(legacy)) return legacy;
    return join(this.env.CLAUDE_CONFIG_DIR || this.home, '.claude.json');
  }

  // Schlüssel des Projekts: unter Windows mit Schrägstrichen und Laufwerk groß und klein (VS Code meldet es klein),
  // dazu jede vorhandene Schreibweise desselben Pfads
  keys(projectDir: string, projects: Projects): string[] {
    if (this.platform !== 'win32') return [projectDir];
    const slashed = projectDir.replace(/\\/g, '/');
    const variants = new Set([slashed.charAt(0).toUpperCase() + slashed.slice(1), slashed.charAt(0).toLowerCase() + slashed.slice(1)]);
    const same = this.comparable(slashed);
    for (const key of Object.keys(projects)) if (this.comparable(key) === same) variants.add(key);
    return [...variants];
  }

  // Setzt beide Werte in allen Schlüsseln des Projekts; alles andere in der Datei bleibt unverändert.
  // Eine fehlende Datei wird nicht angelegt, kaputtes JSON nie überschrieben.
  async approve(projectDir: string): Promise<ApprovalResult> {
    const file = this.configFile();
    if (!existsSync(file)) return { status: 'no-config', file, keys: [] };
    const config = await this.read(file);
    const projects = this.projects(config);
    const keys = this.keys(projectDir, projects);
    if (keys.every((k) => projects[k]?.[APPROVED] === true && projects[k]?.[SHOWN] === true)) return { status: 'unchanged', file, keys };
    for (const key of keys) projects[key] = { ...(projects[key] ?? {}), [APPROVED]: true, [SHOWN]: true };
    config.projects = projects;
    await this.writer.write(file, `${JSON.stringify(config, null, 2)}\n`);
    await this.verify(file, keys);
    return { status: 'approved', file, keys };
  }

  // Liest die Datei erneut: hat eine gleichzeitig laufende Session sie überschrieben, gibt es einen Fehler mit Hinweis
  private async verify(file: string, keys: string[]): Promise<void> {
    const projects = this.projects(await this.read(file));
    if (keys.every((k) => projects[k]?.[APPROVED] === true)) return;
    throw new HarnessError('approval-lost', `Die Freigabe in ${file} wurde gleich wieder überschrieben. Bitte \`harness init\` wiederholen.`);
  }

  // Liest die Konfiguration als Objekt
  private async read(file: string): Promise<Record<string, unknown>> {
    const text = await readFile(file, 'utf8');
    try {
      const parsed: unknown = JSON.parse(text);
      if (typeof parsed === 'object' && parsed !== null && !Array.isArray(parsed)) return parsed as Record<string, unknown>;
    } catch {
      // fällt unten in den Fehler
    }
    throw new HarnessError('claude-config-unreadable', `${file} ist kein gültiges JSON. Bitte prüfen; die Datei wurde nicht verändert.`);
  }

  // Der Abschnitt `projects` als Objekt
  private projects(config: Record<string, unknown>): Projects {
    const value = config.projects;
    return typeof value === 'object' && value !== null && !Array.isArray(value) ? { ...(value as Projects) } : {};
  }

  // Vergleichsform eines Windows-Pfads: Schrägstriche, ohne Schrägstrich am Ende, Kleinbuchstaben
  private comparable(path: string): string {
    return path.replace(/\\/g, '/').replace(/\/+$/, '').toLowerCase();
  }
}
