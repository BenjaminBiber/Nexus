import { isHumanInteraction, type Comment, type HistoryEntry, type Ticket } from '../board/BoardTypes.js';
import type { DispatchRun, DispatchRunOutcome } from '../board/DispatchRunTypes.js';
import { AbortReasonClassifier } from './AbortReasonClassifier.js';
import { DISPATCH_MARKERS } from './DispatchConfig.js';

// „[dispatch] Start: an <agent> übergeben – Branch …, Profil <p> (Modell <m>, Effort <e>, max. n Schritte, n min). Grund: <why>.“
const START = /^\[dispatch\] Start: an (\S+) übergeben – Branch .*?, Profil (\S+) \(Modell ([^,]+), Effort ([^,]+), max\. \d+ Schritte, \d+ min\)\. Grund: ([^\n]*?)\.?(?:\n|$)/;
// Basisprofil am Anfang der Begründung (ProfileChooser.choose)
const BASE = /^(?:Standardprofil (\S+?)|Label profil:(\S+?)|Aufwand laut Plan: (\S+?)|Regel \d+ → (\S+?))(?:,| |$)/;
const ESCALATED = /Versuch (\d+): eskaliert auf (\S+?)(?:,|$)/;
const ABORT = /^\[dispatch\] Abbruch Versuch (\d+)\/\d+: ([^\n]*?)\.?(?:\n|$)/;

export interface BackfillInput {
  projectId: number;
  /** Kommentare „[dispatch] …“ der Tickets des Projekts, nach ID */
  comments: Comment[];
  /** Statuswechsel und Blockiert-Gründe dieser Tickets */
  history: HistoryEntry[];
  tickets: Array<Pick<Ticket, 'id' | 'boardId' | 'status'>>;
}

interface Event {
  at: string;
  outcome: DispatchRunOutcome;
  reason?: string;
  /** Blockiert-Grund laut Verlauf */
  blocked?: string;
}

// Rekonstruiert Läufe aus den [dispatch]-Kommentaren (für die Zeit vor der Erfassung in board.DispatchRuns): je Start-Kommentar
// ein Lauf mit Profil, Modell-Alias, Effort, Versuch und Eskalation; das Ergebnis aus dem nächsten Dispatcher-Kommentar bzw.
// Statuswechsel. Kosten und tatsächliches Modell sind nicht bekannt.
export class DispatchRunBackfill {
  private readonly classifier = new AbortReasonClassifier();

  runs(input: BackfillInput): DispatchRun[] {
    const tickets = new Map(input.tickets.map((t) => [t.id, t]));
    const out: DispatchRun[] = [];
    const byTicket = new Map<string, Comment[]>();
    for (const c of input.comments) (byTicket.get(c.ticketId) ?? byTicket.set(c.ticketId, []).get(c.ticketId)!).push(c);
    for (const [ticketId, comments] of byTicket) {
      const ticket = tickets.get(ticketId);
      const history = input.history.filter((h) => h.ticketId === ticketId).sort((a, b) => a.at.localeCompare(b.at));
      const sorted = [...comments].sort((a, b) => a.id - b.id);
      sorted.forEach((c, i) => {
        const start = START.exec(c.body);
        if (!start) return;
        const nextStart = sorted.slice(i + 1).find((n) => n.body.startsWith(DISPATCH_MARKERS.START));
        const until = nextStart?.createdAt ?? null;
        const end = this.end(sorted.slice(i + 1).filter((n) => !nextStart || n.id < nextStart.id), history, c.createdAt, until);
        // Letzter Start ohne Ergebnis an einem Ticket in Arbeit: der Lauf läuft vermutlich noch
        if (!end && !nextStart && ticket?.status === 'in_progress') return;
        const [, agent, profile, model, effort, why] = start;
        const escalated = ESCALATED.exec(why);
        const base = BASE.exec(why);
        const aborts = sorted.slice(0, i).filter((p) => p.body.startsWith(DISPATCH_MARKERS.ABORT)).length;
        const event = end ?? { at: until ?? c.createdAt, outcome: 'other' as const };
        out.push({
          id: `c${c.id}`, kind: 'work', subjectId: ticketId, projectId: input.projectId, boardId: ticket?.boardId ?? null, agent,
          attempt: aborts + 1, profile, escalatedFrom: escalated ? (base?.slice(1).find(Boolean) ?? null) : null,
          requestedModel: model === 'Standard' ? null : model, model: null, effort: effort === 'Standard' ? null : effort,
          startedAt: c.createdAt, endedAt: event.at, durationMs: Math.max(0, Date.parse(event.at) - Date.parse(c.createdAt)),
          numTurns: null, exitCode: null, timedOut: false, outcome: event.outcome,
          abortCategory: event.reason === undefined ? null : this.classifier.category(event.reason),
          abortReason: (event.reason ?? event.blocked)?.slice(0, 500) ?? null,
          costUsd: null, inputTokens: null, outputTokens: null, cacheReadTokens: null, cacheWriteTokens: null, modelUsage: null, source: 'comments',
          // Rekonstruierte Läufe kennen weder Limits noch Konfig-Version oder abgelehnte Werkzeuge
          maxTurns: null, timeoutMinutes: null, profileReason: null, ruleIndex: null, configHash: null, deniedCount: null, deniedTools: null,
        });
      });
    }
    return out.sort((a, b) => a.startedAt.localeCompare(b.startedAt) || a.id.localeCompare(b.id));
  }

  // Ergebnis eines Laufs: erster aussagekräftiger Dispatcher-Kommentar, sonst der erste Statuswechsel weg von In Arbeit
  private end(after: Comment[], history: HistoryEntry[], from: string, until: string | null): Event | null {
    for (const c of after) {
      const abort = ABORT.exec(c.body);
      if (abort) return { at: c.createdAt, outcome: /letzte automatische Versuch/.test(c.body) ? 'abort-final' : 'abort', reason: abort[2] };
      if (c.body.startsWith(`${DISPATCH_MARKERS.PAUSE}: Nutzungslimit`)) return { at: c.createdAt, outcome: 'pause-limit' };
      if (c.body.startsWith(`${DISPATCH_MARKERS.PAUSE}: Claude Code wurde nicht gefunden`)) return { at: c.createdAt, outcome: 'pause-claude-missing' };
      if (c.body.startsWith(DISPATCH_MARKERS.QUESTION)) return { at: c.createdAt, outcome: 'question' };
      if (c.body.startsWith(DISPATCH_MARKERS.SETUP)) return { at: c.createdAt, outcome: 'setup-failed' };
      if (c.body.startsWith(`${DISPATCH_MARKERS.REVIEW}:`)) return { at: c.createdAt, outcome: 'review' };
    }
    const change = history.find((h) => h.field === 'status' && h.at >= from && (until === null || h.at <= until) && h.to !== 'in_progress');
    if (!change) return null;
    if (change.to === 'done') return { at: change.at, outcome: 'done' };
    if (change.to === 'blocked') {
      // Der Grund steht im selben Verlaufsschritt; „Human Interaction: …“ ist fertig, nur vom Menschen abzunehmen
      const reason = history.find((h) => h.field === 'blockedReason' && h.at === change.at)?.to ?? undefined;
      return { at: change.at, outcome: isHumanInteraction(reason) ? 'human' : 'blocked', ...(reason ? { blocked: reason } : {}) };
    }
    return { at: change.at, outcome: 'other' };
  }
}
