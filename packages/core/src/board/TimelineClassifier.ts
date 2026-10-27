import type { AuthorKind } from '../actors/Actor.js';
import { DISPATCH_MARKERS } from '../dispatch/DispatchConfig.js';
import type { Comment, HistoryEntry } from './BoardTypes.js';
import type { TimelineActor, TimelineAgentNames, TimelineEvent, TimelineKind } from './TicketTimeline.js';

// Art je Feld des Verlaufs; nicht genannte Felder sind „other“
const FIELD_KINDS: Record<string, TimelineKind> = {
  status: 'status', blockedReason: 'status',
  assignee: 'claim', planningClaimBy: 'claim',
  question: 'question',
  branch: 'git', worktree: 'git', targetBranch: 'git',
  title: 'content', description: 'content', plan: 'content', handbook: 'content', diagram: 'content',
  acceptance: 'content', step: 'content', steps: 'content',
};

// Dispatcher-Kommentare: Marker → Art und Typ; spezielle Marker vor ihren Vorsilben („Merge-Konflikt“ vor „Merge“)
const DISPATCH_KINDS: Array<[string, TimelineKind, string]> = [
  [DISPATCH_MARKERS.START, 'run', 'dispatch:start'],
  [DISPATCH_MARKERS.ABORT, 'run', 'dispatch:abort'],
  [DISPATCH_MARKERS.PAUSE, 'run', 'dispatch:pause'],
  [DISPATCH_MARKERS.QUESTION, 'run', 'dispatch:question'],
  [DISPATCH_MARKERS.SETUP, 'run', 'dispatch:setup'],
  [DISPATCH_MARKERS.REVIEW_RETURN, 'run', 'dispatch:review-return'],
  [DISPATCH_MARKERS.REVIEW, 'run', 'dispatch:review'],
  [DISPATCH_MARKERS.MERGED, 'git', 'merge'],
  [DISPATCH_MARKERS.MERGE_CONFLICT, 'git', 'dispatch:merge-conflict'],
  [DISPATCH_MARKERS.MERGE_WAIT, 'git', 'dispatch:merge-wait'],
  [DISPATCH_MARKERS.MERGE, 'git', 'dispatch:merge'],
];

const QUESTION_ID = /\b[A-Z][A-Z0-9]*-F\d+\b/;

// Ordnet Verlaufseinträge und Kommentare einer Art der Zeitleiste zu und schätzt, ob ein Mensch oder ein Agent handelte.
export class TimelineClassifier {
  // names null = keine Dispatch-Konfiguration; dann zählen nur „claude“ und die Art des Autors
  constructor(private readonly names: TimelineAgentNames | null) {}

  // Agent, wenn die Art des Autors es sagt (Fragen) oder der Name nach Agent aussieht
  actor(name: string, kind?: AuthorKind | null): TimelineActor {
    if (kind) return { name, type: kind === 'human' ? 'person' : 'agent' };
    return { name, type: this.isAgentName(name) ? 'agent' : 'person' };
  }

  // Ereignis zu einem Verlaufseintrag des Tickets
  history(h: HistoryEntry): TimelineEvent {
    const event: TimelineEvent = {
      at: h.at, kind: FIELD_KINDS[h.field] ?? 'other', type: `field:${h.field}`, actor: this.actor(h.byName),
      field: h.field, from: h.from, to: h.to, note: h.note,
    };
    const question = h.field === 'question' ? QUESTION_ID.exec(h.note ?? '')?.[0] : undefined;
    if (question) event.ref = { questionId: question };
    return event;
  }

  // Ereignis zu einem Kommentar; Dispatcher-Kommentare werden an ihrem Marker erkannt
  comment(c: Comment): TimelineEvent {
    const base = { at: c.createdAt, actor: this.actor(c.authorName), text: c.body };
    const match = DISPATCH_KINDS.find(([marker]) => c.body.startsWith(marker));
    if (!match) return { ...base, kind: 'comment', type: 'comment' };
    const [, kind, type] = match;
    const event: TimelineEvent = { ...base, kind, type };
    if (type === 'dispatch:start') {
      const profile = /Profil ([^\s(,]+)/.exec(c.body)?.[1];
      if (profile) event.ref = { profile };
    }
    if (type === 'merge') {
      const commit = /^\[dispatch\] Gemergt in .+? \(([^)]+)\)\./.exec(c.body)?.[1];
      if (commit) event.ref = { commit };
    }
    return event;
  }

  private isAgentName(name: string): boolean {
    const lower = name.trim().toLowerCase();
    if (lower === 'claude' || lower.startsWith('claude-')) return true;
    if (!this.names) return false;
    return name === this.names.dispatcherName || (this.names.agentPrefix !== '' && name.startsWith(this.names.agentPrefix));
  }
}
