import { createContext, useContext, useState, useSyncExternalStore, type ReactNode } from 'react';
import { useApi } from '../api/ApiContext.js';
import type { ApiClient } from '../api/ApiClient.js';
import type { DocQuestionResultDto, QuestionDraftInputDto, QuestionDraftResultDto } from '../api/types.js';
import { jobKey, type DocTarget } from './docTicketJobs.js';

/** Stand einer Fragen-Erzeugung zu einem Dokument */
export interface DocQuestionJob extends DocTarget {
  key: string;
  /** Nummer des Laufs; eine Antwort eines abgebrochenen oder ersetzten Laufs fällt weg */
  run: number;
  running: boolean;
  /** Entwürfe im offenen Dialog; null, solange er zu ist */
  result: QuestionDraftResultDto | null;
  creating: boolean;
  created: DocQuestionResultDto | null;
  error: string | null;
}

// Fragen-Erzeugungen aller Dokumente außerhalb der Seiten (wie DocTicketJobs): ein Lauf überlebt den Seitenwechsel.
// Die Entwürfe werden nicht auf dem Server gespeichert; wer den Dialog schließt, verwirft sie.
export class DocQuestionJobs {
  private jobs: DocQuestionJob[] = [];
  private listeners = new Set<() => void>();
  private runs = 0;

  constructor(private readonly api: ApiClient) {}

  subscribe = (listener: () => void) => {
    this.listeners.add(listener);
    return () => void this.listeners.delete(listener);
  };

  list = () => this.jobs;

  // Ändert den Lauf eines Dokuments (legt ihn bei Bedarf an) und meldet die neue Liste
  private patch(target: DocTarget, change: Partial<DocQuestionJob>) {
    const key = jobKey(target);
    const old = this.jobs.find((j) => j.key === key)
      ?? { ...target, key, run: 0, running: false, result: null, creating: false, created: null, error: null };
    this.jobs = [...this.jobs.filter((j) => j.key !== key), { ...old, ...change }];
    this.listeners.forEach((l) => l());
  }

  private find(target: DocTarget): DocQuestionJob | undefined {
    return this.jobs.find((j) => j.key === jobKey(target));
  }

  // Lässt Claude die offenen Fragen herauslesen; die Antwort zählt nur, solange der Lauf noch derselbe ist
  async request(target: DocTarget): Promise<void> {
    const run = ++this.runs;
    this.patch(target, { run, running: true, result: null, created: null, error: null });
    const current = () => this.find(target)?.run === run;
    try {
      const result = await this.api.createQuestionDrafts(target.pid, target.path, target.board);
      if (current()) this.patch(target, { running: false, result });
    } catch (e) {
      if (current()) this.patch(target, { running: false, error: (e as Error).message });
    }
  }

  // Bricht die laufende Erzeugung ab: neue Laufnummer (die Antwort fällt weg), der Server beendet Claude
  async cancel(target: DocTarget): Promise<void> {
    if (this.find(target)) this.patch(target, { run: ++this.runs, running: false, error: null });
    try {
      await this.api.cancelQuestionDrafts(target.pid);
    } catch {
      // Läuft serverseitig nichts mehr oder ist der Server weg, bleibt nichts zu tun
    }
  }

  // Legt die gewählten Entwürfe als Fragen an; false hält den Dialog mit den Entwürfen offen
  async create(target: DocTarget, drafts: QuestionDraftInputDto[]): Promise<boolean> {
    this.patch(target, { creating: true, error: null });
    try {
      const created = await this.api.createDocQuestions(target.pid, target.path, drafts, target.board);
      this.patch(target, { creating: false, created });
      return true;
    } catch (e) {
      this.patch(target, { creating: false, error: (e as Error).message });
      return false;
    }
  }

  // Vergisst den Lauf eines Dokuments (Dialog geschlossen, Entwürfe verworfen)
  reset(key: string) {
    if (!this.jobs.some((j) => j.key === key)) return;
    this.jobs = this.jobs.filter((j) => j.key !== key);
    this.listeners.forEach((l) => l());
  }
}

const Context = createContext<DocQuestionJobs | null>(null);

// Hält die Fragen-Erzeugungen für die ganze Oberfläche, über Seiten- und Projektwechsel hinweg
export function DocQuestionJobsProvider({ children }: { children: ReactNode }) {
  const api = useApi();
  const [jobs] = useState(() => new DocQuestionJobs(api));
  return <Context.Provider value={jobs}>{children}</Context.Provider>;
}

// Fragen-Erzeugung eines Dokuments; ohne Provider (einzeln gerenderte Seiten, Tests) gilt ein eigener Speicher je Komponente
export function useDocQuestions(pid: string, path: string | null, board?: number) {
  const shared = useContext(Context);
  const api = useApi();
  const [own] = useState(() => (shared ? null : new DocQuestionJobs(api)));
  const jobs = shared ?? own!;
  const list = useSyncExternalStore(jobs.subscribe, jobs.list);
  const target = path ? { pid, path, board } : null;
  const job = target ? list.find((j) => j.key === jobKey(target)) : undefined;
  return {
    request: () => { if (target) void jobs.request(target); },
    cancel: () => { if (target) void jobs.cancel(target); },
    create: async (drafts: QuestionDraftInputDto[]) => (target ? jobs.create(target, drafts) : false),
    close: () => { if (target) jobs.reset(jobKey(target)); },
    running: job?.running ?? false,
    creating: job?.creating ?? false,
    result: job?.result ?? null,
    created: job?.created ?? null,
    error: job?.error ?? null,
  };
}
