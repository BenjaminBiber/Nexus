// Prüft, dass die aus Rust erzeugten Typen zu den heutigen TypeScript-Typen passen.
// Die Datei wird nie ausgeführt, nur übersetzt: `npx tsc -p crates/nexus-core/bindings-check/tsconfig.json`.
// Schlägt der Übersetzer an, weicht ein Rust-Struct vom TypeScript-Vertrag ab.

import type * as GenBoard from '../../../packages/web/src/api/generated/board';
import type * as GenQuestion from '../../../packages/web/src/api/generated/question';
import type * as GenDispatch from '../../../packages/web/src/api/generated/dispatch';
import type * as GenSearch from '../../../packages/web/src/api/generated/search';
import type * as GenInbox from '../../../packages/web/src/api/generated/inbox';

import type * as CoreBoard from '../../../packages/core/src/board/BoardTypes.js';
import type * as CoreQuestion from '../../../packages/core/src/board/BoardQuestionTypes.js';
import type * as CoreDispatch from '../../../packages/core/src/board/DispatchRunTypes.js';
import type * as CoreSearch from '../../../packages/core/src/board/BoardSearchTypes.js';
import type * as CoreInbox from '../../../packages/core/src/board/inbox/InboxTypes.js';
import type * as CoreActor from '../../../packages/core/src/actors/Actor.js';
import type * as CoreQuestionState from '../../../packages/core/src/questions/Question.js';

import type * as WebDb from '../../../packages/web/src/api/dbTypes.js';
import type * as WebInbox from '../../../packages/web/src/api/inboxTypes.js';
import type * as WebStats from '../../../packages/web/src/api/statsTypes.js';
import type * as WebQuestion from '../../../packages/web/src/api/questionTypes.js';

// A und B sind gegenseitig zuweisbar
type Same<A, B> = [A] extends [B] ? ([B] extends [A] ? true : false) : false;
// A ist nach B zuweisbar (für Web-Typen, die absichtlich weiter gefasst sind)
type AssignableTo<A, B> = [A] extends [B] ? true : false;

const ok = <T extends true>() => {};

// ---------------------------------------------------------------------------
// board.ts ↔ packages/core/src/board/BoardTypes.ts
// ---------------------------------------------------------------------------
ok<Same<GenBoard.TicketStatus, CoreBoard.TicketStatus>>();
ok<Same<GenBoard.Priority, CoreBoard.Priority>>();
ok<Same<GenBoard.NoteState, CoreBoard.NoteState>>();
ok<Same<GenBoard.BoardActor, CoreBoard.BoardActor>>();
ok<Same<GenBoard.BoardUser, CoreBoard.BoardUser>>();
ok<Same<GenBoard.TicketStep, CoreBoard.TicketStep>>();
ok<Same<GenBoard.StepProgress, CoreBoard.StepProgress>>();
ok<Same<GenBoard.Project, CoreBoard.Project>>();
ok<Same<GenBoard.Board, CoreBoard.Board>>();
ok<Same<GenBoard.Ticket, CoreBoard.Ticket>>();
ok<Same<GenBoard.Comment, CoreBoard.Comment>>();
ok<Same<GenBoard.HistoryEntry, CoreBoard.HistoryEntry>>();
ok<Same<GenBoard.Note, CoreBoard.Note>>();
ok<Same<GenBoard.Deletion, CoreBoard.Deletion>>();

// ---------------------------------------------------------------------------
// question.ts ↔ packages/core/src/board/BoardQuestionTypes.ts (dazu Actor.ts und Question.ts)
// ---------------------------------------------------------------------------
ok<Same<GenQuestion.AuthorKind, CoreActor.AuthorKind>>();
ok<Same<GenQuestion.QuestionState, CoreQuestionState.QuestionState>>();
ok<Same<GenQuestion.BoardQuestionHistoryType, CoreQuestion.BoardQuestionHistoryType>>();
ok<Same<GenQuestion.BoardQuestionActor, CoreQuestion.BoardQuestionActor>>();
ok<Same<GenQuestion.BoardQuestionOption, CoreQuestion.BoardQuestionOption>>();
ok<Same<GenQuestion.BoardQuestionResearch, CoreQuestion.BoardQuestionResearch>>();
ok<Same<GenQuestion.BoardQuestionResearchSummary, CoreQuestion.BoardQuestionResearchSummary>>();
ok<Same<GenQuestion.BoardQuestionDecision, CoreQuestion.BoardQuestionDecision>>();
// In TypeScript steht der Ausgang als Union direkt im Feld, in Rust als eigenes Enum
ok<Same<GenQuestion.BoardQuestionDecisionOutcome, CoreQuestion.BoardQuestionDecision['outcome']>>();
ok<Same<GenQuestion.BoardQuestion, CoreQuestion.BoardQuestion>>();
ok<Same<GenQuestion.BoardQuestionComment, CoreQuestion.BoardQuestionComment>>();
ok<Same<GenQuestion.BoardQuestionHistoryEntry, CoreQuestion.BoardQuestionHistoryEntry>>();
ok<Same<GenQuestion.BoardQuestionTicketRef, CoreQuestion.BoardQuestionTicketRef>>();
ok<Same<GenQuestion.BoardQuestionWaiting, CoreQuestion.BoardQuestionWaiting>>();
ok<Same<GenQuestion.BoardQuestionView, CoreQuestion.BoardQuestionView>>();
ok<Same<GenQuestion.BoardQuestionDetail, CoreQuestion.BoardQuestionDetail>>();
ok<Same<GenQuestion.BoardOpenQuestion, CoreQuestion.BoardOpenQuestion>>();
ok<Same<GenQuestion.NotifiableBoardQuestion, CoreQuestion.NotifiableBoardQuestion>>();
// Die anonymen Objekte der Meldung heißen in Rust NotifiableOrigin und NotifiableResearch
ok<Same<GenQuestion.NotifiableOrigin, CoreQuestion.NotifiableBoardQuestion['origin']>>();
ok<Same<GenQuestion.NotifiableResearch, CoreQuestion.NotifiableBoardQuestion['research'][number]>>();

// ---------------------------------------------------------------------------
// dispatch.ts ↔ packages/core/src/board/DispatchRunTypes.ts
// DispatchStatsFilter fehlt mit Absicht: dort stehen in TypeScript Date-Werte,
// der Rust-Typ bleibt intern und wird nicht nach TypeScript erzeugt.
// ---------------------------------------------------------------------------
ok<Same<GenDispatch.DispatchRunKind, CoreDispatch.DispatchRunKind>>();
ok<Same<GenDispatch.DispatchRunOutcome, CoreDispatch.DispatchRunOutcome>>();
ok<Same<GenDispatch.DispatchProfileReason, CoreDispatch.DispatchProfileReason>>();
ok<Same<GenDispatch.DispatchRunSource, CoreDispatch.DispatchRunSource>>();
ok<Same<GenDispatch.DispatchRun, CoreDispatch.DispatchRun>>();
ok<Same<GenDispatch.DispatchConfigVersion, CoreDispatch.DispatchConfigVersion>>();
ok<Same<GenDispatch.DispatchRunFilter, CoreDispatch.DispatchRunFilter>>();
ok<Same<GenDispatch.DispatchCommentCounts, CoreDispatch.DispatchCommentCounts>>();
ok<Same<GenDispatch.DispatchProfileRow, CoreDispatch.DispatchProfileRow>>();
ok<Same<GenDispatch.DispatchStats, CoreDispatch.DispatchStats>>();
// Die anonymen Teile der Auswertung heißen in Rust eigene Structs
ok<Same<GenDispatch.DispatchStatsKpis, CoreDispatch.DispatchStats['kpis']>>();
ok<Same<GenDispatch.DispatchSeries, CoreDispatch.DispatchStats['series']>>();
ok<Same<GenDispatch.DispatchSeriesUnit, CoreDispatch.DispatchStats['series']['unit']>>();
ok<Same<GenDispatch.DispatchSeriesPoint, CoreDispatch.DispatchStats['series']['points'][number]>>();
ok<Same<GenDispatch.DispatchLadder, CoreDispatch.DispatchStats['ladder']>>();
ok<Same<GenDispatch.DispatchAbortReason, CoreDispatch.DispatchStats['abortReasons'][number]>>();

// ---------------------------------------------------------------------------
// search.ts ↔ packages/core/src/board/BoardSearchTypes.ts
// ---------------------------------------------------------------------------
ok<Same<GenSearch.SearchKind, CoreSearch.SearchKind>>();
ok<Same<GenSearch.SearchField, CoreSearch.SearchField>>();
ok<Same<GenSearch.SearchRequest, CoreSearch.SearchRequest>>();
ok<Same<GenSearch.SearchRow, CoreSearch.SearchRow>>();
ok<Same<GenSearch.SearchRows, CoreSearch.SearchRows>>();
ok<Same<GenSearch.SearchHit, CoreSearch.SearchHit>>();
ok<Same<GenSearch.SearchResult, CoreSearch.SearchResult>>();
// Projekt und Board eines Treffers sind in TypeScript anonym
ok<Same<GenSearch.SearchProjectRef, CoreSearch.SearchHit['project']>>();
ok<Same<GenSearch.SearchBoardRef, NonNullable<CoreSearch.SearchHit['board']>>>();

// ---------------------------------------------------------------------------
// inbox.ts ↔ packages/core/src/board/inbox/InboxTypes.ts
// InboxTicketKind fehlt mit Absicht: das ist eine reine TypeScript-Verengung
// von InboxKind ohne eigenes JSON.
// ---------------------------------------------------------------------------
ok<Same<GenInbox.InboxKind, CoreInbox.InboxKind>>();
ok<Same<GenInbox.InboxItem, CoreInbox.InboxItem>>();
ok<Same<GenInbox.InboxList, CoreInbox.InboxList>>();
// counts ist in Rust ein Struct mit festen Feldern, in TypeScript Record<InboxKind, number>
ok<Same<GenInbox.InboxCounts, CoreInbox.InboxList['counts']>>();

// ---------------------------------------------------------------------------
// Gegenstücke der Oberfläche (packages/web/src/api): die erzeugten Typen müssen in
// die Teilmenge passen, die der Web-DTO beschreibt.
// ---------------------------------------------------------------------------
ok<Same<GenBoard.TicketStatus, WebDb.DbStatus>>();
ok<Same<GenBoard.Priority, WebDb.DbPriority>>();
ok<Same<GenBoard.NoteState, WebDb.DbNoteState>>();
ok<Same<GenBoard.BoardUser, WebDb.DbUserDto>>();
ok<Same<GenBoard.TicketStep, WebDb.DbStepDto>>();
ok<Same<Pick<WebDb.DbTicketDto, keyof GenBoard.Ticket>, GenBoard.Ticket>>();
ok<Same<Pick<WebDb.DbBoardDto, keyof GenBoard.Board>, GenBoard.Board>>();
ok<Same<Pick<WebDb.DbNoteDto, keyof GenBoard.Note>, GenBoard.Note>>();
ok<Same<WebDb.DbHistoryDto, GenBoard.HistoryEntry>>();
// DbProjectDto fehlt mit Absicht: die Oberfläche kennt nextQuestionNumber nicht.
// DbCommentDto fehlt mit Absicht: dort ist readByAgentAt optional (`?`), im Kern und
// in der Datenbank steht das Feld immer.
ok<AssignableTo<GenBoard.Comment, WebDb.DbCommentDto>>();

ok<Same<GenQuestion.AuthorKind, WebDb.DbOriginKind>>();
ok<Same<GenQuestion.AuthorKind, WebQuestion.OriginKind>>();
ok<Same<GenQuestion.QuestionState, WebDb.DbQuestionState>>();
ok<Same<GenQuestion.BoardQuestionOption, WebDb.DbQuestionOptionDto>>();
ok<Same<GenQuestion.BoardQuestionOption, WebQuestion.QuestionOptionDto>>();
ok<Same<GenQuestion.BoardQuestionResearch, WebDb.DbQuestionResearchDto>>();
ok<Same<GenQuestion.BoardQuestionDecision, WebDb.DbQuestionDecisionDto>>();
ok<Same<GenQuestion.BoardQuestionTicketRef, WebDb.DbQuestionTicketRefDto>>();
ok<Same<GenQuestion.BoardQuestionComment, WebDb.DbQuestionCommentDto>>();
ok<Same<GenQuestion.BoardOpenQuestion, WebDb.DbOpenQuestionDto>>();
ok<Same<Pick<WebDb.DbQuestionDto, keyof GenQuestion.BoardQuestionView>, GenQuestion.BoardQuestionView>>();
// DbQuestionHistoryDto beschreibt `type` als string statt als Union; der erzeugte Typ
// passt hinein, umgekehrt gilt das nicht.
ok<AssignableTo<GenQuestion.BoardQuestionHistoryEntry, WebDb.DbQuestionHistoryDto>>();

ok<Same<GenSearch.SearchKind, WebDb.DbSearchKind>>();
ok<Same<GenSearch.SearchField, WebDb.DbSearchField>>();
ok<Same<GenSearch.SearchHit, WebDb.DbSearchHitDto>>();
ok<Same<GenSearch.SearchResult, WebDb.DbSearchResultDto>>();

ok<Same<GenInbox.InboxKind, WebInbox.InboxKind>>();
ok<Same<GenInbox.InboxItem, WebInbox.InboxItemDto>>();
ok<Same<GenInbox.InboxList, WebInbox.InboxListDto>>();

ok<Same<GenDispatch.DispatchRunKind, WebStats.DispatchRunKindDto>>();
ok<Same<GenDispatch.DispatchProfileRow, WebStats.ProfileRowDto>>();
ok<Same<GenDispatch.DispatchSeriesPoint, WebStats.SeriesPointDto>>();
ok<Same<Pick<WebStats.DispatchStatsDto, keyof GenDispatch.DispatchStats>, GenDispatch.DispatchStats>>();
// WebStats.DispatchStatsFilter fehlt mit Absicht: das ist der Filter der HTTP-Anfrage
// (Projekt-Schlüssel, Tage als Text), nicht der Filter der Auswertung im Kern.
