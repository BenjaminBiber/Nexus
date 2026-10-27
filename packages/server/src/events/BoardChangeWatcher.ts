export interface DbBoardEvent {
  type: 'dbboard';
  boardId: number;
}

export interface DbQuestionsEvent {
  type: 'dbquestions';
  projectId: number;
}

type Listener<E> = (event: E) => void;

interface Entry<E> {
  listeners: Set<Listener<E>>;
  marker: string | null;
}

// Fragt für jede beobachtete ID (Board oder Projekt) mit offenen Ereignisströmen regelmäßig einen Änderungskennwert ab
// und meldet Änderungen; das Ereignis bildet eventOf (Standard: dbboard).
export class BoardChangeWatcher<E = DbBoardEvent> {
  private readonly boards = new Map<number, Entry<E>>();
  private timer: ReturnType<typeof setInterval> | null = null;
  private polling = false;

  constructor(
    private readonly markerOf: (id: number) => Promise<string>,
    private readonly intervalMs = 2500,
    private readonly eventOf: (id: number) => E = (id) => ({ type: 'dbboard', boardId: id }) as unknown as E,
  ) {}

  // Beobachtet eine ID; der Rückgabewert meldet den Empfänger wieder ab
  watch(id: number, listener: Listener<E>): () => void {
    let entry = this.boards.get(id);
    if (!entry) {
      entry = { listeners: new Set(), marker: null };
      this.boards.set(id, entry);
      void this.prime(entry, id);
    }
    entry.listeners.add(listener);
    this.ensureTimer();
    const joined = entry;
    return () => {
      joined.listeners.delete(listener);
      if (joined.listeners.size === 0 && this.boards.get(id) === joined) this.boards.delete(id);
      if (this.boards.size === 0) this.stopTimer();
    };
  }

  // Ein Durchlauf: Kennwerte abfragen und Änderungen melden (nie zwei Durchläufe gleichzeitig)
  async poll(): Promise<void> {
    if (this.polling) return;
    this.polling = true;
    try {
      for (const [id, entry] of [...this.boards]) {
        let marker: string;
        try {
          marker = await this.markerOf(id);
        } catch {
          continue;
        }
        if (entry.marker !== null && marker !== entry.marker) {
          const event = this.eventOf(id);
          entry.listeners.forEach((listener) => listener(event));
        }
        entry.marker = marker;
      }
    } finally {
      this.polling = false;
    }
  }

  // Beendet alle Beobachtungen
  close(): void {
    this.boards.clear();
    this.stopTimer();
  }

  // Ausgangswert beim ersten Beobachten (ohne Meldung); ein Fehler wartet auf den nächsten Durchlauf
  private async prime(entry: Entry<E>, id: number): Promise<void> {
    try {
      const marker = await this.markerOf(id);
      if (entry.marker === null) entry.marker = marker;
    } catch {
      // nächster Durchlauf setzt den Ausgangswert
    }
  }

  // Startet den Takt, falls er noch nicht läuft
  private ensureTimer(): void {
    if (this.timer) return;
    this.timer = setInterval(() => void this.poll(), this.intervalMs);
    this.timer.unref?.();
  }

  // Hält den Takt an
  private stopTimer(): void {
    if (this.timer) clearInterval(this.timer);
    this.timer = null;
  }
}
