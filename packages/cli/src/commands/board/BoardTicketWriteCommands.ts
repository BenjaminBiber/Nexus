import type { Command } from 'commander';
import { HarnessError, TextSource, type BoardTicketPatch, type Priority } from '@harness/core';
import type { CliContext } from '../../CliContext.js';
import { BoardOptionParser, collect } from '../../board/BoardOptionParser.js';
import { BoardSelection } from '../../board/BoardSelection.js';

interface CreateOptions {
  desc?: string; descFile?: string; status?: string; priority?: string; label?: string[]; planned?: boolean;
  acceptance?: string; acceptanceFile?: string; dependsOn?: string; board?: string; by?: string; force?: boolean; step?: string[];
  targetBranch?: string;
}

interface SetOptions {
  title?: string; desc?: string; descFile?: string; plan?: string; planFile?: string; handbook?: string; handbookFile?: string;
  diagram?: string; diagramFile?: string; status?: string; priority?: string; assignee?: string; label?: string[];
  blockedReason?: string; acceptance?: string; acceptanceFile?: string; needsPlanning?: string; dependsOn?: string;
  dependsOnAdd?: string[]; dependsOnRm?: string[]; toBoard?: string; clearBranch?: boolean; rev?: string; force?: boolean; by?: string;
  targetBranch?: string;
}

const TARGET_BRANCH_HELP = 'Zielbranch, in den der Dispatcher mergt (fehlt er, wird er angelegt; ohne: ausgecheckter Branch des Hauptordners)';

// `harness board create|set|move|delete`: Tickets schreiben.
export class BoardTicketWriteCommands {
  private readonly selection = new BoardSelection();
  private readonly parse = new BoardOptionParser();
  private readonly text = new TextSource();

  constructor(private readonly context: CliContext) {}

  // Meldet die Schreibbefehle unter `board` an
  register(board: Command): void {
    board
      .command('create <titel>')
      .description('Legt ein Ticket auf dem Board dieses Repos an, sonst auf dem aktiven Board (Standard: Backlog, normal, braucht Planung).')
      .option('--desc <text>', 'Beschreibung')
      .option('--desc-file <datei>', 'Beschreibung aus Datei („-“ = Standardeingabe)')
      .option('--status <status>', 'Startspalte')
      .option('--priority <prio>', 'low, normal, high oder urgent')
      .option('--label <label>', 'Label (wiederholbar oder kommagetrennt)', collect)
      .option('--planned', 'braucht keine Planung')
      .option('--acceptance <text>', 'Abnahme: woran man sieht, dass es fertig ist')
      .option('--acceptance-file <datei>', 'Abnahme aus Datei')
      .option('--depends-on <ids>', 'Voraussetzungen, kommagetrennt')
      .option('--step <text>', 'Arbeitsschritt (wiederholbar, in dieser Reihenfolge; Kommas bleiben erhalten)', collect)
      .option('--target-branch <name>', TARGET_BRANCH_HELP)
      .option('--board <id>', 'Board (Standard: Board dieses Repos, sonst aktives Board)')
      .option('--force', 'trotz offenem Ticket mit gleichem Titel anlegen; Sperren für Voraussetzungen und „nicht mehr nötig“ übergehen')
      .option('--by <name>', 'wer anlegt (z. B. claude)')
      .addHelpText('after', '\nBeispiele:\n  harness board create "Login bauen" --desc "Kontext" --priority high --label backend --by claude\n  harness board create "Export bauen" --step "Test schreiben" --step "Umsetzen"')
      .action((titel: string, o: CreateOptions) => this.context.withBoard(async (rt) => {
        const created = await rt.services.tickets.create(await this.selection.one(rt, o.board, this.context.projectDir()), {
          title: titel,
          description: this.text.read(o.desc, o.descFile),
          status: o.status ? this.parse.status(o.status) : undefined,
          priority: o.priority as Priority | undefined,
          labels: o.label ?? [],
          needsPlanning: o.planned ? false : undefined,
          acceptance: this.text.read(o.acceptance, o.acceptanceFile),
          dependsOn: o.dependsOn ? this.parse.dependsOn(o.dependsOn) : undefined,
          steps: o.step?.map((text) => ({ text, done: false })),
          targetBranch: o.targetBranch,
        }, rt.actor(o.by), { force: o.force, allowDuplicate: o.force });
        this.context.printer().result(created, [`${created.id} angelegt: ${created.title}`]);
      }));

    board
      .command('set <id>')
      .description('Ändert Felder eines Tickets. Texte nur ins Feld – Status, Planungsbedarf und Zuweisung bleiben, wenn nicht angegeben.')
      .option('--title <text>', 'Titel')
      .option('--desc <text>', 'Beschreibung').option('--desc-file <datei>', 'Beschreibung aus Datei')
      .option('--plan <text>', 'Plan').option('--plan-file <datei>', 'Plan aus Datei')
      .option('--handbook <text>', 'Handbuch').option('--handbook-file <datei>', 'Handbuch aus Datei')
      .option('--diagram <text>', 'Mermaid-Diagramm').option('--diagram-file <datei>', 'Diagramm aus Datei')
      .option('--status <status>', 'Status (gleiche Sperren wie move)')
      .option('--priority <prio>', 'low, normal, high oder urgent')
      .option('--assignee <name>', 'Zuweisung; „none“ entfernt sie')
      .option('--label <label>', 'ersetzt die Labels (wiederholbar)', collect)
      .option('--blocked-reason <text>', 'Grund für Blockiert')
      .option('--acceptance <text>', 'Abnahme').option('--acceptance-file <datei>', 'Abnahme aus Datei')
      .option('--needs-planning <true|false>', 'Planungsbedarf')
      .option('--depends-on <ids>', 'Voraussetzungen ersetzen; „none“ leert')
      .option('--depends-on-add <id>', 'Voraussetzung ergänzen (wiederholbar)', collect)
      .option('--depends-on-rm <id>', 'Voraussetzung entfernen (wiederholbar)', collect)
      .option('--target-branch <name>', `${TARGET_BRANCH_HELP}; „none“ entfernt ihn (nur vor dem Start)`)
      .option('--to-board <id>', 'auf ein anderes Board desselben Projekts verschieben')
      .option('--clear-branch', 'Branch und Worktree am Ticket leeren (z. B. wenn sie zu einem anderen oder aufgeräumten Worktree gehören); Branch und Ordner selbst bleiben')
      .option('--rev <n>', 'nur schreiben, wenn das Ticket noch diese Version hat')
      .option('--force', 'Sperren für Voraussetzungen und „nicht mehr nötig“ übergehen')
      .option('--by <name>', 'wer ändert')
      .addHelpText('after', '\nBeispiele:\n  harness board set ARC-0007 --priority high --by claude\n  harness board set ARC-0007 --plan-file plan.md\n  harness board set ARC-0007 --status blockiert --blocked-reason "Zugang fehlt"\n  harness board set ARC-0007 --clear-branch')
      .action((id: string, o: SetOptions) => this.context.withBoard(async (rt) => {
        const updated = await rt.services.tickets.update(id, this.patch(o), rt.actor(o.by), { rev: this.parse.rev(o.rev), force: o.force });
        this.context.printer().result(updated, [`${updated.id} geändert (rev ${updated.rev})`]);
      }));

    board
      .command('move <id> <status>')
      .description('Verschiebt in eine Spalte. Erledigt braucht eine Abnahme; In Arbeit/Erledigt keine offene Voraussetzung.')
      .option('--acceptance <text>', 'Abnahme im selben Schritt setzen')
      .option('--rev <n>', 'erwartete Version')
      .option('--force', 'Sperren für Voraussetzungen und „nicht mehr nötig“ übergehen')
      .option('--by <name>', 'wer verschiebt')
      .addHelpText('after', '\nBeispiele:\n  harness board move ARC-0007 erledigt --acceptance "Test deckt den Fall ab" --by claude\n  harness board move ARC-0007 geplant')
      .action((id: string, status: string, o: { acceptance?: string; rev?: string; force?: boolean; by?: string }) => this.context.withBoard(async (rt) => {
        const moved = await rt.services.tickets.move(id, this.parse.status(status), rt.actor(o.by), { acceptance: o.acceptance, rev: this.parse.rev(o.rev), force: o.force });
        this.context.printer().result(moved, [`${moved.id} → ${moved.status}`]);
      }));

    board
      .command('delete <id>')
      .description('Löscht ein Ticket endgültig (wird protokolliert). Nur auf ausdrückliche Anweisung.')
      .option('--by <name>', 'wer löscht')
      .addHelpText('after', '\nBeispiel:\n  harness board delete ARC-0007')
      .action((id: string, o: { by?: string }) => this.context.withBoard(async (rt) => {
        await rt.services.tickets.delete(id, rt.actor(o.by));
        this.context.printer().result({ deleted: id }, [`${id} gelöscht.`]);
      }));
  }

  // Baut die Änderung aus den Optionen
  private patch(o: SetOptions): BoardTicketPatch {
    if (o.dependsOn !== undefined && (o.dependsOnAdd?.length || o.dependsOnRm?.length)) {
      throw new HarnessError('usage', '--depends-on lässt sich nicht mit --depends-on-add/--depends-on-rm kombinieren.');
    }
    return {
      title: o.title,
      description: this.text.read(o.desc, o.descFile),
      plan: this.text.read(o.plan, o.planFile),
      handbook: this.text.read(o.handbook, o.handbookFile),
      diagram: this.text.read(o.diagram, o.diagramFile),
      status: o.status ? this.parse.status(o.status) : undefined,
      priority: o.priority as Priority | undefined,
      assignee: o.assignee === undefined ? undefined : o.assignee.trim().toLowerCase() === 'none' ? null : o.assignee,
      labels: o.label?.length ? o.label : undefined,
      blockedReason: o.blockedReason,
      acceptance: this.text.read(o.acceptance, o.acceptanceFile),
      needsPlanning: o.needsPlanning === undefined ? undefined : this.parse.bool(o.needsPlanning),
      dependsOn: o.dependsOn === undefined ? undefined : this.parse.dependsOn(o.dependsOn),
      dependsOnAdd: o.dependsOnAdd?.length ? o.dependsOnAdd : undefined,
      dependsOnRemove: o.dependsOnRm?.length ? o.dependsOnRm : undefined,
      targetBranch: o.targetBranch === undefined ? undefined : o.targetBranch.trim().toLowerCase() === 'none' ? null : o.targetBranch,
      boardId: o.toBoard === undefined ? undefined : this.selection.parseId(o.toBoard),
      ...(o.clearBranch ? { branch: null, worktree: null } : {}),
    };
  }
}
