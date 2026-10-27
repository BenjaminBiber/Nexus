import {
  COLUMN_ORDER, STATUS_LABELS, WAITING_LABEL, noteTitle,
  type BoardColumn, type BoardNote, type BoardSummary, type BoardTicketDetail, type BoardTicketView,
} from '@harness/core';
import { CommentAuthorKind } from './CommentAuthorKind.js';

const NOTE_STATE_LABELS = { open: 'offen', taken: 'übernommen', dropped: 'verworfen' } as const;
// So viele Agenten-Kommentare bleiben in der kompakten Ausgabe stehen
const KEPT_AGENT_COMMENTS = 3;

/** Was `showText` zusätzlich ausgibt */
export interface ShowTextOptions { history?: boolean; allComments?: boolean }

// Textausgaben des Boards für Menschen (Text) und Agenten (Markdown).
export class BoardTextFormatter {
  private readonly authorKind = new CommentAuthorKind();

  // Spaltentitel
  columnLabel(column: BoardColumn): string {
    return column === 'waiting' ? WAITING_LABEL : STATUS_LABELS[column];
  }

  // Liste je Spalte: „  ARC-0007  Titel  [wartet auf …] [planung] @agent (high) ui,api“
  listText(tickets: BoardTicketView[]): string[] {
    if (!tickets.length) return ['(keine Tickets gefunden)'];
    const lines: string[] = [];
    for (const column of COLUMN_ORDER) {
      const inColumn = tickets.filter((t) => t.column === column);
      if (!inColumn.length) continue;
      lines.push('', `${this.columnLabel(column)} (${inColumn.length})`);
      for (const t of inColumn) {
        const marks = this.marks(t);
        lines.push(`  ${t.id}  ${t.title}${marks.length ? `  ${marks.join(' ')}` : ''}`);
      }
    }
    return lines;
  }

  // Markdown je Spalte mit Blockiert-Grund, Abnahme und offenen Voraussetzungen
  listMarkdown(heading: string, tickets: BoardTicketView[]): string[] {
    const lines = [`## ${heading}`];
    if (!tickets.length) return [...lines, '', '- (keine Tickets gefunden)'];
    for (const column of COLUMN_ORDER) {
      const inColumn = tickets.filter((t) => t.column === column);
      if (!inColumn.length) continue;
      lines.push('', `### ${this.columnLabel(column)} (${inColumn.length})`);
      for (const t of inColumn) {
        const meta = [t.needsPlanning ? '[planung]' : '', t.assignee ? `@${t.assignee}` : '', t.priority !== 'normal' ? t.priority : '', t.stepsTotal ? `${t.stepsDone}/${t.stepsTotal} Schritte` : '', t.labels.join(',')].filter(Boolean);
        lines.push(`- **${t.id}** ${t.title}${meta.length ? ` — ${meta.join(' · ')}` : ''}`);
        if (t.dependsOpen.length) lines.push(`  - Wartet auf: ${t.dependsOpen.join(', ')}`);
        if (t.waitingOnQuestions.length) lines.push(`  - Wartet auf Frage: ${t.waitingOnQuestions.map((q) => `${q.id} ${q.title}`).join('; ')}`);
        if (t.blockedReason) lines.push(`  - Blockiert: ${t.blockedReason}`);
        if (t.acceptance) lines.push(`  - Abnahme: ${t.acceptance}`);
      }
    }
    return lines;
  }

  // Ein Ticket mit allen Texten; Verlauf und ältere Agenten-Kommentare nur auf Wunsch
  showText(t: BoardTicketDetail, options: ShowTextOptions = {}): string[] {
    const head = [
      `Status ${t.status}${t.waiting ? ' (wartet)' : ''}`,
      `Prio ${t.priority}`,
      `rev ${t.rev}`,
      t.assignee ? `@${t.assignee}` : 'nicht zugewiesen',
      ...(t.needsPlanning ? ['braucht Planung'] : []),
      ...(t.labels.length ? [t.labels.join(',')] : []),
    ];
    const lines = [`${t.id}  ${t.title}`, head.join(' | ')];
    if (t.blockedReason) lines.push(`Blockiert: ${t.blockedReason}`);
    if (t.acceptance) lines.push(`Abnahme: ${t.acceptance}`);
    if (t.dependsOn.length) lines.push(`Setzt voraus: ${t.dependsOn.map((d) => `${d} (${t.dependsOpen.includes(d) ? 'offen' : 'erfüllt'})`).join(', ')}`);
    if (t.dependents.length) lines.push(`Voraussetzung für: ${t.dependents.join(', ')}`);
    if (t.waitingOnQuestions.length) lines.push(`Wartet auf Frage: ${t.waitingOnQuestions.map((q) => `${q.id} ${q.title}`).join('; ')}`);
    if (t.spec) lines.push(`Spec: ${t.spec}${t.specTask ? ` / ${t.specTask}` : ''}`);
    if (t.targetBranch) lines.push(`Zielbranch: ${t.targetBranch}`);
    if (t.branch) lines.push(`Branch: ${t.branch}${t.worktree ? ` (Worktree ${t.worktree})` : ''}`);
    if (t.touches.length) lines.push(`Bereiche: ${t.touches.join(', ')}`);
    if (t.consumes.length) lines.push(`Nutzt: ${t.consumes.join('; ')}`);
    if (t.produces.length) lines.push(`Liefert: ${t.produces.join('; ')}`);
    if (t.sessions.length) lines.push(`Sessions: ${t.sessions.join(', ')}`);
    const sections: Array<[string, string]> = [['Beschreibung', t.description], ['Plan', t.plan], ['Handbuch', t.handbook], ['Diagramm (mermaid)', t.diagram]];
    for (const [title, text] of sections) if (text.trim()) lines.push('', `## ${title}`, text.trimEnd());
    if (t.steps.length) {
      lines.push('', `## Schritte (${t.stepsDone}/${t.stepsTotal})`);
      t.steps.forEach((s, i) => {
        lines.push(`  [${s.done ? 'x' : ' '}] ${i + 1}. ${s.text}`);
        if (s.detail) lines.push(...s.detail.split('\n').map((l) => `        ${l}`));
      });
    }
    lines.push('', '## Kommentare', ...this.commentLines(t, options.allComments === true));
    if (options.history) {
      lines.push('', '## Verlauf');
      for (const h of t.history) lines.push(`  ${h.at} ${h.byName} ${h.field}: ${h.from ?? '–'} → ${h.to ?? '–'}${h.note ? ` (${h.note})` : ''}`);
    } else if (t.history.length) {
      lines.push('', `Verlauf: ${t.history.length} ${t.history.length === 1 ? 'Eintrag' : 'Einträge'} – --history zeigt sie`);
    }
    return lines;
  }

  // Kommentare; ohne `all` bleiben von den Agenten nur die letzten drei, Menschen immer
  private commentLines(t: BoardTicketDetail, all: boolean): string[] {
    if (!t.comments.length) return ['(keine)'];
    const agentIndexes = t.comments.map((c, i) => (this.authorKind.isAgent(c.authorName) ? i : -1)).filter((i) => i >= 0);
    const hidden = all ? [] : agentIndexes.slice(0, Math.max(0, agentIndexes.length - KEPT_AGENT_COMMENTS));
    const shown = t.comments.filter((_, i) => !hidden.includes(i));
    const note = hidden.length ? [`  ${hidden.length} ältere Agenten-Kommentare ausgeblendet – --all-comments zeigt sie`] : [];
    return [...note, ...shown.map((c) => `  ${c.createdAt} ${c.authorName}: ${c.body}`)];
  }

  // Boards mit Markierung des aktiven Boards und deinem Repo-Pfad
  boardsText(boards: BoardSummary[], active: number | null): string[] {
    if (!boards.length) return ['Keine Boards. Mit „harness board boards create <projekt> <name>“ anlegen.'];
    return boards.map((b) => `${b.id === active ? '*' : ' '} ${b.id}  ${b.projectKey}  ${b.name}  @${b.ownerName}  offen ${b.openTickets}${b.repoPath ? `  ${this.repoPathText(b)}` : ''}`);
  }

  // Eigener Repo-Pfad eines Boards; aus der DB übernommen (Altbestand) wird gekennzeichnet
  repoPathText(board: { repoPath: string | null; repoSource?: string | null }): string {
    if (!board.repoPath) return '(keiner)';
    return board.repoSource === 'legacy' ? `${board.repoPath} (aus der DB, Altbestand)` : board.repoPath;
  }

  // Notizen als Text
  notesText(notes: BoardNote[]): string[] {
    if (!notes.length) return ['(keine Notizen)'];
    return notes.map((n) => `  ${n.id}  [${NOTE_STATE_LABELS[n.state]}] ${noteTitle(n)}${n.seenCount > 1 ? ` [${n.seenCount}×]` : ''}${n.source ? ` (aus ${n.source})` : ''}${n.labels.length ? `  ${n.labels.join(',')}` : ''}`);
  }

  // Notizen als Markdown
  notesMarkdown(notes: BoardNote[]): string[] {
    if (!notes.length) return ['- (keine Notizen)'];
    return notes.map((n) => {
      const meta = [NOTE_STATE_LABELS[n.state], n.seenCount > 1 ? `${n.seenCount}× aufgefallen` : '', n.source ? `aus ${n.source}` : '', n.ticketId ? `→ ${n.ticketId}` : '', n.labels.join(',')].filter(Boolean);
      return `- **${n.id}** ${noteTitle(n)} — ${meta.join(' · ')}`;
    });
  }

  // Eine Notiz vollständig
  noteText(n: BoardNote): string[] {
    const lines = [`Notiz ${n.id}  [${NOTE_STATE_LABELS[n.state]}]  ${n.createdAt}  ${n.authorName}`];
    if (n.source) lines.push(`Aus: ${n.source}`);
    if (n.seenCount > 1) lines.push(`Aufgefallen: ${n.seenCount}×${n.seenIn.length ? ` (auch bei ${n.seenIn.join(', ')})` : ''}`);
    if (n.labels.length) lines.push(`Labels: ${n.labels.join(', ')}`);
    if (n.ticketId) lines.push(`Ticket: ${n.ticketId}`);
    if (n.reason) lines.push(`Grund: ${n.reason}`);
    // Ältere Notizen ohne Titel: der ganze Text steht im Inhalt
    const head = n.title ? ['', `# ${n.title}`] : [];
    return [...lines, ...head, ...(n.body.trim() ? ['', n.body.trimEnd()] : [])];
  }

  // Merkmale eines Tickets in der Textliste
  private marks(t: BoardTicketView): string[] {
    const marks: string[] = [];
    if (t.waiting) marks.push(`[wartet auf ${t.dependsOpen.join(', ')}]`);
    if (t.waitingOnQuestions.length) marks.push(`[Frage ${t.waitingOnQuestions.map((q) => q.id).join(', ')}]`);
    if (t.needsPlanning) marks.push('[planung]');
    if (t.assignee) marks.push(`@${t.assignee}`);
    if (t.priority !== 'normal') marks.push(`(${t.priority})`);
    if (t.stepsTotal) marks.push(`[${t.stepsDone}/${t.stepsTotal} Schritte]`);
    if (t.labels.length) marks.push(t.labels.join(','));
    return marks;
  }
}
