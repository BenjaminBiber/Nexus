import type { Board } from '../board/BoardTypes.js';
import type { TicketView } from '../board/TicketService.js';
import type { DispatchConfig } from './DispatchConfig.js';
import { ProfileChooser } from './ProfileChooser.js';

// Sagt, warum der Dispatcher ein Ticket (noch) nicht nimmt – oder null, wenn es bereit ist.
export class DispatchReadiness {
  private readonly chooser: ProfileChooser;

  constructor(private readonly cfg: DispatchConfig) {
    this.chooser = new ProfileChooser(cfg);
  }

  // Erster fehlender Punkt in fester Reihenfolge; repoExists prüft den Repo-Pfad des Boards
  reason(ticket: TicketView, board: Pick<Board, 'ownerId' | 'repoPath'>, userId: string, repoExists: (path: string) => boolean): string | null {
    if (ticket.status !== 'planned') return 'Status ist nicht Geplant';
    if (ticket.assignee) return `bereits ${ticket.assignee} zugewiesen`;
    if (ticket.needsPlanning) return 'Planungs-Flag ist noch gesetzt';
    if (this.cfg.optOutLabel && ticket.labels.includes(this.cfg.optOutLabel)) return `Label „${this.cfg.optOutLabel}“`;
    if (this.cfg.optInLabel && !ticket.labels.includes(this.cfg.optInLabel)) return `Label „${this.cfg.optInLabel}“ fehlt`;
    if (!ticket.plan.trim()) return 'Kein Plan';
    if (!ticket.handbook.trim()) return 'Kein Handbuch';
    if (!ticket.acceptance.trim()) return 'Keine Abnahme';
    if (ticket.dependsOpen.length) return `Wartet auf ${ticket.dependsOpen.join(', ')}`;
    // Erledigt reicht nicht: erst gemergt (kein Branch mehr) steckt der Code der Voraussetzung im neuen Branch
    if (ticket.dependsUnmerged.length) return `wartet auf Merge von ${ticket.dependsUnmerged.join(', ')}`;
    if (ticket.waitingOnQuestions.length) return `Wartet auf Frage ${ticket.waitingOnQuestions.map((q) => q.id).join(', ')}`;
    const labelProblem = this.chooser.labelProblem(ticket);
    if (labelProblem) return labelProblem;
    if (board.ownerId !== userId) return 'Board gehört nicht dir';
    if (!board.repoPath) return 'Board hat keinen Repo-Pfad (harness board boards repo <id> <pfad>)';
    if (!repoExists(board.repoPath)) return `Repo-Pfad ${board.repoPath} gibt es nicht`;
    return null;
  }
}
