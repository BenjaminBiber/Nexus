import type { Command } from 'commander';
import { BoardConfigStore, HarnessError } from '@harness/core';
import type { CliContext } from '../../CliContext.js';
import { BoardSelection } from '../../board/BoardSelection.js';
import { BoardTextFormatter } from '../../board/BoardTextFormatter.js';

// `harness board boards …`: Boards anlegen, auflisten, umschalten, eigenen Repo-Pfad setzen, löschen.
export class BoardBoardsCommands {
  private readonly selection = new BoardSelection();
  private readonly format = new BoardTextFormatter();

  constructor(private readonly context: CliContext) {}

  // Meldet die Gruppe `boards` unter `board` an
  register(board: Command): void {
    const boards = board.command('boards').description('Boards anlegen, auflisten, umschalten und löschen.');
    boards
      .command('list')
      .description('Listet Boards mit Projekt, Besitzer, offenen Tickets und deinem Repo-Pfad; * = aktives Board.')
      .option('--project <key>', 'nur Boards dieses Projekts')
      .option('--mine', 'nur eigene Boards')
      .addHelpText('after', '\nBeispiele:\n  harness board boards list\n  harness board boards list --project ARC --mine')
      .action((options: { project?: string; mine?: boolean }) => this.context.withBoard(async (rt) => {
        const list = await rt.services.boards.list({ projectKey: options.project, ownerId: options.mine ? rt.config.userId : undefined });
        this.context.printer().result(list, this.format.boardsText(list, rt.config.activeBoard));
      }));
    boards
      .command('create <projekt> <name>')
      .description('Legt ein eigenes Board im Projekt an.')
      .option('--use', 'danach als aktives Board setzen')
      .addHelpText('after', '\nBeispiel:\n  harness board boards create ARC "Sprint 12" --use')
      .action((projekt: string, name: string, options: { use?: boolean }) => this.context.withBoard(async (rt) => {
        const created = await rt.services.boards.create(projekt, name, rt.actor());
        if (options.use) await this.activate(created.id);
        this.context.printer().result(created, [`Board ${created.id} angelegt: ${created.name}${options.use ? ' (aktiv)' : ''}`]);
      }));
    boards
      .command('rename <id> <name>')
      .description('Benennt ein Board um.')
      .addHelpText('after', '\nBeispiel:\n  harness board boards rename 7 "Sprint 13"')
      .action((id: string, name: string) => this.context.withBoard(async (rt) => {
        const renamed = await rt.services.boards.rename(this.selection.parseId(id), name);
        this.context.printer().result(renamed, [`Board ${renamed.id} heißt jetzt ${renamed.name}`]);
      }));
    boards
      .command('use <id>')
      .description('Setzt das aktive Board dieses Rechners (Standard für alle Ticketbefehle).')
      .addHelpText('after', '\nBeispiel:\n  harness board boards use 7')
      .action((id: string) => this.context.withBoard(async (rt) => {
        const boardId = this.selection.parseId(id);
        const found = await rt.services.db.transaction((u) => rt.services.boards.get(u, boardId));
        await this.activate(found.id);
        this.context.printer().result(found, [`Aktives Board: ${found.id} ${found.name}`]);
      }));
    boards
      .command('repo <id> [pfad]')
      .description('Setzt deinen Repo-Pfad des Boards – nur für dich auf diesem Rechner (~/.harness/board-repos.yaml), nicht in der gemeinsamen DB; ohne Pfad zeigt er ihn.')
      .option('--clear', 'eigenen Repo-Pfad entfernen')
      .addHelpText('after', '\nBeispiele:\n  harness board boards repo 7 C:\\Projects\\Arches\\Shopsystem\n  harness board boards repo 7\n  harness board boards repo 7 --clear')
      .action((id: string, pfad: string | undefined, options: { clear?: boolean }) => this.context.withBoard(async (rt) => {
        const boardId = this.selection.parseId(id);
        if (pfad !== undefined && options.clear) throw new HarnessError('usage', 'Entweder einen Pfad angeben oder --clear, nicht beides.');
        const board = pfad !== undefined || options.clear
          ? await rt.services.boards.setRepoPath(boardId, options.clear ? null : pfad ?? null)
          : (await rt.services.boards.list()).find((b) => b.id === boardId) ?? await rt.services.boards.require(boardId);
        this.context.printer().result(board, [`Mein Repo-Pfad von Board ${board.id}: ${this.format.repoPathText(board)}`]);
      }));
    boards
      .command('delete <id>')
      .description('Löscht ein Board. Mit offenen Tickets nur mit --delete-open (löscht dann alles).')
      .option('--delete-open', 'auch offene Tickets endgültig löschen')
      .option('--by <name>', 'Name im Löschprotokoll')
      .addHelpText('after', '\nBeispiele:\n  harness board boards delete 7\n  harness board boards delete 7 --delete-open')
      .action((id: string, options: { deleteOpen?: boolean; by?: string }) => this.context.withBoard(async (rt) => {
        const boardId = this.selection.parseId(id);
        const result = await rt.services.boards.delete(boardId, { deleteOpen: Boolean(options.deleteOpen) }, rt.actor(options.by));
        if (rt.config.activeBoard === boardId) await this.activate(null);
        this.context.printer().result(result, [`Board ${boardId} gelöscht (${result.deletedTickets.length} Tickets).`]);
      }));
  }

  // Schreibt das aktive Board in die Konfig
  private async activate(id: number | null): Promise<void> {
    const store = new BoardConfigStore(this.context.home());
    const config = await store.require();
    await store.write({ ...config, activeBoard: id });
  }
}
