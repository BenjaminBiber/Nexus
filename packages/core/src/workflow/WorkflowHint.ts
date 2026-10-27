import { STATUS_LABELS, type Ticket } from '../board/BoardTypes.js';
import { WORKFLOW_PHASES, type WorkflowPhase } from './WorkflowPhases.js';

// Befehl, der das nächste bereite Ticket zeigt
const NEXT_READY = '`harness board list --status geplant --md`';
// Befehle zum Planen: nächstes Ticket mit Planungsbedarf holen bzw. Plan, Handbuch und Diagramm schreiben (setzt Geplant).
// Sie stehen hier und nicht in WORKFLOW_PHASES, weil dort nur Befehle für Claude Code (Slash-Befehle) stehen.
const NEXT_PLANNING = '`harness --json board next-planning --claim claude`';
const planJson = (id: string) => '`harness board plan-json ' + id + ' --file payload.json --by claude`';

// Baut den Hinweis auf den Ablauf für den Agenten, damit er dem Nutzer den nächsten Schritt vorschlagen kann.
export class WorkflowHint {
  // Ausnahme vom Ablauf für schnelle Aufgaben; gilt nur nach ausdrücklicher Bestätigung des Nutzers
  static readonly SIDE_TASKS =
    'Schnelle Aufgaben und Nebenaufgaben dürfen ohne den Ablauf (ohne Spec, Ticket und Worktree) erledigt werden, aber nur, wenn der Nutzer das ausdrücklich bestätigt: Wirkt eine Anfrage wie eine schnelle Aufgabe und hat der Nutzer nicht schon selbst „ohne Workflow“ gesagt, frage zuerst „Ohne Workflow direkt umsetzen?“ und arbeite erst nach einem klaren Ja so. Ohne diese Bestätigung folgst du dem Ablauf.';

  constructor(private readonly phases: WorkflowPhase[] = WORKFLOW_PHASES) {}

  // Ablauf plus Vorschlag zum aktuellen Stand der Tickets
  build(tickets: Array<Pick<Ticket, 'id' | 'status'>>): string {
    return [
      this.overview(),
      'Schlage nach einem erledigten Schritt den nächsten vor. Halte nur an, wenn eine Entscheidung des Nutzers nötig ist (unklare Anforderung, rote Tests, Merge, Push); sonst mache mit dem nächsten bereiten Ticket derselben Spec bzw. desselben Plans weiter, nie mit dem einer anderen.',
      WorkflowHint.SIDE_TASKS,
      this.state(tickets),
    ].join('\n');
  }

  // Eine Zeile mit allen Phasen und ihren Befehlen; eine Phase ohne Befehl steht nur mit Titel
  private overview(): string {
    const parts = this.phases.map((p) => (this.commands(p).length ? `${p.title}: ${this.commands(p).join(' → ')}` : p.title));
    return `Ablauf im Harness: ${parts.join('; ')}. Aus einem Superpowers-Plan legst du auf Wunsch je Task ein Ticket an und schreibst Plan, Handbuch und Diagramm mit ${planJson('<ID>')} (setzt Geplant; Skill board, „Tickets aus einem Superpowers-Plan“). Tickets mit Planungsbedarf (etwa aus der Ticket-Auswahl der Docs-Seite) werden vor dem Umsetzen geplant: ${NEXT_PLANNING} holt das nächste, ${planJson('<ID>')} schreibt Plan, Handbuch und Diagramm. Abgeschlossen wird mit \`harness board done\`, gemergt wird von Hand.`;
  }

  // Befehle einer Phase in Reihenfolge; Schritte ohne Befehl entfallen
  private commands(phase: WorkflowPhase): string[] {
    return phase.steps.flatMap((s) => (s.command ? [s.command] : []));
  }

  // Erster Befehl einer Phase, etwa der zum Planen oder Umsetzen
  private first(id: string): string {
    return this.phases.find((p) => p.id === id)?.steps.find((s) => s.command)?.command ?? '';
  }

  // Verweis auf die Phase für den Fall ohne Befehl; fehlt die Phase, entfällt er
  private see(id: string): string {
    const title = this.phases.find((p) => p.id === id)?.title;
    return title ? ` (siehe Phase „${title}“)` : '';
  }

  // „eine neue Funktion mit <Befehl> planen“; ohne Befehl mit Verweis auf die Phase
  private planning(): string {
    const command = this.first('planen');
    return command ? `eine neue Funktion mit ${command} planen` : `eine neue Funktion planen${this.see('planen')}`;
  }

  // „mit <Befehl> [<Ticket>] umsetzen“; ohne Befehl „<Ticket> umsetzen“ mit Verweis auf die Phase
  private implementing(ticket?: string): string {
    const command = this.first('umsetzen');
    if (command) return `mit ${command}${ticket ? ` ${ticket}` : ''} umsetzen`;
    return `${ticket ?? 'das Ticket'} umsetzen${this.see('umsetzen')}`;
  }

  // Ticket im Backlog oder In Planung: erst planen, dann umsetzen. In Planung gehört dem Nutzer (next-planning überspringt es)
  private planningTicket(id: string, withUser: boolean): string {
    const write = `Plan, Handbuch und Diagramm mit ${planJson(id)} schreiben (setzt Geplant)`;
    return withUser
      ? `den Plan mit dem Nutzer abschließen (In Planung gehört ihm), dann ${write}; erst danach ${this.implementing(id)}`
      : `vor dem Umsetzen planen: ${write}; erst danach ${this.implementing(id)}`;
  }

  // Vorschlag passend zum Stand des ersten offenen Tickets
  private state(tickets: Array<Pick<Ticket, 'id' | 'status'>>): string {
    if (tickets.length === 0) {
      return `Stand: keinem Ticket zugeordnet. Vorschlag: ${this.planning()} oder mit ${NEXT_READY} das nächste bereite Ticket suchen.`;
    }
    const open = tickets.find((t) => t.status !== 'done' && t.status !== 'not_needed');
    if (!open) {
      return `Stand: ${tickets[0].id} ist erledigt. Vorschlag: mit ${NEXT_READY} das nächste bereite Ticket derselben Spec bzw. desselben Plans suchen und ${this.implementing()}.`;
    }
    const next = open.status === 'in_progress'
      ? 'nach der Umsetzung Review und Abschluss mit `harness board done` vorschlagen'
      : open.status === 'blocked'
        ? 'den Grund für Blockiert klären, bevor weitergearbeitet wird'
        : open.status === 'backlog' || open.status === 'in_planning'
          ? this.planningTicket(open.id, open.status === 'in_planning')
          : this.implementing(open.id);
    return `Stand: ${open.id} ist ${STATUS_LABELS[open.status]}. Vorschlag: ${next}.`;
  }
}
