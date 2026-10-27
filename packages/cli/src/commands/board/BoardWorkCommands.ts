import type { Command } from 'commander';
import { SpecCommitCheck, TextSource, WorktreeManager } from '@harness/core';
import type { CliContext } from '../../CliContext.js';

// `harness board start|done`: Ticket im eigenen Worktree beginnen und abschließen (Schritte: BoardStepCommands).
export class BoardWorkCommands {
  private readonly text = new TextSource();
  private readonly commitCheck = new SpecCommitCheck();

  constructor(private readonly context: CliContext) {}

  // Meldet die beiden Befehle unter `board` an
  register(board: Command): void {
    board
      .command('start <id>')
      .description('Startet ein Ticket: eigener Branch ticket/<ID>-<Kurztitel> (vom Zielbranch des Tickets, sonst vom ausgecheckten Branch) und Worktree neben dem Projekt, Status In Arbeit (Sperren gelten). Mit --join tritt es stattdessen einem offenen Worktree derselben Spec bei, wenn sich die Bereiche nicht überschneiden. Nur für Tickets des Boards, das diesem Repo zugeordnet ist. Warnt, wenn das Spec-Dokument des Tickets nicht (vollständig) committet ist.')
      .option('--in <id>', 'dem Worktree dieses Tickets beitreten (bei Überschneidung entsteht ein eigener)')
      .option('--join', 'einem offenen Worktree derselben Spec ohne Überschneidung beitreten (früheres Verhalten)')
      .option('--force', 'offene Voraussetzungen und Fragen übergehen (nur auf ausdrückliche Anweisung)')
      .option('--by <name>', 'wer startet (z. B. claude)')
      .addHelpText('after', '\nBeispiele:\n  harness board start ARC-0007 --by claude\n  harness board start ARC-0008 --in ARC-0007\n  harness board start ARC-0009 --join')
      .action((id: string, o: { in?: string; join?: boolean; force?: boolean; by?: string }) => this.context.withBoard(async (rt) => {
        const project = await this.context.openProject();
        const manager = new WorktreeManager(project.layout, rt.services, project.root, { joinWorktrees: await this.context.joinWorktrees() });
        const repoBoard = await rt.services.boards.requireForRepo(project.root, rt.config);
        const result = await manager.start(id, { inTicket: o.in, join: o.join, force: o.force, sessionId: process.env.HARNESS_SESSION, repoBoard }, this.context.projectDir(), rt.actor(o.by));
        const printer = this.context.printer();
        if (result.warning) printer.warn(result.warning);
        // Worktrees entstehen aus dem letzten Commit: ein nicht committetes Spec-Dokument fehlt dort (nur Warnung)
        const spec = (await rt.services.tickets.get(result.id)).spec;
        const specWarning = spec ? this.commitCheck.warningForDoc(project.root, spec) : null;
        if (specWarning) printer.warn(specWarning);
        const lines = [`${result.id} ${result.joined ? 'arbeitet jetzt' : 'läuft jetzt'} auf Branch ${result.branch} im Worktree ${result.path}`];
        if (result.targetCreated) lines.push(result.targetCreated);
        (result.removed ?? []).forEach((p) => lines.push(`Worktree ${p} aufgeräumt (der Branch bleibt erhalten).`));
        printer.result(result, lines);
      }));
    board
      .command('done <id>')
      .description('Setzt das Ticket auf Erledigt (Abnahme nötig) und meldet, ob der Worktree zum Merge bereit ist. Es wird nichts gemergt. Nur für Tickets des Boards dieses Repos.')
      .option('--acceptance <text>', 'Abnahme: woran man sieht, dass es fertig ist (falls noch keine eingetragen ist)')
      .option('--acceptance-file <datei>', 'Abnahme aus Datei („-“ = Standardeingabe)')
      .option('--force', 'offene Voraussetzungen und Fragen übergehen (nur auf ausdrückliche Anweisung)')
      .option('--by <name>', 'wer abschließt')
      .addHelpText('after', '\nBeispiel:\n  harness board done ARC-0007 --acceptance "Tests grün, Review erledigt" --by claude')
      .action((id: string, o: { acceptance?: string; acceptanceFile?: string; force?: boolean; by?: string }) => this.context.withBoard(async (rt) => {
        const project = await this.context.openProject();
        const manager = new WorktreeManager(project.layout, rt.services, project.root, { joinWorktrees: await this.context.joinWorktrees() });
        const repoBoard = await rt.services.boards.requireForRepo(project.root, rt.config);
        const result = await manager.finish(id, { acceptance: this.text.read(o.acceptance, o.acceptanceFile), force: o.force, repoBoard }, this.context.projectDir(), rt.actor(o.by));
        const printer = this.context.printer();
        result.warnings.forEach((w) => printer.warn(w));
        const lines = [`${result.id} ist erledigt.`];
        if (result.readyToMerge) lines.push(`Der Branch ${result.branch} ist bereit zum Merge (es wurde nichts gemergt).`);
        result.removed.forEach((p) => lines.push(`Worktree ${p} aufgeräumt (der Branch bleibt erhalten).`));
        printer.result(result, lines);
      }));
  }
}
