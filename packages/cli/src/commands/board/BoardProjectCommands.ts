import type { Command } from 'commander';
import type { CliContext } from '../../CliContext.js';

// `harness board project list|create|rename`: Projekte verwalten.
export class BoardProjectCommands {
  constructor(private readonly context: CliContext) {}

  // Meldet die Gruppe `project` unter `board` an
  register(board: Command): void {
    const project = board.command('project').description('Projekte des Boards verwalten (Projekt → Boards → Tickets).');
    project
      .command('list')
      .description('Listet alle Projekte mit Key und Namen.')
      .addHelpText('after', '\nBeispiel:\n  harness board project list')
      .action(() => this.context.withBoard(async (rt) => {
        const projects = await rt.services.projects.list();
        this.context.printer().result(projects, projects.length ? projects.map((p) => `${p.key}  ${p.name}`) : ['Keine Projekte.']);
      }));
    project
      .command('create <key> <name>')
      .description('Legt ein Projekt an. Der Key (2–6 Zeichen) wird Präfix der Ticket-IDs.')
      .addHelpText('after', '\nBeispiel:\n  harness board project create ARC "Arches"')
      .action((key: string, name: string) => this.context.withBoard(async (rt) => {
        const created = await rt.services.projects.create(key, name, rt.actor());
        this.context.printer().result(created, [`Projekt ${created.key} angelegt: ${created.name}`]);
      }));
    project
      .command('rename <key> <name>')
      .description('Benennt ein Projekt um (der Key bleibt).')
      .addHelpText('after', '\nBeispiel:\n  harness board project rename ARC "Arches Shop"')
      .action((key: string, name: string) => this.context.withBoard(async (rt) => {
        const renamed = await rt.services.projects.rename(key, name);
        this.context.printer().result(renamed, [`Projekt ${renamed.key} heißt jetzt ${renamed.name}`]);
      }));
  }
}
