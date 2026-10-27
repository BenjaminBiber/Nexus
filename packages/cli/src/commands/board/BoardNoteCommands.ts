import type { Command } from 'commander';
import { HarnessError, TicketIds, type NoteState, type Priority } from '@harness/core';
import type { CliContext } from '../../CliContext.js';
import { BoardOptionParser, collect } from '../../board/BoardOptionParser.js';
import { BoardSelection } from '../../board/BoardSelection.js';
import { BoardTextFormatter } from '../../board/BoardTextFormatter.js';

const STATES: Record<string, NoteState[]> = {
  offen: ['open'], open: ['open'],
  uebernommen: ['taken'], übernommen: ['taken'], taken: ['taken'],
  verworfen: ['dropped'], dropped: ['dropped'],
  alle: ['open', 'taken', 'dropped'], all: ['open', 'taken', 'dropped'],
};

// `harness board note …`: Notizen als Vorrat für spätere Tickets.
export class BoardNoteCommands {
  private readonly selection = new BoardSelection();
  private readonly parse = new BoardOptionParser();
  private readonly format = new BoardTextFormatter();
  private readonly ids = new TicketIds();

  constructor(private readonly context: CliContext) {}

  // Meldet die Gruppe `note` unter `board` an
  register(board: Command): void {
    const note = board.command('note').description('Notizen: Befunde über etwas anderes als das laufende Ticket.');
    note
      .command('add <titel>')
      .description('Hält einen Befund fest, der nicht zum laufenden Ticket gehört. Gibt es ihn schon, wird abgelehnt.')
      .option('--details <text>', 'wo, woran man es sieht, vermutete Ursache, Vorschlag')
      .option('--from <id>', 'Ticket, bei dem es aufgefallen ist')
      .option('--label <label>', 'Label (wiederholbar)', collect)
      .option('--author <name>', 'wer notiert (z. B. claude)')
      .option('--board <id>', 'Board (Standard: Board des --from-Tickets, sonst Board dieses Repos, sonst aktives Board)')
      .option('--force', 'auch anlegen, wenn eine ähnliche Notiz existiert')
      .addHelpText('after', [
        '',
        'Der Titel ist ein Satz, der ohne Kontext verständlich ist: was ist kaputt oder fehlt – ohne Ticket-ID.',
        'Die Herkunft steht in --from; „unabhängig von <Ticket>“ gehört nicht in den Text.',
        '',
        'Beispiel:',
        '  harness board note add "Cache der Board-Ansicht läuft nie ab" --details "BoardViewService.ts: ohne TTL; nach Ticketwechsel bleibt die alte Spalte stehen. Vorschlag: changeMarker prüfen." --from ARC-0009 --label cache --author claude',
      ].join('\n'))
      .action((title: string, o: { details?: string; from?: string; label?: string[]; author?: string; board?: string; force?: boolean }) => this.context.withBoard(async (rt) => {
        const boardId = o.board === undefined && o.from !== undefined
          ? (await rt.services.tickets.get(o.from)).boardId
          : await this.selection.one(rt, o.board, this.context.projectDir());
        const created = await rt.services.notes.add(boardId, { title, body: o.details }, rt.actor(o.author), { source: o.from, labels: o.label ?? [], force: o.force });
        this.context.printer().result(created, [`Notiz ${created.id} gespeichert.`]);
      }));
    note
      .command('seen <id>')
      .description('Derselbe Befund ist erneut aufgefallen: zählt die offene Notiz hoch statt einer neuen.')
      .option('--from <id>', 'Ticket, bei dem es erneut aufgefallen ist')
      .addHelpText('after', '\nBeispiel:\n  harness board note seen 12 --from ARC-0014')
      .action((id: string, o: { from?: string }) => this.context.withBoard(async (rt) => {
        const updated = await rt.services.notes.seen(this.noteId(id), { source: o.from });
        this.context.printer().result(updated, [`Notiz ${updated.id}: ${updated.seenCount}× aufgefallen.`]);
      }));
    note
      .command('list')
      .description('Listet Notizen; ohne --state nur offene.')
      .option('--state <stand>', 'offen, uebernommen, verworfen oder alle')
      .option('--from <id>', 'nur Notizen aus diesem Ticket')
      .option('--md', 'Ausgabe als Markdown')
      .option('--board <id>', 'Board (Standard: Board dieses Repos, sonst aktives Board)')
      .addHelpText('after', '\nBeispiele:\n  harness board note list --md\n  harness board note list --state alle')
      .action((o: { state?: string; from?: string; md?: boolean; board?: string }) => this.context.withBoard(async (rt) => {
        const states = o.state === undefined ? undefined : STATES[o.state.toLowerCase()];
        if (o.state !== undefined && !states) throw new HarnessError('state-invalid', `Stand „${o.state}“ ist ungültig. Erlaubt: offen, uebernommen, verworfen, alle.`);
        const notes = await rt.services.notes.list(await this.selection.one(rt, o.board, this.context.projectDir()), { states, source: o.from === undefined ? undefined : this.ids.canonical(o.from) });
        this.context.printer().result(notes, o.md ? this.format.notesMarkdown(notes) : this.format.notesText(notes));
      }));
    note
      .command('show <id>')
      .description('Zeigt eine Notiz vollständig.')
      .addHelpText('after', '\nBeispiel:\n  harness board note show 12')
      .action((id: string) => this.context.withBoard(async (rt) => {
        const found = await rt.services.notes.get(this.noteId(id));
        this.context.printer().result(found, this.format.noteText(found));
      }));
    note
      .command('take <id>')
      .description('Macht aus einer offenen Notiz ein Ticket (Titel der Notiz = Titel, Details = Beschreibung, Labels gehen mit).')
      .option('--title <text>', 'anderer Titel')
      .option('--status <status>', 'Startspalte (Standard: Backlog)')
      .option('--priority <prio>', 'low, normal, high oder urgent')
      .option('--label <label>', 'Labels ersetzen (wiederholbar)', collect)
      .option('--by <name>', 'wer übernimmt')
      .addHelpText('after', '\nBeispiel:\n  harness board note take 12 --priority high')
      .action((id: string, o: { title?: string; status?: string; priority?: string; label?: string[]; by?: string }) => this.context.withBoard(async (rt) => {
        const result = await rt.services.notes.take(this.noteId(id), rt.actor(o.by), {
          title: o.title,
          status: o.status ? this.parse.status(o.status) : undefined,
          priority: o.priority as Priority | undefined,
          labels: o.label?.length ? o.label : undefined,
        });
        this.context.printer().result(result, [`Notiz ${result.note.id} → ${result.ticket.id} ${result.ticket.title}`]);
      }));
    note
      .command('drop <id>')
      .description('Verwirft eine Notiz mit Grund (wird nie gelöscht).')
      .option('--reason <text>', 'warum daraus nichts wird (Pflicht)')
      .addHelpText('after', '\nBeispiel:\n  harness board note drop 12 --reason "erledigt sich mit ARC-0020"')
      .action((id: string, o: { reason?: string }) => this.context.withBoard(async (rt) => {
        const dropped = await rt.services.notes.drop(this.noteId(id), o.reason ?? '');
        this.context.printer().result(dropped, [`Notiz ${dropped.id} verworfen.`]);
      }));
  }

  // Notiz-Nummer aus Text
  private noteId(raw: string): number {
    const id = Number(raw);
    if (!Number.isInteger(id) || id < 1) throw new HarnessError('note-id-invalid', `„${raw}“ ist keine Notiz-Nummer.`);
    return id;
  }
}
