import type { Command } from 'commander';
import { HarnessError, INBOX_KIND_LABELS, INBOX_KINDS, InboxOrder, InboxOrderFile, type InboxItem, type InboxKind } from '@harness/core';
import type { CliContext } from '../../CliContext.js';
import { BoardSelection } from '../../board/BoardSelection.js';

// `harness board inbox`: „Meine Aufgaben“ – was auf den eigenen Boards gerade auf mich wartet, in meiner Reihenfolge.
export class BoardInboxCommands {
  private readonly selection = new BoardSelection();
  private readonly order = new InboxOrder();

  constructor(private readonly context: CliContext) {}

  register(board: Command): void {
    board
      .command('inbox')
      .description('Meine Aufgaben: wartende Tickets, offene Fragen und Notizen der eigenen Boards.')
      .option('--board <id>', 'nur dieses eigene Board (Standard: alle eigenen Boards)')
      .option('--kind <arten>', `nur diese Arten, kommagetrennt (${INBOX_KINDS.join(', ')})`)
      .option('--md', 'Ausgabe als Markdown')
      .addHelpText('after', '\nBeispiele:\n  harness board inbox --md\n  harness board inbox --kind merge,accept')
      .action((o: { board?: string; kind?: string; md?: boolean }) => this.context.withBoard(async (rt) => {
        const kinds = this.kinds(o.kind);
        const boardId = o.board === undefined ? undefined : this.selection.parseId(o.board);
        const listed = await rt.services.inbox.list(rt.actor().userId, { boardId });
        const ordered = this.order.apply(listed, await new InboxOrderFile(this.context.home()).read());
        const items = kinds ? ordered.filter((i) => kinds.includes(i.kind)) : ordered;
        this.context.printer().result(items, o.md ? this.markdown(items) : this.text(items));
      }));
  }

  // Kommagetrennte Arten oder undefined (alle)
  private kinds(raw?: string): InboxKind[] | undefined {
    if (raw === undefined) return undefined;
    const kinds = raw.split(',').map((k) => k.trim()).filter(Boolean);
    const unknown = kinds.find((k) => !(INBOX_KINDS as readonly string[]).includes(k));
    if (unknown || !kinds.length) {
      throw new HarnessError('input-invalid', `„${unknown ?? raw}“ ist keine Art. Erlaubt: ${INBOX_KINDS.join(', ')}.`);
    }
    return kinds as InboxKind[];
  }

  private text(items: InboxItem[]): string[] {
    if (!items.length) return ['Nichts wartet auf dich.'];
    return items.flatMap((item, index) => [
      `${index + 1}. [${INBOX_KIND_LABELS[item.kind]}] ${this.id(item)}  ${item.title}  (${this.where(item)})`,
      ...(item.detail ? [`   ${item.detail}`] : []),
      `   → ${this.action(item)}`,
    ]);
  }

  private markdown(items: InboxItem[]): string[] {
    if (!items.length) return ['_Nichts wartet auf dich._'];
    return items.map((item, index) => {
      const detail = item.detail ? ` – ${item.detail.replace(/\s+/g, ' ')}` : '';
      return `${index + 1}. **${INBOX_KIND_LABELS[item.kind]}** \`${this.id(item)}\` ${item.title} (${this.where(item)})${detail} → \`${this.action(item)}\``;
    });
  }

  private id(item: InboxItem): string {
    return item.questionId ?? item.ticketId ?? `Notiz ${item.noteId}`;
  }

  private where(item: InboxItem): string {
    const waiting = item.kind === 'question' ? `, ${item.waitingTickets} Ticket(s) warten` : '';
    return `${item.projectKey}${item.boardName ? ` · ${item.boardName}` : ''}${waiting}`;
  }

  // Befehl, mit dem es weitergeht
  private action(item: InboxItem): string {
    switch (item.kind) {
      case 'merge': return `harness board merge ${item.ticketId}`;
      case 'accept': return `harness board accept ${item.ticketId}`;
      case 'question': return `harness board question show ${item.questionId}`;
      case 'note': return `harness board note take ${item.noteId}`;
      default: return `harness board show ${item.ticketId}`;
    }
  }
}
