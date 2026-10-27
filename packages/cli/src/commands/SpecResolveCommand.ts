import type { Command } from 'commander';
import { DocCommentStore } from '@harness/core';
import type { CliContext } from '../CliContext.js';

// `harness spec resolve`: markiert einen Kommentar als erledigt und hält fest, was geändert wurde.
export class SpecResolveCommand {
  constructor(private readonly context: CliContext) {}

  // Meldet den Befehl in der Gruppe `spec` an
  register(spec: Command): void {
    spec
      .command('resolve <kommentar> <antwort>')
      .description('Markiert einen Kommentar als erledigt und speichert eine kurze Antwort.')
      .addHelpText('after', '\nBeispiel:\n  harness spec resolve K-1a2b3c4d "Ziel in Abschnitt 2 präzisiert"')
      .action((id: string, antwort: string) => this.run(id, antwort));
  }

  // Setzt den Status und die Antwort
  private async run(id: string, antwort: string): Promise<void> {
    const project = await this.context.openProject();
    const done = await new DocCommentStore(project.root).resolve(id, antwort);
    this.context.printer().result(done, [`Kommentar ${id} erledigt.`]);
  }
}
