import { existsSync } from 'node:fs';
import { cp, mkdir, rm } from 'node:fs/promises';
import { basename, dirname, join, resolve } from 'node:path';
import { HarnessError } from '../errors/HarnessError.js';
import type { HarnessHome } from '../registry/HarnessHome.js';
import { LauncherWriter } from './LauncherWriter.js';
import { ShellProfilePath } from './ShellProfilePath.js';
import type { UserPath } from './UserPath.js';
import { WindowsUserPath } from './WindowsUserPath.js';

export interface InstallResult {
  /** Ordner mit den Startdateien (liegt im PATH) */
  binDir: string;
  /** Ordner mit dem kopierten CLI, Server und der Oberfläche */
  appDir: string;
  /** Ordner mit den zentralen Regeln, die Projekte importieren */
  rulesDir: string;
  /** Angelegte Startdateien */
  launchers: string[];
  /** true, wenn der PATH-Eintrag neu gesetzt wurde */
  pathAdded: boolean;
  /** true, wenn der PATH nicht angefasst wurde (Option --no-path) */
  pathSkipped: boolean;
}

// Richtet `harness` für das normale Terminal ein: kopiert das gebaute Plugin nach `~/.harness/app`, die Regeln
// nach `~/.harness/rules`, legt Startdateien in `~/.harness/bin` an und trägt diesen Ordner in den Benutzer-PATH ein.
export class CliInstaller {
  private readonly launchers = new LauncherWriter();

  constructor(
    private readonly home: HarnessHome,
    private readonly userPath: UserPath = process.platform === 'win32' ? new WindowsUserPath() : new ShellProfilePath(),
  ) {}

  // Plugin-Wurzel zur laufenden `cli.mjs` (Aufbau `<wurzel>/lib/cli.mjs`, daneben `lib/server.mjs` und `web/`); sonst null
  bundleRoot(entryFile: string): string | null {
    if (basename(entryFile) !== 'cli.mjs' || basename(dirname(entryFile)) !== 'lib') return null;
    const root = dirname(dirname(entryFile));
    const complete = existsSync(join(root, 'lib', 'server.mjs')) && existsSync(join(root, 'web', 'index.html'));
    return complete ? root : null;
  }

  // Installiert (oder aktualisiert) das CLI aus der Plugin-Wurzel; wiederholbar
  async install(root: string, options: { registerPath?: boolean } = {}): Promise<InstallResult> {
    const appDir = join(this.home.dir, 'app');
    const binDir = join(this.home.dir, 'bin');
    if (!this.isSame(root, appDir)) await this.copyApp(root, appDir);
    await this.copyRules(appDir);
    await this.copyCustomizations(appDir);
    const launchers = await this.launchers.write(binDir);
    const registerPath = options.registerPath !== false;
    const pathAdded = registerPath ? await this.userPath.add(binDir) : false;
    return { binDir, appDir, rulesDir: this.home.rulesDir, launchers, pathAdded, pathSkipped: !registerPath };
  }

  // Kopiert `lib/`, `web/` und, falls vorhanden, `rules/` und `customizations/` frisch (alte Dateien verschwinden)
  private async copyApp(root: string, appDir: string): Promise<void> {
    if (!existsSync(join(root, 'lib', 'cli.mjs'))) {
      throw new HarnessError('install-source-missing', `Im Ordner ${root} fehlt lib/cli.mjs.`);
    }
    await rm(appDir, { recursive: true, force: true });
    await mkdir(appDir, { recursive: true });
    await cp(join(root, 'lib'), join(appDir, 'lib'), { recursive: true });
    await cp(join(root, 'web'), join(appDir, 'web'), { recursive: true });
    if (existsSync(join(root, 'rules'))) await cp(join(root, 'rules'), join(appDir, 'rules'), { recursive: true });
    if (existsSync(join(root, 'customizations'))) await cp(join(root, 'customizations'), join(appDir, 'customizations'), { recursive: true });
  }

  // Legt die Regeln aus der Kopie frisch unter `~/.harness/rules` ab, dem festen Importziel der Projekte.
  // Der Ordner des Plugins in Claude Code taugt dafür nicht, er wechselt mit jeder Version.
  private async copyRules(appDir: string): Promise<void> {
    const source = join(appDir, 'rules');
    if (!existsSync(source)) return;
    await rm(this.home.rulesDir, { recursive: true, force: true });
    await cp(source, this.home.rulesDir, { recursive: true });
  }

  // Legt die angewendeten Anpassungen frisch unter `~/.harness/customizations` ab; Hook und Server lesen dort
  private async copyCustomizations(appDir: string): Promise<void> {
    const source = join(appDir, 'customizations');
    if (!existsSync(source)) return;
    await rm(this.home.customizationsDir, { recursive: true, force: true });
    await cp(source, this.home.customizationsDir, { recursive: true });
  }

  // Quelle und Ziel gleich? (Aufruf aus der bereits installierten Kopie; Windows ohne Groß-/Kleinschreibung)
  private isSame(a: string, b: string): boolean {
    const norm = (p: string) => (process.platform === 'win32' ? resolve(p).toLowerCase() : resolve(p));
    return norm(a) === norm(b);
  }
}
