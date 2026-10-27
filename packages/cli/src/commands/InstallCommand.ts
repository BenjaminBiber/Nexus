import { fileURLToPath } from 'node:url';
import type { Command } from 'commander';
import { CliInstaller, HarnessError, HarnessHome } from '@harness/core';
import type { CliContext } from '../CliContext.js';

// `harness install`: macht `harness` im normalen Terminal verfügbar (kopiert das CLI nach ~/.harness, ergänzt den PATH).
export class InstallCommand {
  constructor(private readonly context: CliContext) {}

  // Meldet den Befehl beim Programm an
  register(program: Command): void {
    program
      .command('install')
      .description('Macht den Befehl harness im normalen Terminal verfügbar (Kopie unter ~/.harness, Eintrag im PATH des Benutzers).')
      .option('--no-path', 'PATH nicht ändern, nur kopieren')
      .addHelpText('after', '\nBeispiel:\n  harness install\n\nWiederholbar; aktualisiert die Kopie nach einem Plugin-Update.')
      .action((options: { path: boolean }) => this.run(options.path));
  }

  // Ablauf: gebündeltes Plugin erkennen, installieren, nächste Schritte nennen
  private async run(registerPath: boolean): Promise<void> {
    const installer = new CliInstaller(new HarnessHome());
    const root = installer.bundleRoot(fileURLToPath(import.meta.url));
    if (!root) {
      throw new HarnessError(
        'install-not-bundled',
        'harness install funktioniert nur aus dem gebauten Plugin (npm run build:plugin). Im Repository: npm link -w packages/cli.',
      );
    }
    const result = await installer.install(root, { registerPath });
    this.context.printer().result(result, this.lines(result, registerPath));
  }

  // Ausgabe für Menschen
  private lines(result: { binDir: string; appDir: string; rulesDir: string; pathAdded: boolean }, registerPath: boolean): string[] {
    const lines = [`Installiert: ${result.appDir}`, `Startdateien: ${result.binDir}`, `Regeln: ${result.rulesDir}`];
    if (!registerPath) lines.push('PATH nicht geändert (--no-path).');
    else if (result.pathAdded) lines.push('PATH ergänzt. Bitte ein neues Terminal öffnen, dann funktioniert `harness` überall.');
    else lines.push('PATH war schon eingerichtet. Falls `harness` fehlt: neues Terminal öffnen.');
    return lines;
  }
}
