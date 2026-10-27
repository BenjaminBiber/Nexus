import type { Command } from 'commander';
import { HarnessError, TextSource } from '@harness/core';
import type { CliContext } from '../../CliContext.js';
import { BoardOptionParser } from '../../board/BoardOptionParser.js';
import { BoardSelection } from '../../board/BoardSelection.js';

interface PlanOptions {
  plan?: string; planFile?: string; handbook?: string; handbookFile?: string; diagram?: string; diagramFile?: string;
  status?: string; by?: string;
}

// `harness board plan|plan-json|next-planning`: Planungslauf.
export class BoardPlanningCommands {
  private readonly parse = new BoardOptionParser();
  private readonly selection = new BoardSelection();
  private readonly text = new TextSource();

  constructor(private readonly context: CliContext) {}

  // Meldet die Planungsbefehle unter `board` an
  register(board: Command): void {
    board
      .command('plan <id>')
      .description('Planungslauf: schreibt Plan/Handbuch/Diagramm, setzt Geplant und löscht den Planungsbedarf.')
      .option('--plan <text>', 'Plan').option('--plan-file <datei>', 'Plan aus Datei („-“ = Standardeingabe)')
      .option('--handbook <text>', 'Handbuch').option('--handbook-file <datei>', 'Handbuch aus Datei')
      .option('--diagram <text>', 'Mermaid ohne Code-Fence').option('--diagram-file <datei>', 'Diagramm aus Datei')
      .option('--status <status>', 'Zielstatus (Standard: planned)')
      .option('--by <name>', 'wer plant (Standard: Anzeigename)')
      .addHelpText('after', '\nBeispiel:\n  harness board plan ARC-0007 --plan-file plan.md --handbook-file hb.md --by claude')
      .action((id: string, o: PlanOptions) => this.context.withBoard(async (rt) => {
        const planned = await rt.services.planning.plan(id, {
          plan: this.text.read(o.plan, o.planFile),
          handbook: this.text.read(o.handbook, o.handbookFile),
          diagram: this.text.read(o.diagram, o.diagramFile),
          status: o.status ? this.parse.status(o.status) : undefined,
        }, rt.actor(o.by));
        this.context.printer().result(planned, [`${planned.id} geplant (${planned.status})`]);
      }));
    board
      .command('plan-json <id>')
      .description('Planungslauf aus JSON {plan, handbook, diagram, acceptance?, status?, by?} in einem Aufruf.')
      .requiredOption('--file <datei>', 'JSON-Datei („-“ = Standardeingabe)')
      .option('--status <status>', 'Zielstatus, falls nicht im JSON')
      .option('--by <name>', 'wer plant, falls nicht im JSON')
      .addHelpText('after', '\nBeispiel:\n  harness board plan-json ARC-0007 --file payload.json --by claude')
      .action((id: string, o: { file: string; status?: string; by?: string }) => this.context.withBoard(async (rt) => {
        const input = this.readJson(o.file);
        const status = input.status ?? o.status;
        const planned = await rt.services.planning.plan(id, {
          plan: input.plan,
          handbook: input.handbook,
          diagram: input.diagram,
          acceptance: input.acceptance,
          status: status ? this.parse.status(status) : undefined,
        }, rt.actor(input.by ?? o.by));
        this.context.printer().result(planned, [`${planned.id} geplant (${planned.status})`]);
      }));
    board
      .command('next-planning')
      .description('Nächstes Ticket mit Planungsbedarf; mit --claim für 15 Minuten beansprucht. Überspringt In Planung.')
      .option('--claim <agent>', 'Ticket für diesen Agenten beanspruchen')
      .option('--board <id|all>', 'Board (Standard: Board dieses Repos, sonst aktives Board)')
      .addHelpText('after', '\nBeispiel:\n  harness --json board next-planning --claim claude')
      .action((o: { claim?: string; board?: string }) => this.context.withBoard(async (rt) => {
        const ticket = await rt.services.planning.next(await this.selection.many(rt, o.board, this.context.projectDir()), o.claim ?? null, rt.actor(o.claim));
        this.context.printer().result({ ticket }, [ticket ? `${ticket.id}  ${ticket.title}` : 'Kein Ticket mit Planungsbedarf.']);
      }));
  }

  // Liest und prüft die JSON-Eingabe
  private readJson(file: string): { plan?: string; handbook?: string; diagram?: string; acceptance?: string; status?: string; by?: string } {
    const raw = this.text.read(undefined, file) ?? '';
    let parsed: unknown;
    try {
      parsed = JSON.parse(raw);
    } catch {
      throw new HarnessError('input-invalid', 'Die Eingabe für plan-json ist kein gültiges JSON.');
    }
    if (typeof parsed !== 'object' || parsed === null || Array.isArray(parsed)) {
      throw new HarnessError('input-invalid', 'Die Eingabe für plan-json muss ein JSON-Objekt sein.');
    }
    const input = parsed as Record<string, unknown>;
    for (const key of ['plan', 'handbook', 'diagram', 'acceptance', 'status', 'by']) {
      if (input[key] !== undefined && typeof input[key] !== 'string') {
        throw new HarnessError('input-invalid', `Das Feld „${key}“ in der Eingabe für plan-json muss ein Text sein.`);
      }
    }
    return input as { plan?: string; handbook?: string; diagram?: string; acceptance?: string; status?: string; by?: string };
  }
}
