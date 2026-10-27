// Teilformate der Fragen, die die Oberfläche der Board-DB-Fragen weiterhin nutzt (Recherche, Optionen, Verlauf).

export type OriginKind = 'human' | 'session' | 'subagent';

export interface QuestionOptionDto {
  id: string;
  title: string;
  text: string;
  pros: string[];
  cons: string[];
  recommended: boolean;
}

export interface ResearchDto {
  at: string;
  author: string;
  kind: OriginKind;
  context: string;
  options: QuestionOptionDto[];
}

export interface QuestionEntryDto {
  at: string;
  type: string;
  author: string;
  kind: OriginKind;
  read?: boolean;
  text: string;
}
