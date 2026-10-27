import type { Command } from 'commander';
import { HarnessError, QuestionIds, ResearchInputReader, TicketIds, type BoardQuestionActor, type QuestionState } from '@harness/core';
import type { BoardRuntime } from '@harness/db';
import type { CliContext } from '../../CliContext.js';
import { collect } from '../../board/BoardOptionParser.js';
import { BoardQuestionFormatter } from '../../board/BoardQuestionFormatter.js';
import { BoardSelection } from '../../board/BoardSelection.js';

// Zustände für --state (deutsch und englisch)
const STATES: Record<string, QuestionState | 'all'> = {
  offen: 'open', open: 'open',
  beantwortet: 'answered', answered: 'answered',
  verworfen: 'discarded', discarded: 'discarded',
  alle: 'all', all: 'all',
};

// Wer bei Agenten-Befehlen handelt
interface WhoOptions {
  kind?: string;
  author?: string;
  session?: string;
}

const KIND_HELP = 'Art des Autors: human, session oder subagent (Standard: session innerhalb einer Session, sonst human)';
const AUTHOR_HELP = 'Name des Autors (Standard: Name aus board.yaml)';
const SESSION_HELP = 'Kennung der Session (Standard: Umgebungsvariable HARNESS_SESSION)';

// `harness board question …`: Fragen auf der Board-Datenbank stellen, recherchieren, verknüpfen und (nur Menschen) entscheiden.
export class BoardQuestionCommands {
  private readonly selection = new BoardSelection();
  private readonly format = new BoardQuestionFormatter();
  private readonly reader = new ResearchInputReader();
  private readonly questionIds = new QuestionIds();
  private readonly ticketIds = new TicketIds();

  constructor(private readonly context: CliContext) {}

  // Meldet die Gruppe `question` unter `board` an
  register(board: Command): void {
    const question = board.command('question').description('Fragen auf der Board-Datenbank: fragen, recherchieren, entscheiden (entscheiden nur Menschen).');
    this.registerAsk(question);
    this.registerRead(question);
    this.registerDecide(question);
    this.registerRelations(question);
  }

  // add, research, comment: was Agenten tun
  private registerAsk(question: Command): void {
    question
      .command('add <titel>')
      .description('Stellt eine Frage; betroffene Tickets warten, bis sie entschieden ist.')
      .option('--description <text>', 'Beschreibung (Markdown, auch Mermaid)')
      .option('--ticket <id>', 'betroffenes Ticket (wiederholbar)', collect)
      .option('--depends <id>', 'Frage oder Ticket, von dem die Klärung abhängt (wiederholbar)', collect)
      .option('--tag <tag>', 'Tag (wiederholbar)', collect)
      .option('--occasion <text>', 'Anlass, z. B. „Planung ARC-0007“')
      .option('--project <key>', 'Board-Projekt (Standard: Projekt des Boards dieses Repos, sonst des aktiven Boards)')
      .option('--kind <art>', KIND_HELP)
      .option('--author <name>', AUTHOR_HELP)
      .option('--session <kennung>', SESSION_HELP)
      .addHelpText('after', '\nBeispiel:\n  harness board question add "Welcher Speicher?" --ticket ARC-0007 --occasion "Planung ARC-0007" --kind session --author claude')
      .action((title: string, o: WhoOptions & { description?: string; ticket?: string[]; depends?: string[]; tag?: string[]; occasion?: string; project?: string }) =>
        this.context.withBoard(async (rt) => {
          const created = await rt.services.questions.create(await this.projectKey(rt, o.project), {
            title, description: o.description, tags: o.tag ?? [], occasion: o.occasion ?? null, ticketIds: o.ticket ?? [], dependsOn: o.depends ?? [],
          }, this.actor(rt, o));
          this.context.printer().result(created, [`Frage ${created.id} angelegt: ${created.title}`]);
        }));
    question
      .command('research <id>')
      .description('Ergänzt eine Recherche mit Kontext und Optionen; frühere Recherchen bleiben erhalten.')
      .option('--from <datei>', 'JSON-Datei mit context und options, „-“ liest die Standardeingabe')
      .option('--context <text>', 'Nur Kontext als Text (ohne Optionen)')
      .option('--kind <art>', KIND_HELP)
      .option('--author <name>', AUTHOR_HELP)
      .option('--session <kennung>', SESSION_HELP)
      .addHelpText(
        'after',
        '\nDatei: {"context": "…", "options": [{"title": "…", "text": "…", "pros": ["…"], "cons": ["…"], "recommended": true}]}\n' +
          'Beispiel:\n  harness board question research ARC-F0001 --from recherche.json --kind session --author claude',
      )
      .action((id: string, o: WhoOptions & { from?: string; context?: string }) => {
        const input = this.reader.read({ from: o.from, context: o.context });
        return this.context.withBoard(async (rt) => {
          const updated = await rt.services.questions.addResearch(id, input, this.actor(rt, o));
          this.context.printer().result(updated.research.at(-1), [`Recherche zu ${updated.id} ergänzt: ${updated.history.at(-1)?.text ?? ''}.`]);
        });
      });
    question
      .command('comment <id> <text>')
      .description('Hängt einen Kommentar an eine Frage (Rückfrage, Zwischenstand, Hinweis).')
      .option('--kind <art>', KIND_HELP)
      .option('--author <name>', AUTHOR_HELP)
      .option('--session <kennung>', SESSION_HELP)
      .addHelpText('after', '\nBeispiel:\n  harness board question comment ARC-F0001 "Neuer Befund: …" --kind session --author claude')
      .action((id: string, text: string, o: WhoOptions) => this.context.withBoard(async (rt) => {
        const updated = await rt.services.questions.comment(id, text, this.actor(rt, o));
        this.context.printer().result(updated, [`Kommentar an ${updated.id} gespeichert.`]);
      }));
  }

  // list, show
  private registerRead(question: Command): void {
    question
      .command('list')
      .description('Listet Fragen eines Projekts (bereite zuerst, älteste zuerst); ohne --state nur offene.')
      .option('--state <zustand>', 'offen, beantwortet, verworfen oder alle')
      .option('--ticket <id>', 'nur Fragen, die dieses Ticket betreffen')
      .option('--project <key>', 'Board-Projekt (Standard: Projekt des Boards dieses Repos, sonst des aktiven Boards)')
      .option('--md', 'Ausgabe als Markdown')
      .addHelpText('after', '\nBeispiele:\n  harness board question list --md\n  harness board question list --state alle --ticket ARC-0007')
      .action((o: { state?: string; ticket?: string; project?: string; md?: boolean }) => this.context.withBoard(async (rt) => {
        const state = o.state === undefined ? 'open' : STATES[o.state.toLowerCase()];
        if (!state) throw new HarnessError('state-invalid', `Zustand „${o.state}“ ist ungültig. Erlaubt: offen, beantwortet, verworfen, alle.`);
        const result = await rt.services.questions.list(await this.projectKey(rt, o.project), { state, ticket: o.ticket });
        this.context.printer().result(result, o.md ? this.format.listMarkdown(result.questions) : this.format.listText(result.questions));
      }));
    question
      .command('show <id>')
      .description('Zeigt eine Frage mit Beschreibung, Tickets, Recherchen, Entscheidung, Kommentaren und Verlauf.')
      .addHelpText('after', '\nBeispiel:\n  harness board question show ARC-F0001')
      .action((id: string) => this.context.withBoard(async (rt) => {
        const found = await rt.services.questions.get(id);
        this.context.printer().result(found, this.format.showText(found));
      }));
  }

  // answer, discard, reopen: nur Menschen, geprüft vor dem Öffnen der Datenbank
  private registerDecide(question: Command): void {
    question
      .command('answer <id>')
      .description('Beantwortet eine Frage mit einer Option der neuesten Recherche oder eigenem Text; Begründung freiwillig. Nur Menschen.')
      .option('--option <nummer>', 'gewählte Option der neuesten Recherche, z. B. O2')
      .option('--text <text>', 'eigene Antwort statt einer Option')
      .option('--reason <text>', 'Begründung der Entscheidung (freiwillig)')
      .option('--author <name>', 'Name des Entscheidenden (Standard: Name aus board.yaml)')
      .addHelpText('after', '\nBeispiel:\n  harness board question answer ARC-F0001 --option O2 --reason "Passt zum Betrieb"')
      .action((id: string, o: { option?: string; text?: string; reason?: string; author?: string }) => {
        this.context.humanActor('human', o.author ?? '');
        return this.context.withBoard(async (rt) => {
          const updated = await rt.services.questions.answer(id, { option: o.option, text: o.text, reason: o.reason }, this.human(rt, o.author));
          this.context.printer().result(updated, [`${updated.id} beantwortet: ${updated.history.at(-1)?.text ?? ''}`]);
        });
      });
    question
      .command('discard <id>')
      .description('Verwirft eine Frage mit Begründung. Nur Menschen.')
      .option('--reason <text>', 'Begründung (Pflicht)')
      .option('--author <name>', 'Name des Entscheidenden (Standard: Name aus board.yaml)')
      .addHelpText('after', '\nBeispiel:\n  harness board question discard ARC-F0001 --reason "erledigt sich mit ARC-F0002"')
      .action((id: string, o: { reason?: string; author?: string }) => {
        this.context.humanActor('human', o.author ?? '');
        return this.context.withBoard(async (rt) => {
          const updated = await rt.services.questions.discard(id, o.reason, this.human(rt, o.author));
          this.context.printer().result(updated, [`${updated.id} verworfen.`]);
        });
      });
    question
      .command('reopen <id>')
      .description('Öffnet eine beantwortete oder verworfene Frage wieder. Nur Menschen.')
      .option('--reason <text>', 'Grund (optional)')
      .option('--author <name>', 'Name (Standard: Name aus board.yaml)')
      .addHelpText('after', '\nBeispiel:\n  harness board question reopen ARC-F0001 --reason "neue Lage"')
      .action((id: string, o: { reason?: string; author?: string }) => {
        this.context.humanActor('human', o.author ?? '');
        return this.context.withBoard(async (rt) => {
          const updated = await rt.services.questions.reopen(id, o.reason, this.human(rt, o.author));
          this.context.printer().result(updated, [`${updated.id} wieder geöffnet.`]);
        });
      });
  }

  // link (betroffene Tickets), depends (Fragen oder Tickets)
  private registerRelations(question: Command): void {
    question
      .command('link <id>')
      .description('Fügt betroffene Tickets hinzu oder entfernt sie.')
      .option('--add <ticket>', 'Ticket hinzufügen (wiederholbar)', collect, [])
      .option('--remove <ticket>', 'Ticket entfernen (wiederholbar)', collect, [])
      .option('--kind <art>', KIND_HELP)
      .option('--author <name>', AUTHOR_HELP)
      .option('--session <kennung>', SESSION_HELP)
      .addHelpText('after', '\nBeispiel:\n  harness board question link ARC-F0001 --add ARC-0008 --remove ARC-0007')
      .action((id: string, o: WhoOptions & { add: string[]; remove: string[] }) => {
        if (!o.add.length && !o.remove.length) throw new HarnessError('usage', 'Bitte --add oder --remove angeben.');
        return this.context.withBoard(async (rt) => {
          const current = await rt.services.questions.get(id);
          const removed = new Set(o.remove.map((t) => this.ticketIds.canonical(t)));
          const next = [...current.ticketIds.filter((t) => !removed.has(t)), ...o.add];
          const updated = await rt.services.questions.setTickets(id, next, this.actor(rt, o), { rev: current.rev });
          this.context.printer().result(updated, [`Betroffene Tickets von ${updated.id}: ${updated.ticketIds.join(', ') || '–'}`]);
        });
      });
    question
      .command('depends <id>')
      .description('Fügt Abhängigkeiten (Fragen oder Tickets) hinzu oder entfernt sie; Kreise werden abgelehnt.')
      .option('--add <id>', 'Frage oder Ticket hinzufügen (wiederholbar)', collect, [])
      .option('--remove <id>', 'Abhängigkeit entfernen (wiederholbar)', collect, [])
      .option('--kind <art>', KIND_HELP)
      .option('--author <name>', AUTHOR_HELP)
      .option('--session <kennung>', SESSION_HELP)
      .addHelpText('after', '\nBeispiel:\n  harness board question depends ARC-F0002 --add ARC-F0001')
      .action((id: string, o: WhoOptions & { add: string[]; remove: string[] }) => {
        if (!o.add.length && !o.remove.length) throw new HarnessError('usage', 'Bitte --add oder --remove angeben.');
        return this.context.withBoard(async (rt) => {
          const current = await rt.services.questions.get(id);
          const removed = new Set(o.remove.map((r) => this.questionIds.reference(r)));
          const next = [...current.dependsOn.filter((d) => !removed.has(d)), ...o.add];
          const updated = await rt.services.questions.setDepends(id, next, this.actor(rt, o), { rev: current.rev });
          this.context.printer().result(updated, [`Abhängigkeiten von ${updated.id}: ${updated.dependsOn.join(', ') || '–'}`]);
        });
      });
  }

  // Handelnder bei Agenten-Befehlen: Art wie bei Datei-Fragen, Name aus --author oder board.yaml
  private actor(rt: BoardRuntime, o: WhoOptions): BoardQuestionActor {
    const who = this.context.questionActor(o.kind, o.author ?? rt.config.userName, o.session);
    return { ...rt.actor(who.author), kind: who.kind, session: who.session ?? null };
  }

  // Handelnder beim Entscheiden: immer ein Mensch (die Sitzungsprüfung lief schon vor dem Öffnen der Datenbank)
  private human(rt: BoardRuntime, author?: string): BoardQuestionActor {
    return { ...rt.actor(author), kind: 'human', session: null };
  }

  // Projekt aus --project, sonst das Projekt des Boards dieses Repos bzw. des aktiven Boards
  private async projectKey(rt: BoardRuntime, option?: string): Promise<string> {
    if (option) return option;
    const boardId = await this.selection.one(rt, undefined, this.context.projectDir());
    const board = (await rt.services.boards.list()).find((b) => b.id === boardId);
    if (!board) throw new HarnessError('board-not-found', `Board ${boardId} gibt es nicht.`);
    return board.projectKey;
  }
}
