/** Stelle im Dokument, aus der ein Entwurf stammt; Zeilen 1-basiert und einschließlich */
export interface TicketDraftSource {
  heading: string;
  startLine: number;
  endLine: number;
}

/** Vorschlag für ein Ticket aus einer markierten Stelle; `dependsOn` sind 0-basierte Indizes in dieselbe Liste */
export interface TicketDraft {
  title: string;
  description: string;
  acceptance: string;
  steps: string[];
  dependsOn: number[];
  labels: string[];
  source: TicketDraftSource;
}

/** Geprüfte Entwürfe samt kurzen Hinweisen auf Verworfenes */
export interface TicketDraftCheck {
  drafts: TicketDraft[];
  dropped: string[];
}
