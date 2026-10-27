import { mkdir, readFile, rename } from 'node:fs/promises';
import { dirname } from 'node:path';
import { AtomicWriter } from '../../locking/AtomicWriter.js';
import type { HarnessHome } from '../../registry/HarnessHome.js';

export type MergePhase = 'pending' | 'waiting' | 'session';

// Merge-Auftrag eines erledigten Tickets
export interface MergeEntry {
  ticketId: string;
  branch: string;
  phase: MergePhase;
  /** true, sobald für diesen Merge-Versuch eine Merge-Session gestartet wurde */
  sessionUsed: boolean;
  /** „Merge wartet auf …“ wurde schon kommentiert */
  waitNoted: boolean;
  /** Ziel-Branch der Merge-Session */
  target: string | null;
  /** Der letzte Merge-Versuch hatte einen Konflikt, die Merge-Session durfte aber nicht starten (aus, Pause, außerhalb);
   *  bis sie starten darf, wird nicht erneut gemergt. „Erneut mergen“ setzt den Merker zurück. */
  conflictSeen?: boolean;
  /** Art des zuletzt kommentierten Grundes, warum der Merge auf seinen Ordner wartet: 'busy' (laufende Git-Aktion), 'staged'
   *  (vorgemerkte), 'overlap' (überschneidende lokale Änderungen) oder 'leftover' (Reste im Merge-Worktree); nur eine neue Art wird erneut kommentiert.
   *  „Erneut mergen“ setzt sie zurück. */
  heldBy?: string | null;
  since: string;
}

// Recherche-Läufe einer Frage
export interface ResearchEntry {
  questionId: string;
  lastStartedAt: string;
  /** Zahl der gestarteten Recherche-Läufe (von Hand und automatisch) */
  count: number;
  /** Zeitpunkt des automatischen Anstoßes; die Automatik stößt jede Frage nur einmal an */
  autoAt: string | null;
}

// Zustand der Recherche-Automatik
export interface ResearchAutoState {
  /** Seit wann die Automatik an ist; nur Fragen, die danach entstanden sind, kommen in Frage (null = aus) */
  since: string | null;
  /** Kalendertag (Ortszeit, JJJJ-MM-TT) des Tageszählers */
  day: string | null;
  /** Automatisch gestartete Recherchen an diesem Tag */
  count: number;
  /** Nach einem Umgebungs- oder Repo-Fehler: frühester Neuversuch */
  retryAt: string | null;
  /** Letzter Umgebungs- oder Repo-Fehler (wird je Text nur einmal protokolliert) */
  lastError: string | null;
}

// Planungsläufe eines Tickets
export interface PlanningEntry {
  ticketId: string;
  lastStartedAt: string;
  /** Zahl der gestarteten Planungsläufe (von Hand und automatisch) */
  count: number;
  /** Zeitpunkt des automatischen Anstoßes; die Automatik stößt jedes Ticket nur einmal an */
  autoAt: string | null;
}

// Zustand der Planungs-Automatik
export interface PlanningAutoState {
  /** Seit wann die Automatik an ist; nur Tickets, die danach dazugekommen sind, kommen in Frage (null = aus) */
  since: string | null;
  /** Nach einem Umgebungs- oder Repo-Fehler: frühester Neuversuch */
  retryAt: string | null;
  /** Letzter Umgebungs- oder Repo-Fehler (wird je Text nur einmal protokolliert) */
  lastError: string | null;
}

export interface DispatchState {
  pausedUntil: string | null;
  merges: Record<string, MergeEntry>;
  research: Record<string, ResearchEntry>;
  researchAuto: ResearchAutoState;
  planning: Record<string, PlanningEntry>;
  planningAuto: PlanningAutoState;
}

// Pause, Merge-Aufträge und Recherchen in ~/.harness/dispatch/state.json; Änderungen laufen nacheinander.
export class DispatchStateStore {
  private readonly writer = new AtomicWriter();
  private queue: Promise<unknown> = Promise.resolve();

  constructor(private readonly home: HarnessHome) {}

  // Leerer Zustand (ohne Datei oder nach kaputtem JSON)
  static empty(): DispatchState {
    return {
      pausedUntil: null, merges: {}, research: {}, researchAuto: { since: null, day: null, count: 0, retryAt: null, lastError: null },
      planning: {}, planningAuto: { since: null, retryAt: null, lastError: null },
    };
  }

  // Aktueller Zustand; ohne Datei leer. Kaputtes JSON wird nach state.json.corrupt-<Zeitstempel> verschoben
  // und gilt als leer; andere Lesefehler werfen
  async read(): Promise<DispatchState> {
    const file = this.home.dispatchStateFile;
    let text: string;
    try {
      text = await readFile(file, 'utf8');
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code === 'ENOENT') return DispatchStateStore.empty();
      throw error;
    }
    try {
      const raw = JSON.parse(text) as Partial<DispatchState> | null;
      return {
        pausedUntil: raw?.pausedUntil ?? null, merges: raw?.merges ?? {}, research: raw?.research ?? {},
        researchAuto: { ...DispatchStateStore.empty().researchAuto, ...(raw?.researchAuto ?? {}) },
        planning: raw?.planning ?? {},
        planningAuto: { ...DispatchStateStore.empty().planningAuto, ...(raw?.planningAuto ?? {}) },
      };
    } catch {
      await rename(file, `${file}.corrupt-${new Date().toISOString().replace(/[:.]/g, '-')}`).catch(() => undefined);
      return DispatchStateStore.empty();
    }
  }

  // Liest, ändert und schreibt in einem Zug; Aufrufe warten aufeinander
  update(mutate: (state: DispatchState) => void): Promise<DispatchState> {
    const next = this.queue.then(async () => {
      const state = await this.read();
      mutate(state);
      await mkdir(dirname(this.home.dispatchStateFile), { recursive: true });
      await this.writer.write(this.home.dispatchStateFile, JSON.stringify(state, null, 2));
      return state;
    });
    this.queue = next.catch(() => undefined);
    return next;
  }
}
