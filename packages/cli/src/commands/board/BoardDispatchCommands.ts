import type { Command } from 'commander';
import { HarnessError } from '@harness/core';
import { BoardSelection } from '../../board/BoardSelection.js';
import { DispatchClient, type DispatchStatusView } from '../../board/DispatchClient.js';
import { DispatchOptionParser } from '../../board/DispatchOptionParser.js';
import { DispatchStatusFormatter } from '../../board/DispatchStatusFormatter.js';
import type { CliContext } from '../../CliContext.js';

// `harness board dispatch …` steuert den Dispatcher im lokalen Server, `harness board merge <id>` stößt einen Merge erneut an.
export class BoardDispatchCommands {
  private readonly formatter = new DispatchStatusFormatter();
  private readonly parser = new DispatchOptionParser();
  private readonly selection = new BoardSelection();
  private board?: Command;

  constructor(private readonly context: CliContext) {}

  // Meldet die Befehle unter `board` an
  register(board: Command): void {
    this.board = board;
    const dispatch = board
      .command('dispatch')
      .description('Dispatcher: übernimmt bereite Tickets (Label auto) der eigenen Boards und lässt sie von claude -p umsetzen. Einstellungen in der Oberfläche oder ~/.harness/dispatch.yaml.');
    dispatch
      .command('status')
      .description('Zeigt Zustand, laufende Worker, Merges und warum Geplant-Tickets (noch) nicht bereit sind. Ändert nichts.')
      .option('--board <id>', 'Board für die Bereitschaft (Standard: aktives Board)')
      .addHelpText('after', '\nBeispiel:\n  harness board dispatch status --board 3')
      .action((o: { board?: string }) => this.show((c) => c.status(this.parser.boardId(o.board))));
    dispatch
      .command('run')
      .description('Führt sofort einen Durchlauf aus (sonst jede Minute).')
      .option('--board <id>', 'Board für die anschließende Übersicht')
      .addHelpText('after', '\nBeispiel:\n  harness board dispatch run')
      .action((o: { board?: string }) => this.show((c) => c.run(this.parser.boardId(o.board))));
    dispatch
      .command('pause')
      .description('Startet für eine Weile nichts Neues (laufende Worker laufen weiter).')
      .option('--minutes <n>', 'Dauer in Minuten (1 bis 1440)', '60')
      .addHelpText('after', '\nBeispiel:\n  harness board dispatch pause --minutes 30')
      .action((o: { minutes: string }) => this.show((c) => c.pause(this.parser.minutes(o.minutes))));
    dispatch.command('resume').description('Hebt eine Pause auf.').addHelpText('after', '\nBeispiel:\n  harness board dispatch resume').action(() => this.show((c) => c.resume()));
    dispatch.command('enable').description('Schaltet den Dispatcher ein.').addHelpText('after', '\nBeispiel:\n  harness board dispatch enable').action(() => this.show((c) => c.setEnabled(true)));
    dispatch.command('disable').description('Schaltet den Dispatcher aus (laufende Worker laufen zu Ende).').addHelpText('after', '\nBeispiel:\n  harness board dispatch disable').action(() => this.show((c) => c.setEnabled(false)));
    board
      .command('dispatch-runs')
      .description('Läufe des Dispatchers für die Auswertung (Dashboard).')
      .command('backfill')
      .description('Trägt Läufe aus den [dispatch]-Kommentaren nach (Profil, Modell-Alias, Effort, Versuch, Eskalation, Abbruchgrund; ohne Kosten). ' +
        'Schon nachgetragene Läufe bleiben unverändert – beliebig oft ausführbar.')
      .option('--project <key>', 'Projekt (Standard: Projekt des Boards dieses Repos, sonst des aktiven Boards)')
      .option('--dry-run', 'nur zählen, nichts schreiben')
      .addHelpText('after', '\nBeispiele:\n  harness board dispatch-runs backfill --project ARC --dry-run\n  harness board dispatch-runs backfill')
      .action((o: { project?: string; dryRun?: boolean }) => this.backfill(o.project, o.dryRun === true));
    board
      .command('merge <id>')
      .description('Stößt den Merge eines erledigten Tickets eines eigenen Boards erneut an (nach „Blockiert: Merge: …“), erlaubt dabei wieder eine Merge-Session und nennt das Ergebnis (gemergt, blockiert mit Grund, wartet, Merge-Session).')
      .addHelpText('after', '\nBeispiel:\n  harness board merge ARC-0009')
      .action((id: string) => this.mergeTicket(id));
    board
      .command('accept <id>')
      .description('Nimmt ein Ticket ab, das mit Grund „Human Interaction: …“ blockiert ist (nur ein Mensch, nie ein Agent): setzt Erledigt (Abnahme-Text nötig), leert den Grund, vermerkt es im Verlauf und stößt auf einem eigenen Board den Merge an wie „harness board merge“. Nennt das Ergebnis.')
      .option('--by <name>', 'wer abnimmt (Standard: Git-Benutzername)')
      .addHelpText('after', '\nBeispiel:\n  harness board accept ARC-0009 --by Florian')
      .action((id: string, o: { by?: string }) => this.acceptTicket(id, o.by));
    board
      .command('plan-start <id>')
      .description('Lässt ein Ticket eines eigenen Boards sofort von einer Claude-Session planen (claude -p, nur lesend, eigener losgelöster Worktree; ' +
        'wie „Planen“ im Ticket). Der Plan landet über plan-json am Ticket; fehlt eine Entscheidung, entsteht eine Frage. Läuft über den lokalen Server, ' +
        'unabhängig davon, ob der Dispatcher eingeschaltet ist.')
      .addHelpText('after', '\nBeispiel:\n  harness board plan-start ARC-0007')
      .action((id: string) => this.startPlanning(id));
    const question = board.commands.find((c) => c.name() === 'question');
    question
      ?.command('research-start <id>')
      .description('Lässt eine offene Frage sofort von einer Claude-Session (claude -p, nur lesend, eigener losgelöster Worktree) recherchieren; ' +
        'das Ergebnis landet als Recherche an der Frage. Läuft über den lokalen Server, unabhängig davon, ob der Dispatcher eingeschaltet ist.')
      .addHelpText('after', '\nBeispiel:\n  harness board question research-start ARC-F0007')
      .action((id: string) => this.startResearch(id));
  }

  // `harness board dispatch-runs backfill`: Läufe aus Kommentaren direkt in der Board-Datenbank nachtragen
  private backfill(projectKey: string | undefined, dryRun: boolean): Promise<void> {
    return this.context.withBoard(async (rt) => {
      const projects = await rt.services.projects.list();
      let projectId: number;
      if (projectKey) {
        const found = projects.find((p) => p.key.toUpperCase() === projectKey.toUpperCase());
        if (!found) throw new HarnessError('project-not-found', `Projekt ${projectKey} gibt es nicht.`);
        projectId = found.id;
      } else {
        projectId = (await rt.services.boards.require(await this.selection.one(rt, undefined, this.context.projectDir()))).projectId;
      }
      const key = projects.find((p) => p.id === projectId)?.key ?? String(projectId);
      const result = await rt.services.dispatchRuns.backfill(projectId, { dryRun });
      const lines = [
        `Projekt ${key}: ${result.found} Läufe in den [dispatch]-Kommentaren gefunden, ${result.created.length} ${dryRun ? 'würden neu angelegt (Trockenlauf)' : 'neu angelegt'}.`,
        ...result.created.map((r) =>
          `  ${r.subjectId}  Versuch ${r.attempt ?? '-'}  ${r.profile}${r.escalatedFrom ? ` (eskaliert von ${r.escalatedFrom})` : ''}  ${r.outcome}${r.abortCategory ? ` (${r.abortCategory})` : ''}`),
      ];
      this.context.printer().result(result, lines);
    });
  }

  // `harness board question research-start <id>`: Recherche über den Server starten
  private async startResearch(id: string): Promise<void> {
    const view = await new DispatchClient(this.port()).startResearch(id);
    this.context.printer().result(view, this.formatter.researchLines(view));
  }

  // `harness board plan-start <id>`: Planung über den Server starten
  private async startPlanning(id: string): Promise<void> {
    const view = await new DispatchClient(this.port()).startPlanning(id);
    this.context.printer().result(view, this.formatter.planLines(view));
  }

  // „Erneut mergen“ und das Ergebnis für dieses Ticket ausgeben (mit --json die ganze Antwort)
  private async mergeTicket(id: string): Promise<void> {
    const view = await new DispatchClient(this.port()).merge(id);
    this.context.printer().result(view, this.formatter.mergeLines(view));
  }

  // „Abnehmen“ über den Server (nur Mensch, nicht aus einer Session) und das Ergebnis für dieses Ticket ausgeben
  private async acceptTicket(id: string, by?: string): Promise<void> {
    const actor = this.context.humanActor(undefined, by);
    const view = await new DispatchClient(this.port()).accept(id, actor.author);
    this.context.printer().result(view, this.formatter.mergeLines(view));
  }

  // Führt den Aufruf aus und gibt die Übersicht aus
  private async show(call: (client: DispatchClient) => Promise<DispatchStatusView>): Promise<void> {
    const status = await call(new DispatchClient(this.port()));
    this.context.printer().result(status, this.formatter.lines(status));
  }

  // Port: Option --port von `board`, dann HARNESS_PORT, dann Standard
  private port(): number {
    return this.parser.port(this.board?.opts<{ port?: string }>().port);
  }
}
