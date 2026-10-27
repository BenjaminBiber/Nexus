// Bearbeitungsstand eines Kommentars: offen oder erledigt.
export type DocCommentStatus = 'open' | 'resolved';

// Ein gespeicherter Kommentar zu einer Textstelle eines Dokuments.
export interface DocComment {
  id: string;
  status: DocCommentStatus;
  text: string;
  createdAt: string;
  quote: string;
  heading: string | null;
  lines: [number, number] | null;
  resolution: string | null;
}

// Angaben, aus denen ein neuer Kommentar entsteht.
export interface NewDocComment {
  text: string;
  quote: string;
  heading: string | null;
  lines: [number, number] | null;
}

// Änderbare Felder eines bestehenden Kommentars.
export interface DocCommentPatch {
  text?: string;
  status?: DocCommentStatus;
  resolution?: string | null;
}

// Ein offener Kommentar samt Pfad des Dokuments, zu dem er gehört.
export interface OpenDocComment {
  path: string;
  comment: DocComment;
}
