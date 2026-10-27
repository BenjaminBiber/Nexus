import { resolve } from 'node:path';
import type { Command } from 'commander';
import { DEFAULT_SERVER_PORT, HarnessError } from '@harness/core';
import { BrowserOpener } from '../board/BrowserOpener.js';
import { ServerLauncher } from '../board/ServerLauncher.js';
import type { CliContext } from '../CliContext.js';

// `harness board`: startet bei Bedarf den Server, meldet das Projekt an und öffnet den Browser.
export class BoardCommand {
  private readonly launcher = new ServerLauncher();
  private readonly opener = new BrowserOpener();

  constructor(private readonly context: CliContext) {}

  // Meldet den Befehl beim Programm an und liefert ihn für die Unterbefehle zurück
  register(program: Command): Command {
    return program
      .command('board')
      .description('Ohne Unterbefehl: startet bei Bedarf den Server und öffnet das Board. Mit Unterbefehl: Board-Datenbank bedienen.')
      .option('--port <nummer>', `Port des Servers (Standard ${DEFAULT_SERVER_PORT} oder HARNESS_PORT)`)
      .option('--no-open', 'Browser nicht öffnen, nur die Adresse ausgeben')
      .addHelpText('after', '\nBeispiele:\n  harness board\n  harness board --port 4800 --no-open')
      .action((options: { port?: string; open: boolean }, command: Command) => this.dispatch(options, command.args));
  }

  // Ohne weitere Wörter startet der Server; Wörter, die kein registrierter Unterbefehl sind, werden abgewiesen.
  // Die Abweisung erfolgt ohne Datenbankzugriff.
  private async dispatch(options: { port?: string; open: boolean }, extra: string[]): Promise<void> {
    if (!extra.length) return this.run(options);
    throw new HarnessError('command-unknown', `Unbekannter Befehl „board ${extra.join(' ')}“. Hilfe: harness board --help`);
  }

  // Ablauf laut Vertrag: Projekt prüfen, Port prüfen, Server starten, anmelden, öffnen
  private async run(options: { port?: string; open: boolean }): Promise<void> {
    await this.context.openProject();
    const port = this.port(options.port);
    const state = await this.launcher.probe(port);
    if (state === 'foreign') {
      throw new HarnessError(
        'port-foreign',
        `Port ${port} wird von einem anderen Programm benutzt. Mit --port einen anderen Port wählen.`,
      );
    }
    if (state === 'free') await this.launcher.start(port);
    const project = await this.announceProject(port);
    const url = `http://127.0.0.1:${port}/p/${project.id}/board`;
    this.context.printer().result({ port, url, started: state === 'free', project }, [
      `Server läuft auf http://127.0.0.1:${port}`,
      `Board: ${url}`,
    ]);
    if (options.open && !(await this.opener.open(url))) {
      this.context.printer().warn('Der Browser konnte nicht geöffnet werden. Bitte die Adresse selbst öffnen.');
    }
  }

  // Port aus Option, Umgebung oder Standard
  private port(option?: string): number {
    const raw = option ?? process.env.HARNESS_PORT ?? String(DEFAULT_SERVER_PORT);
    const port = Number(raw);
    if (!Number.isInteger(port) || port < 1 || port > 65535) {
      throw new HarnessError('port-invalid', `Der Port „${raw}“ ist ungültig. Erlaubt sind Zahlen von 1 bis 65535.`);
    }
    return port;
  }

  // Meldet das aktuelle Projekt beim Server an
  private async announceProject(port: number): Promise<{ id: string; name: string; path: string }> {
    const res = await fetch(`http://127.0.0.1:${port}/api/projects`, {
      method: 'POST',
      headers: { 'content-type': 'application/json', 'x-harness': '1' },
      body: JSON.stringify({ path: resolve(this.context.projectDir()) }),
    });
    const body = (await res.json()) as { id?: string; name: string; path: string; message?: string };
    if (!res.ok || !body.id) {
      throw new HarnessError('register-failed', body.message ?? 'Das Projekt konnte nicht angemeldet werden.');
    }
    return body as { id: string; name: string; path: string };
  }
}
