// Art des Handelnden: Mensch, Claude-Session oder Subagent
export type AuthorKind = 'human' | 'session' | 'subagent';

// Wer eine Änderung oder einen Kommentar macht
export interface Actor {
  author: string;
  kind: AuthorKind;
}

// Wer bei Fragen handelt; bei Sessions mit der Kennung der Session
export type QuestionActor = Actor & { session?: string };
