// Die Dokumente eines Projekts haben sich geändert; projectId ist die pid aus der Route.
export interface SpecDocsEvent {
  type: 'docs-changed';
  projectId: string;
}

type Listener = (event: SpecDocsEvent) => void;

interface Entry {
  listeners: Set<Listener>;
  signature: string | null;
}

// Fragt für jedes Projekt mit offenem Ereignisstrom regelmäßig die Signatur seiner Superpowers-Dokumente ab
// und meldet Änderungen als docs-changed.
export class SpecDocWatcher {
  private readonly projects = new Map<string, Entry>();
  private timer: ReturnType<typeof setInterval> | null = null;
  private polling = false;

  constructor(
    private readonly signatureOf: (pid: string) => Promise<string>,
    private readonly intervalMs = 10000,
  ) {}

  // Beobachtet ein Projekt; der Rückgabewert meldet den Empfänger wieder ab
  watch(pid: string, listener: Listener): () => void {
    let entry = this.projects.get(pid);
    if (!entry) {
      entry = { listeners: new Set(), signature: null };
      this.projects.set(pid, entry);
      void this.prime(entry, pid);
    }
    entry.listeners.add(listener);
    this.ensureTimer();
    const joined = entry;
    return () => {
      joined.listeners.delete(listener);
      if (joined.listeners.size === 0 && this.projects.get(pid) === joined) this.projects.delete(pid);
      if (this.projects.size === 0) this.stopTimer();
    };
  }

  // Ein Durchlauf: Signaturen abfragen und Änderungen melden (nie zwei Durchläufe gleichzeitig)
  async poll(): Promise<void> {
    if (this.polling) return;
    this.polling = true;
    try {
      for (const [pid, entry] of [...this.projects]) {
        let signature: string;
        try {
          signature = await this.signatureOf(pid);
        } catch {
          continue;
        }
        if (entry.signature !== null && signature !== entry.signature) {
          const event: SpecDocsEvent = { type: 'docs-changed', projectId: pid };
          entry.listeners.forEach((listener) => listener(event));
        }
        entry.signature = signature;
      }
    } finally {
      this.polling = false;
    }
  }

  // Beendet alle Beobachtungen
  close(): void {
    this.projects.clear();
    this.stopTimer();
  }

  // Ausgangswert beim ersten Beobachten (ohne Meldung); ein Fehler wartet auf den nächsten Durchlauf
  private async prime(entry: Entry, pid: string): Promise<void> {
    try {
      const signature = await this.signatureOf(pid);
      if (entry.signature === null) entry.signature = signature;
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
