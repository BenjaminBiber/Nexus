import { createContext, useContext, useState, useSyncExternalStore, type ReactNode } from 'react';
import { useApi } from '../api/ApiContext.js';
import type { ApiClient } from '../api/ApiClient.js';
import type { DocTicketResultDto, TicketDraftDto, TicketDraftResultDto } from '../api/types.js';

/** Dokument, zu dem Entwürfe erzeugt werden */
export interface DocTarget {
  pid: string;
  path: string;
  board?: number;
}

/** Stand einer Ticket-Erzeugung zu einem Dokument */
export interface DocTicketJob extends DocTarget {
  key: string;
  /** Nummer des Laufs; eine Antwort eines abgebrochenen oder ersetzten Laufs fällt weg */
  run: number;
  running: boolean;
  /** Entwürfe im offenen Dialog; null, solange er zu ist */
  result: TicketDraftResultDto | null;
  /** Gespeicherte Entwürfe der Datei (Server), die sich mit „Entwürfe öffnen“ wieder öffnen lassen */
  saved: TicketDraftResultDto | null;
  creating: boolean;
  created: DocTicketResultDto | null;
  error: string | null;
}

export const jobKey = (t: DocTarget) => `${t.pid}|${t.board ?? ''}|${t.path}`;

// Ticket-Erzeugungen aller Dokumente außerhalb der Seiten: Ein Lauf überlebt den Wechsel auf eine andere Seite,
// und beim Zurückkommen stehen Ladeanzeige, Entwürfe oder Fehler wieder am Dokument.
export class DocTicketJobs {
  private jobs: DocTicketJob[] = [];
  private listeners = new Set<() => void>();
  private runs = 0;

  constructor(private readonly api: ApiClient) {}

  subscribe = (listener: () => void) => {
    this.listeners.add(listener);
    return () => void this.listeners.delete(listener);
  };

  list = () => this.jobs;

  // Ändert den Lauf eines Dokuments (legt ihn bei Bedarf an) und meldet die neue Liste
  private patch(target: DocTarget, change: Partial<DocTicketJob>) {
    const key = jobKey(target);
    const old = this.jobs.find((j) => j.key === key)
      ?? { ...target, key, run: 0, running: false, result: null, saved: null, creating: false, created: null, error: null };
    this.jobs = [...this.jobs.filter((j) => j.key !== key), { ...old, ...change }];
    this.listeners.forEach((l) => l());
  }

  // Lässt Claude das ganze Dokument in Entwürfe zerlegen. Die Antwort zählt nur, solange der Lauf
  // noch derselbe ist: nach „Abbrechen“ (oder einem neuen Lauf) fällt sie weg.
  async request(target: DocTarget): Promise<void> {
    const run = ++this.runs;
    this.patch(target, { run, running: true, result: null, created: null, error: null });
    const current = () => this.jobs.find((j) => j.key === jobKey(target))?.run === run;
    try {
      const result = await this.api.createTicketDrafts(target.pid, { path: target.path, selections: [], note: '', whole: true }, target.board);
      if (current()) this.patch(target, { running: false, result, saved: result });
    } catch (e) {
      if (current()) this.patch(target, { running: false, error: (e as Error).message });
    }
  }

  // Holt die gespeicherten Entwürfe der Datei (beim Öffnen des Dokuments); ein laufender oder offener Lauf bleibt unberührt
  async load(target: DocTarget): Promise<void> {
    try {
      const { result } = await this.api.getSavedTicketDrafts(target.pid, target.path, target.board);
      const job = this.find(target);
      if (job?.running || job?.result) return;
      if (result || job) this.patch(target, { saved: result && result.drafts.length > 0 ? result : null });
    } catch {
      // Ohne gespeicherte Entwürfe bleibt es bei „Tickets erzeugen“
    }
  }

  // Öffnet die gespeicherten Entwürfe im Dialog
  open(target: DocTarget) {
    const saved = this.find(target)?.saved;
    if (saved) this.patch(target, { result: saved, created: null, error: null });
  }

  // Schließt den Dialog. Sind noch keine Tickets angelegt, werden die bearbeiteten Entwürfe gespeichert
  // (alle verworfen = keine mehr gespeichert); danach heißt der Knopf „Entwürfe öffnen“.
  async close(target: DocTarget, drafts: TicketDraftDto[]): Promise<void> {
    const job = this.find(target);
    if (!job || job.created) {
      this.reset(jobKey(target));
      return;
    }
    if (drafts.length === 0) {
      await this.discard(target);
      return;
    }
    const saved = { ...(job.saved ?? job.result!), drafts };
    this.patch(target, { result: null, saved, error: null });
    try {
      await this.api.saveTicketDrafts(target.pid, target.path, drafts, target.board);
    } catch {
      // Der Stand bleibt in dieser Sitzung erhalten; beim nächsten Schließen wird erneut gespeichert
    }
  }

  // Verwirft die gespeicherten Entwürfe der Datei (und schließt den Dialog)
  async discard(target: DocTarget): Promise<void> {
    this.reset(jobKey(target));
    try {
      await this.api.discardTicketDrafts(target.pid, target.path, target.board);
    } catch {
      // Bleibt der Stand auf dem Server, taucht er beim nächsten Öffnen wieder auf
    }
  }

  private find(target: DocTarget): DocTicketJob | undefined {
    return this.jobs.find((j) => j.key === jobKey(target));
  }

  // Bricht die laufende Erzeugung ab: der Lauf endet sofort (neue Laufnummer, die Antwort fällt weg), der Server beendet Claude.
  // Früher gespeicherte Entwürfe bleiben.
  async cancel(target: DocTarget): Promise<void> {
    if (this.find(target)) this.patch(target, { run: ++this.runs, running: false, error: null });
    try {
      await this.api.cancelTicketDrafts(target.pid);
    } catch {
      // Läuft serverseitig nichts mehr oder ist der Server weg, bleibt nichts zu tun
    }
  }

  // Legt die bearbeiteten Entwürfe als Tickets an; false hält den Dialog mit den Karten offen
  async create(target: DocTarget, drafts: TicketDraftDto[]): Promise<boolean> {
    this.patch(target, { creating: true, error: null });
    try {
      const created = await this.api.createDocTickets(target.pid, { path: target.path, drafts }, target.board);
      // Der Server vergisst die gespeicherten Entwürfe, sobald daraus Tickets entstanden sind
      this.patch(target, { creating: false, created, saved: null });
      return true;
    } catch (e) {
      this.patch(target, { creating: false, error: (e as Error).message });
      return false;
    }
  }

  // Vergisst den Lauf eines Dokuments (Dialog geschlossen, Fehler weggeklickt)
  reset(key: string) {
    if (!this.jobs.some((j) => j.key === key)) return;
    this.jobs = this.jobs.filter((j) => j.key !== key);
    this.listeners.forEach((l) => l());
  }
}

const Context = createContext<DocTicketJobs | null>(null);

// Hält die Ticket-Erzeugungen für die ganze Oberfläche, über Seiten- und Projektwechsel hinweg
export function DocTicketJobsProvider({ children }: { children: ReactNode }) {
  const api = useApi();
  const [jobs] = useState(() => new DocTicketJobs(api));
  return <Context.Provider value={jobs}>{children}</Context.Provider>;
}

// Laufende und fertige Erzeugungen; ohne Provider (einzeln gerenderte Seiten, Tests) gilt ein eigener Speicher je Komponente
export function useDocTicketJobs(): { jobs: DocTicketJobs; list: DocTicketJob[] } {
  const shared = useContext(Context);
  const api = useApi();
  const [own] = useState(() => (shared ? null : new DocTicketJobs(api)));
  const jobs = shared ?? own!;
  return { jobs, list: useSyncExternalStore(jobs.subscribe, jobs.list) };
}
