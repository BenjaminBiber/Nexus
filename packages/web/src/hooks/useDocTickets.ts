import { useEffect } from 'react';
import type { TicketDraftDto } from '../api/types.js';
import { jobKey, useDocTicketJobs } from './docTicketJobs.js';

// Lässt Claude Ticket-Entwürfe aus dem ganzen Dokument schreiben und legt die geprüften Entwürfe an.
// Der Stand liegt in DocTicketJobs (hooks/docTicketJobs.tsx), damit ein Lauf den Wechsel auf andere Seiten übersteht.
// Fehler bleiben als Text stehen, damit der Dialog mit den bearbeiteten Karten offen bleiben kann.
export function useDocTickets(pid: string, path: string | null, board?: number) {
  const { jobs, list } = useDocTicketJobs();
  const target = path ? { pid, path, board } : null;
  const job = target ? list.find((j) => j.key === jobKey(target)) : undefined;

  // Gespeicherte Entwürfe der Datei holen, damit der Knopf „Entwürfe öffnen“ anbieten kann
  useEffect(() => {
    if (path) void jobs.load({ pid, path, board });
  }, [jobs, pid, path, board]);

  return {
    // Lässt Claude das ganze Dokument in Ticket-Entwürfe zerlegen
    request: async () => {
      if (target) await jobs.request(target);
    },
    create: async (drafts: TicketDraftDto[]) => (target ? jobs.create(target, drafts) : false),
    reset: () => { if (target) jobs.reset(jobKey(target)); },
    cancel: () => { if (target) void jobs.cancel(target); },
    open: () => { if (target) jobs.open(target); },
    close: (drafts: TicketDraftDto[]) => { if (target) void jobs.close(target, drafts); },
    discard: () => { if (target) void jobs.discard(target); },
    running: job?.running ?? false,
    creating: job?.creating ?? false,
    result: job?.result ?? null,
    saved: job?.saved ?? null,
    created: job?.created ?? null,
    error: job?.error ?? null,
  };
}
