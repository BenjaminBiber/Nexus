import type { Command } from 'commander';
import { HarnessError, type BoardTicketDetail } from '@harness/core';
import type { CliContext } from '../../CliContext.js';
import { BoardOptionParser } from '../../board/BoardOptionParser.js';
import { BoardSelection } from '../../board/BoardSelection.js';
import { BoardTextFormatter } from '../../board/BoardTextFormatter.js';

const RAW_FIELDS = ['title', 'description', 'plan', 'handbook', 'diagram', 'acceptance', 'status', 'blockedReason'] as const;

interface ListOptions { board?: string; status?: string; needsPlanning?: boolean; assignee?: string; label?: string; q?: string; md?: boolean }

// `harness board list` und `harness board show`: Tickets lesen.
export class BoardTicketReadCommands {
  private readonly selection = new BoardSelection();
  private readonly parse = new BoardOptionParser();
  private readonly format = new BoardTextFormatter();

  constructor(private readonly context: CliContext) {}

  // Meldet `list` und `show` unter `board` an
  register(board: Command): void {
    board
      .command('list')
      .description('Listet Tickets je Spalte (Wartet ist abgeleitet). --md für Agenten, --json vollständig.')
      .option('--board <id|all>', 'Board (Standard: Board dieses Repos, sonst aktives Board)')
      .option('--status <spalten>', 'nur diese Spalten, kommagetrennt (auch „wartet“)')
      .option('--needs-planning', 'nur Tickets mit Planungsbedarf')
      .option('--assignee <name>', 'nur Tickets dieses Agenten')
      .option('--label <label>', 'nur Tickets mit diesem Label')
      .option('--q <text>', 'Suchtext in ID, Titel und Beschreibung')
      .option('--md', 'Ausgabe als Markdown')
      .addHelpText('after', '\nBeispiele:\n  harness board list --md\n  harness board list --status geplant,wartet --q login')
      .action((options: ListOptions) => this.context.withBoard(async (rt) => {
        const boardIds = await this.selection.many(rt, options.board, this.context.projectDir());
        const tickets = await rt.services.tickets.list({
          boardIds,
          columns: options.status ? this.parse.columns(options.status) : undefined,
          needsPlanning: options.needsPlanning ? true : undefined,
          assignee: options.assignee,
          label: options.label,
          q: options.q,
        });
        const heading = options.board === 'all' ? 'Alle Boards' : `Board ${boardIds[0]}`;
        this.context.printer().result(tickets, options.md ? this.format.listMarkdown(heading, tickets) : this.format.listText(tickets));
      }));
    board
      .command('show <id>')
      .description('Zeigt ein Ticket mit Texten, Voraussetzungen und Kommentaren (kompakt; --history, --all-comments für alles).')
      .option('--field <feld>', `nur ein Feld roh ausgeben (${RAW_FIELDS.join(', ')})`)
      .option('--history', 'Verlauf vollständig ausgeben')
      .option('--all-comments', 'auch ältere Kommentare von Agenten ausgeben')
      .addHelpText('after', '\nBeispiele:\n  harness board show ARC-0007\n  harness board show ARC-0007 --all-comments --history\n  harness board show ARC-0007 --field plan')
      .action((id: string, options: { field?: string; history?: boolean; allComments?: boolean }) => this.context.withBoard(async (rt) => {
        const detail = await rt.services.tickets.get(id);
        if (options.field) return this.printField(detail, options.field);
        this.context.printer().result(detail, this.format.showText(detail, { history: options.history, allComments: options.allComments }));
      }));
  }

  // Gibt ein einzelnes Feld ohne Formatierung aus
  private printField(detail: BoardTicketDetail, field: string): void {
    const key = RAW_FIELDS.find((f) => f === field);
    if (!key) throw new HarnessError('field-invalid', `Feld „${field}“ gibt es nicht. Erlaubt: ${RAW_FIELDS.join(', ')}.`);
    const value = detail[key] ?? '';
    this.context.printer().result({ [key]: value }, [String(value)]);
  }
}
