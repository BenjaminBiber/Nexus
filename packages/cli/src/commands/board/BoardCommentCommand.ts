import type { Command } from 'commander';
import { HarnessError, TextSource } from '@harness/core';
import type { CliContext } from '../../CliContext.js';

// `harness board comment`: Kommentar an ein Ticket hängen.
export class BoardCommentCommand {
  private readonly text = new TextSource();

  constructor(private readonly context: CliContext) {}

  // Meldet `comment` unter `board` an
  register(board: Command): void {
    board
      .command('comment <id> [text]')
      .description('Hängt einen Kommentar an (Zwischenstand, Nachweis, Begründung).')
      .option('--body-file <datei>', 'Text aus Datei („-“ = Standardeingabe)')
      .option('--author <name>', 'wer kommentiert (z. B. claude)')
      .addHelpText('after', '\nBeispiel:\n  harness board comment ARC-0007 "Schritt 2 von 4 fertig" --author claude')
      .action((id: string, text: string | undefined, o: { bodyFile?: string; author?: string }) => this.context.withBoard(async (rt) => {
        const body = this.text.read(text, o.bodyFile);
        if (body === undefined) throw new HarnessError('usage', 'Kommentartext fehlt (Argument oder --body-file).');
        // Aus einer Session geschrieben: gilt sofort als gelesen und wird nicht in die Session zurückgespeist
        const comment = await rt.services.comments.add(id, body, rt.actor(o.author), { readByAgent: Boolean(process.env.HARNESS_SESSION) });
        this.context.printer().result(comment, [`Kommentar an ${comment.ticketId} gespeichert.`]);
      }));
  }
}
