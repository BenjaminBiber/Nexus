import { HarnessError } from '../errors/HarnessError.js';
import type { BoardDatabase, BoardUnitOfWork } from './BoardDatabase.js';
import {
  MAX_NOTE_LABELS_LENGTH, MAX_NOTE_REASON_LENGTH, MAX_NOTE_SEEN_IN_LENGTH, MAX_NOTE_TITLE_LENGTH,
  type BoardActor, type Note, type NoteState, type Priority, type TicketStatus,
} from './BoardTypes.js';
import { BoardLabels } from './BoardLabels.js';
import { BoardLimits } from './BoardLimits.js';
import { BoardService } from './BoardService.js';
import { NoteSimilarity, noteTitle } from './NoteSimilarity.js';
import { TicketService, type TicketView } from './TicketService.js';

/** Inhalt einer neuen Notiz: ein Titel, der ohne Kontext verständlich ist, und optionale Details */
export interface NewNoteText {
  title: string;
  body?: string;
}

// Notizen: Vorrat für spätere Tickets. Werden nie gelöscht, nur übernommen oder mit Grund verworfen.
export class NoteService {
  private readonly tickets: TicketService;
  private readonly boards: BoardService;
  private readonly similarity = new NoteSimilarity();

  constructor(private readonly db: BoardDatabase, private readonly clock: () => Date) {
    this.tickets = new TicketService(db, clock);
    this.boards = new BoardService(db, clock);
  }

  // Legt eine offene Notiz an; source muss ein existierendes Ticket aus dem Projekt des Boards sein.
  // Gibt es den Befund vermutlich schon (offen oder übernommen), wird abgelehnt – außer mit force.
  add(boardId: number, text: NewNoteText, actor: BoardActor, options: { source?: string; labels?: string[]; force?: boolean } = {}): Promise<Note> {
    const title = text.title.trim();
    const body = (text.body ?? '').trim();
    if (!title) return Promise.reject(new HarnessError('note-empty', 'Die Notiz braucht einen Titel: einen Satz, was auffällt.'));
    if (title.includes('\n')) {
      return Promise.reject(new HarnessError('note-title-multiline', 'Der Titel der Notiz ist eine Zeile; Details gehören in den Inhalt (--details).'));
    }
    if (title.length > MAX_NOTE_TITLE_LENGTH) {
      return Promise.reject(new HarnessError('value-too-long', `Der Titel der Notiz ist zu lang (${title.length} Zeichen, höchstens ${MAX_NOTE_TITLE_LENGTH}). Ein kurzer Satz als Titel, der Rest in den Inhalt (--details).`));
    }
    return this.db.transaction(async (u) => {
      BoardLimits.actor(actor);
      const labels = BoardLabels.normalize(options.labels ?? []);
      BoardLimits.text(labels.join(','), MAX_NOTE_LABELS_LENGTH, 'Die Label-Liste der Notiz');
      const board = await this.boards.get(u, boardId);
      const origin = options.source ? await this.tickets.load(u, options.source) : null;
      if (origin && origin.projectId !== board.projectId) {
        throw new HarnessError('note-source-other-project', `${origin.id} gehört zu einem anderen Projekt als Board ${boardId}. Die Notiz gehört auf das Board des Tickets (--board ${origin.boardId}).`);
      }
      if (!options.force) {
        const known = this.similarity.similar(`${title}\n${body}`, await u.notes.list({ boardId, states: ['open', 'taken'] }));
        if (known.length) throw this.duplicate(known.slice(0, 3), origin?.id ?? null);
      }
      return u.notes.insert({
        boardId, authorId: actor.userId, authorName: actor.name, title, body, source: origin?.id ?? null, labels,
        seenCount: 1, seenIn: [], state: 'open', ticketId: null, reason: null, createdAt: this.clock().toISOString(),
      });
    });
  }

  // Offene und übernommene Notizen des Boards, die denselben Befund beschreiben wie der Text (ähnlichste zuerst)
  similar(boardId: number, text: string): Promise<Note[]> {
    return this.db.transaction(async (u) => this.similarity.similar(text, await u.notes.list({ boardId, states: ['open', 'taken'] })));
  }

  // Der Befund einer offenen Notiz ist erneut aufgefallen: zählt hoch und merkt sich das Ticket
  seen(id: number, options: { source?: string } = {}): Promise<Note> {
    return this.db.transaction(async (u) => {
      const note = await this.load(u, id, true);
      if (note.state === 'taken') throw new HarnessError('note-taken', `Notiz ${id} ist schon als ${note.ticketId} übernommen – dort kommentieren.`);
      if (note.state === 'dropped') throw new HarnessError('note-not-open', `Notiz ${id} ist verworfen (${note.reason}).`);
      const board = await this.boards.get(u, note.boardId);
      const origin = options.source ? await this.tickets.load(u, options.source) : null;
      if (origin && origin.projectId !== board.projectId) {
        throw new HarnessError('note-source-other-project', `${origin.id} gehört zu einem anderen Projekt als Notiz ${id}.`);
      }
      const seenIn = origin && origin.id !== note.source && !note.seenIn.includes(origin.id) ? [...note.seenIn, origin.id] : note.seenIn;
      const updated: Note = {
        ...note,
        seenCount: note.seenCount + 1,
        // Passt die Liste nicht mehr in die Spalte, zählt nur noch die Zahl
        seenIn: seenIn.join(',').length <= MAX_NOTE_SEEN_IN_LENGTH ? seenIn : note.seenIn,
      };
      await u.notes.update(updated);
      return updated;
    });
  }

  // Notizen eines Boards; ohne Angabe nur offene
  list(boardId: number, options: { states?: NoteState[]; source?: string } = {}): Promise<Note[]> {
    return this.db.transaction((u) => u.notes.list({ boardId, states: options.states ?? ['open'], source: options.source }));
  }

  // Eine Notiz
  get(id: number): Promise<Note> {
    return this.db.transaction((u) => this.load(u, id));
  }

  // Macht aus einer offenen Notiz ein Ticket auf demselben Board
  take(id: number, actor: BoardActor, options: { title?: string; status?: TicketStatus; priority?: Priority; labels?: string[] } = {}): Promise<{ note: Note; ticket: TicketView }> {
    return this.db.transaction(async (u) => {
      const note = await this.load(u, id, true);
      if (note.state !== 'open') {
        const detail = note.state === 'taken' ? `schon als ${note.ticketId} übernommen` : 'verworfen';
        throw new HarnessError('note-not-open', `Notiz ${id} ist ${detail}.`);
      }
      const ticket = await this.tickets.createIn(u, note.boardId, {
        title: options.title ?? noteTitle(note).slice(0, MAX_NOTE_TITLE_LENGTH),
        description: this.description(note),
        status: options.status ?? 'backlog',
        priority: options.priority ?? 'normal',
        labels: options.labels ?? note.labels,
      }, actor);
      const taken: Note = { ...note, state: 'taken', ticketId: ticket.id };
      await u.notes.update(taken);
      return { note: taken, ticket: await this.tickets.view(u, ticket) };
    });
  }

  // Verwirft eine Notiz mit Grund; übernommene Notizen lassen sich nicht verwerfen
  drop(id: number, reason: string): Promise<Note> {
    const why = reason.trim();
    if (!why) return Promise.reject(new HarnessError('reason-missing', 'Zum Verwerfen gehört ein Grund (--reason).'));
    if (why.length > MAX_NOTE_REASON_LENGTH) {
      return Promise.reject(new HarnessError('value-too-long', `Der Grund ist zu lang (${why.length} Zeichen, höchstens ${MAX_NOTE_REASON_LENGTH}).`));
    }
    return this.db.transaction(async (u) => {
      const note = await this.load(u, id, true);
      if (note.state === 'taken') throw new HarnessError('note-taken', `Notiz ${id} ist schon als ${note.ticketId} übernommen.`);
      const dropped: Note = { ...note, state: 'dropped', reason: why, ticketId: null };
      await u.notes.update(dropped);
      return dropped;
    });
  }

  // Beschreibung des Tickets aus einer Notiz: Inhalt (bei älteren Notizen der ganze Text) und woher sie kommt
  private description(note: Note): string {
    const origin = note.source ? `, aufgefallen bei ${note.source}` : '';
    const again = note.seenCount > 1 ? `; insgesamt ${note.seenCount}× aufgefallen${note.seenIn.length ? `, auch bei ${note.seenIn.join(', ')}` : ''}` : '';
    const footer = `(Notiz vom ${note.createdAt}${origin}${again})`;
    return note.body ? `${note.body}\n\n${footer}` : footer;
  }

  // Ablehnung „gibt es schon“ mit den Kandidaten und dem Weg, sie zu zählen
  private duplicate(known: Note[], source: string | null): HarnessError {
    const lines = known.map((n) => `  ${n.id}  ${n.state === 'taken' ? `[übernommen als ${n.ticketId}]` : '[offen]'} ${noteTitle(n)}`);
    const open = known.find((n) => n.state === 'open');
    const taken = known.find((n) => n.state === 'taken');
    const hints = [
      open ? `Derselbe Befund: harness board note seen ${open.id}${source ? ` --from ${source}` : ''}` : null,
      taken && !open ? `Schon als Ticket übernommen: an ${taken.ticketId} kommentieren statt neu notieren.` : null,
      'Wirklich etwas anderes: mit --force anlegen.',
    ].filter(Boolean);
    return new HarnessError(
      'note-similar',
      ['Diesen Befund gibt es vermutlich schon:', ...lines, ...hints].join('\n'),
      { notes: known.map((n) => ({ id: n.id, title: noteTitle(n), state: n.state, ticketId: n.ticketId })) },
    );
  }

  // Notiz zur ID oder Fehler; forUpdate sperrt sie bis zum Ende der Transaktion
  private async load(u: BoardUnitOfWork, id: number, forUpdate = false): Promise<Note> {
    const note = await u.notes.byId(id, forUpdate);
    if (!note) throw new HarnessError('note-not-found', `Notiz ${id} gibt es nicht.`);
    return note;
  }
}
