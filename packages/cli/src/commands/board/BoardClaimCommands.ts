import type { Command } from 'commander';
import type { CliContext } from '../../CliContext.js';
import { BoardOptionParser } from '../../board/BoardOptionParser.js';
import { ServerSessionNotice } from '../../hooks/ServerSessionNotice.js';

// `harness board claim|release`: Tickets übernehmen und freigeben.
export class BoardClaimCommands {
  private readonly parse = new BoardOptionParser();

  constructor(private readonly context: CliContext) {}

  // Meldet `claim` und `release` unter `board` an
  register(board: Command): void {
    board
      .command('claim <id>')
      .description('Übernimmt ein Ticket für einen Agenten (Standard: In Arbeit). Bereits zugewiesen (auch an denselben Namen) → Exit 1, außer mit --force. Innerhalb einer Session wird sie mit dem Ticket verknüpft.')
      .requiredOption('--agent <name>', 'Name des Agenten, z. B. claude')
      .option('--status <status>', 'Zielstatus (Standard: in_progress)')
      .option('--force', 'bestehende Zuweisung und offene Voraussetzungen übergehen')
      .addHelpText('after', '\nBeispiel:\n  harness board claim ARC-0007 --agent claude')
      .action((id: string, o: { agent: string; status?: string; force?: boolean }) => this.context.withBoard(async (rt) => {
        const sessionId = process.env.HARNESS_SESSION;
        const claimed = await rt.services.tickets.claim(id, o.agent, rt.actor(o.agent), {
          status: o.status ? this.parse.status(o.status) : undefined, force: o.force, sessionId,
        });
        if (sessionId) await new ServerSessionNotice().tell(this.context.projectDir(), claimed.id, sessionId);
        this.context.printer().result(claimed, [`${claimed.id} übernommen von ${claimed.assignee} (${claimed.status})`]);
      }));
    board
      .command('release <id>')
      .description('Gibt ein Ticket frei; der Status ändert sich nur mit --status.')
      .option('--status <status>', 'neuer Status')
      .option('--by <name>', 'wer freigibt')
      .addHelpText('after', '\nBeispiel:\n  harness board release ARC-0007 --status geplant')
      .action((id: string, o: { status?: string; by?: string }) => this.context.withBoard(async (rt) => {
        const released = await rt.services.tickets.release(id, rt.actor(o.by), { status: o.status ? this.parse.status(o.status) : undefined });
        this.context.printer().result(released, [`${released.id} freigegeben (${released.status})`]);
      }));
  }
}
