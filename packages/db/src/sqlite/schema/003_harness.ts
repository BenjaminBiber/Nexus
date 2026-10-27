import type { SqliteSchemaStep } from './SqliteSchemaStep.js';

// Schema-Version 3 (entspricht packages/db/sql/003_harness.sql): Harness-Felder am Ticket, Schritte, Sessions,
// Lesemarke an Kommentaren
export const step003Harness: SqliteSchemaStep = {
  id: '003_harness',
  version: 3,
  sql: `
-- Spec-Bezug, Branch und Worktree je Ticket; Bereiche und Schnittstellen als JSON-Arrays (Text).
-- SQLite fügt je ALTER TABLE nur eine Spalte hinzu.
ALTER TABLE Tickets ADD COLUMN Spec     TEXT NULL;
ALTER TABLE Tickets ADD COLUMN SpecTask TEXT NULL;
ALTER TABLE Tickets ADD COLUMN Branch   TEXT NULL;
ALTER TABLE Tickets ADD COLUMN Worktree TEXT NULL;
ALTER TABLE Tickets ADD COLUMN Touches  TEXT NOT NULL DEFAULT '[]';
ALTER TABLE Tickets ADD COLUMN Consumes TEXT NOT NULL DEFAULT '[]';
ALTER TABLE Tickets ADD COLUMN Produces TEXT NOT NULL DEFAULT '[]';

-- Abhakbare Arbeitsschritte eines Tickets (Position ab 0)
CREATE TABLE TicketSteps (
  TicketId TEXT    NOT NULL CONSTRAINT FK_TicketSteps_Tickets REFERENCES Tickets(Id) ON DELETE CASCADE,
  Position INTEGER NOT NULL,
  Text     TEXT    NOT NULL,
  Done     INTEGER NOT NULL DEFAULT 0,
  Detail   TEXT    NULL,
  CONSTRAINT PK_TicketSteps PRIMARY KEY (TicketId, Position)
);

-- Claude-Sessions, die an einem Ticket arbeiten (Zuordnung für Hooks und Oberfläche)
CREATE TABLE TicketSessions (
  TicketId  TEXT NOT NULL CONSTRAINT FK_TicketSessions_Tickets REFERENCES Tickets(Id) ON DELETE CASCADE,
  SessionId TEXT NOT NULL,
  AddedAt   TEXT NOT NULL,
  CONSTRAINT PK_TicketSessions PRIMARY KEY (TicketId, SessionId)
);
CREATE INDEX IX_TicketSessions_Session ON TicketSessions(SessionId);

-- Wann ein Agent den Kommentar in seine Session bekommen hat; NULL = noch nicht eingespeist
ALTER TABLE Comments ADD COLUMN ReadByAgentAt TEXT NULL;
`,
};
