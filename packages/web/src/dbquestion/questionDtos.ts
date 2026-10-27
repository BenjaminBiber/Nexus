import type { DbQuestionDetailDto, DbQuestionDto, DbQuestionResearchDto, DbQuestionState } from '../api/dbTypes.js';
import type { QuestionEntryDto, ResearchDto } from '../api/questionTypes.js';
import { filterByFacets, type QuestionFacets, type QuestionFilterState } from '../question/filterQuestions.js';

// Recherchen der Board-Datenbank im Format von ResearchBlock und OptionCard
export function researchDtos(research: DbQuestionResearchDto[]): ResearchDto[] {
  return research.map((r) => ({ at: r.at, author: r.byName, kind: r.kind, context: r.context, options: r.options }));
}

// Verlauf und Kommentare als ein zeitlich sortierter Verlauf im Format von QuestionHistory.
// Einträge ohne Art (z. B. „Ticket gelöscht“) gelten als Eintrag eines Menschen; „ungelesen“ gibt es hier nicht.
export function historyEntries(question: Pick<DbQuestionDetailDto, 'comments' | 'history'>): QuestionEntryDto[] {
  const entries: QuestionEntryDto[] = [
    ...question.history.map((h) => ({ at: h.at, type: h.type, author: h.byName, kind: h.byKind ?? ('human' as const), text: h.text })),
    ...question.comments.map((c) => ({ at: c.createdAt, type: 'comment', author: c.authorName, kind: c.authorKind, text: c.body })),
  ];
  return entries.sort((a, b) => a.at.localeCompare(b.at));
}

// Titel einer Option aus den Recherchen (neueste zuerst)
export function optionTitle(question: Pick<DbQuestionDetailDto, 'research'>, id: string): string | null {
  for (const research of [...question.research].reverse()) {
    const found = research.options.find((o) => o.id === id);
    if (found) return found.title;
  }
  return null;
}

// Merkmale einer Frage der Board-Datenbank für die gemeinsamen Filter; Bezüge sind die betroffenen Tickets
export function dbQuestionFacets(q: DbQuestionDto): QuestionFacets {
  return {
    tags: q.tags,
    links: q.ticketIds.map((target) => ({ target, type: 'Ticket' })),
    origin: q.originKind,
    waiting: q.waitingOn.length > 0,
    researched: q.researched,
    text: [q.id, q.title, q.description, q.occasion ?? '', ...q.tags, ...q.ticketIds],
  };
}

// Fragen nach Zustand und den Filtern der Fragenliste (Tag, Ticket, Herkunft, wartet, recherchiert, Text); Reihenfolge wie vom Server
export function filterDbQuestions(questions: DbQuestionDto[], state: DbQuestionState | 'all', criteria: QuestionFilterState): DbQuestionDto[] {
  return filterByFacets(questions.filter((q) => state === 'all' || q.state === state), criteria, dbQuestionFacets);
}
