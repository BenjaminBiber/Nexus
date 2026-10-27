import type { SqliteSchemaStep } from './SqliteSchemaStep.js';

// Schema-Version 2 (entspricht packages/db/sql/002_questions.sql): Fragen mit Tags, betroffenen Tickets,
// Abhängigkeiten, Recherchen, Optionen, Kommentaren und Verlauf
export const step002Questions: SqliteSchemaStep = {
  id: '002_questions',
  version: 2,
  sql: `
ALTER TABLE Projects ADD COLUMN NextQuestionNumber INTEGER NOT NULL DEFAULT 1;

CREATE TABLE Questions (
  Id              TEXT    NOT NULL CONSTRAINT PK_Questions PRIMARY KEY,
  ProjectId       INTEGER NOT NULL CONSTRAINT FK_Questions_Projects REFERENCES Projects(Id),
  Number          INTEGER NOT NULL,
  Title           TEXT    NOT NULL,
  Description     TEXT    NOT NULL DEFAULT '',
  State           TEXT    NOT NULL CONSTRAINT CK_Questions_State CHECK (State IN ('open','answered','discarded')),
  Occasion        TEXT    NULL,
  OriginKind      TEXT    NOT NULL CONSTRAINT CK_Questions_OriginKind CHECK (OriginKind IN ('human','session','subagent')),
  OriginName      TEXT    NOT NULL,
  OriginSession   TEXT    NULL,
  DecisionOutcome TEXT    NULL CONSTRAINT CK_Questions_Outcome CHECK (DecisionOutcome IN ('answered','discarded')),
  DecisionOption  TEXT    NULL,
  DecisionText    TEXT    NULL,
  DecisionReason  TEXT    NULL,
  DecidedById     TEXT    NULL CONSTRAINT FK_Questions_DecidedBy REFERENCES Users(Id),
  DecidedByName   TEXT    NULL,
  DecidedAt       TEXT    NULL,
  CreatedAt       TEXT    NOT NULL,
  CreatedById     TEXT    NOT NULL CONSTRAINT FK_Questions_Users REFERENCES Users(Id),
  UpdatedAt       TEXT    NOT NULL,
  Rev             INTEGER NOT NULL,
  RowVer          INTEGER NOT NULL DEFAULT 0,
  CONSTRAINT UQ_Questions_Number UNIQUE (ProjectId, Number)
);
CREATE INDEX IX_Questions_Project ON Questions(ProjectId, State);

CREATE TABLE QuestionTags (
  QuestionId TEXT    NOT NULL CONSTRAINT FK_QuestionTags_Questions REFERENCES Questions(Id) ON DELETE CASCADE,
  Position   INTEGER NOT NULL,
  Tag        TEXT    NOT NULL,
  CONSTRAINT PK_QuestionTags PRIMARY KEY (QuestionId, Tag)
);

-- Betroffene Tickets; der Verweis aufs Ticket hat kein CASCADE (der Dienst entfernt den Bezug vor dem Löschen des Tickets)
CREATE TABLE QuestionTickets (
  QuestionId TEXT NOT NULL CONSTRAINT FK_QuestionTickets_Questions REFERENCES Questions(Id) ON DELETE CASCADE,
  TicketId   TEXT NOT NULL CONSTRAINT FK_QuestionTickets_Tickets REFERENCES Tickets(Id),
  CONSTRAINT PK_QuestionTickets PRIMARY KEY (QuestionId, TicketId)
);
CREATE INDEX IX_QuestionTickets_Ticket ON QuestionTickets(TicketId);

-- Abhängigkeiten auf Fragen oder Tickets; ohne Fremdschlüssel aufs Ziel (zwei mögliche Tabellen)
CREATE TABLE QuestionDependencies (
  QuestionId  TEXT    NOT NULL CONSTRAINT FK_QuestionDeps_Questions REFERENCES Questions(Id) ON DELETE CASCADE,
  Position    INTEGER NOT NULL,
  DependsOnId TEXT    NOT NULL,
  CONSTRAINT PK_QuestionDependencies PRIMARY KEY (QuestionId, DependsOnId)
);
CREATE INDEX IX_QuestionDeps_DependsOn ON QuestionDependencies(DependsOnId);

CREATE TABLE QuestionResearch (
  Id         INTEGER PRIMARY KEY,
  QuestionId TEXT NOT NULL CONSTRAINT FK_QuestionResearch_Questions REFERENCES Questions(Id) ON DELETE CASCADE,
  At         TEXT NOT NULL,
  ById       TEXT NOT NULL CONSTRAINT FK_QuestionResearch_Users REFERENCES Users(Id),
  ByName     TEXT NOT NULL,
  Kind       TEXT NOT NULL CONSTRAINT CK_QuestionResearch_Kind CHECK (Kind IN ('human','session','subagent')),
  Context    TEXT NOT NULL DEFAULT ''
);
CREATE INDEX IX_QuestionResearch_Question ON QuestionResearch(QuestionId);

-- Optionen O1, O2 … fortlaufend je Frage; Vor- und Nachteile als JSON-Text. Wie in SQL Server kaskadiert nur der
-- Verweis auf die Recherche; QuestionId hat keinen Fremdschlüssel.
CREATE TABLE QuestionOptions (
  QuestionId  TEXT    NOT NULL,
  ResearchId  INTEGER NOT NULL CONSTRAINT FK_QuestionOptions_Research REFERENCES QuestionResearch(Id) ON DELETE CASCADE,
  OptionId    TEXT    NOT NULL,
  Title       TEXT    NOT NULL,
  Text        TEXT    NOT NULL,
  Pros        TEXT    NOT NULL DEFAULT '[]',
  Cons        TEXT    NOT NULL DEFAULT '[]',
  Recommended INTEGER NOT NULL,
  CONSTRAINT PK_QuestionOptions PRIMARY KEY (QuestionId, OptionId)
);
CREATE INDEX IX_QuestionOptions_Research ON QuestionOptions(ResearchId);

CREATE TABLE QuestionComments (
  Id         INTEGER PRIMARY KEY,
  QuestionId TEXT NOT NULL CONSTRAINT FK_QuestionComments_Questions REFERENCES Questions(Id) ON DELETE CASCADE,
  AuthorId   TEXT NOT NULL CONSTRAINT FK_QuestionComments_Users REFERENCES Users(Id),
  AuthorName TEXT NOT NULL,
  AuthorKind TEXT NOT NULL CONSTRAINT CK_QuestionComments_Kind CHECK (AuthorKind IN ('human','session','subagent')),
  Body       TEXT NOT NULL,
  CreatedAt  TEXT NOT NULL
);
CREATE INDEX IX_QuestionComments_Question ON QuestionComments(QuestionId);

CREATE TABLE QuestionHistory (
  Id         INTEGER PRIMARY KEY,
  QuestionId TEXT NOT NULL CONSTRAINT FK_QuestionHistory_Questions REFERENCES Questions(Id) ON DELETE CASCADE,
  At         TEXT NOT NULL,
  ById       TEXT NOT NULL CONSTRAINT FK_QuestionHistory_Users REFERENCES Users(Id),
  ByName     TEXT NOT NULL,
  ByKind     TEXT NULL CONSTRAINT CK_QuestionHistory_Kind CHECK (ByKind IN ('human','session','subagent')),
  Type       TEXT NOT NULL,
  Text       TEXT NOT NULL
);
CREATE INDEX IX_QuestionHistory_Question ON QuestionHistory(QuestionId);
`,
};
