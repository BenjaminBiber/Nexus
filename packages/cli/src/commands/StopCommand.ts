import type { Command } from 'commander';
import { HarnessHome, ServerStopper } from '@harness/core';
import { ServerLauncher } from '../board/ServerLauncher.js';
import type { CliContext } from '../CliContext.js';

// `harness stop`: beendet den laufenden Server; `harness board` startet ihn bei Bedarf neu.
export class StopCommand {
  constructor(private readonly context: CliContext) {}

  // Meldet den Befehl beim Programm an
  register(program: Command): void {
    program
      .command('stop')
      .description('Beendet den laufenden Harness-Server (nach einem Update, damit die neue Version startet).')
      .option('--restart', 'Startet den Server auf demselben Port neu, wenn er lief')
      .addHelpText('after', '\nBeispiele:\n  harness stop\n  harness stop --restart\n\nDas Board startet mit `harness board` wieder.')
      .action((options: { restart?: boolean }) => this.run(options.restart === true));
  }

  // Beendet den Server, startet ihn auf Wunsch wieder und meldet das Ergebnis
  private async run(restart: boolean): Promise<void> {
    const stopper = new ServerStopper(new HarnessHome());
    const stopped = await stopper.stop();
    const port = stopper.stoppedPort;
    const restarted = restart && stopped && port !== undefined;
    if (restarted) await new ServerLauncher().start(port);
    const lines = !stopped ? ['Es läuft kein Server.'] : restarted ? [`Server neu gestartet auf Port ${port}.`] : ['Server beendet.'];
    this.context.printer().result({ stopped, restarted }, lines);
  }
}
