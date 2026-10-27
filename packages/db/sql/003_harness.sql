-- Schema-Version 3 des Boards: Harness-Funktionen am Ticket (Teilprojekt 4). In AiHarness_Test und in AiHarness
-- ausführen, nach 002. Vorher im Abfragefenster die richtige Datenbank wählen. Ein einziger Batch in einer
-- Transaktion: fehlt Version 2 oder ist Version 3 schon eingespielt, bricht das Skript ab und ändert nichts.
-- Die neuen Spalten werden in diesem Batch nicht weiter verwendet (SQL Server übersetzt den Batch vorab).

SET XACT_ABORT ON;
BEGIN TRANSACTION;

IF NOT EXISTS (SELECT 1 FROM board.SchemaVersion WHERE Version = 2)
  THROW 50001, N'Schema-Version 2 fehlt: zuerst 002_questions.sql ausführen.', 1;
IF EXISTS (SELECT 1 FROM board.SchemaVersion WHERE Version >= 3)
  THROW 50002, N'Schema-Version 3 ist bereits eingespielt.', 1;

-- Spec-Bezug, Branch und Worktree je Ticket; Bereiche und Schnittstellen als JSON-Arrays (Text)
ALTER TABLE board.Tickets ADD
  Spec     nvarchar(200) NULL,
  SpecTask nvarchar(20)  NULL,
  Branch   nvarchar(400) NULL,
  Worktree nvarchar(500) NULL,
  Touches  nvarchar(max) NOT NULL CONSTRAINT DF_Tickets_Touches DEFAULT N'[]',
  Consumes nvarchar(max) NOT NULL CONSTRAINT DF_Tickets_Consumes DEFAULT N'[]',
  Produces nvarchar(max) NOT NULL CONSTRAINT DF_Tickets_Produces DEFAULT N'[]';

-- Abhakbare Arbeitsschritte eines Tickets (Position ab 0)
CREATE TABLE board.TicketSteps (
  TicketId nvarchar(20)   NOT NULL CONSTRAINT FK_TicketSteps_Tickets REFERENCES board.Tickets(Id) ON DELETE CASCADE,
  Position int            NOT NULL,
  Text     nvarchar(1000) NOT NULL,
  Done     bit            NOT NULL CONSTRAINT DF_TicketSteps_Done DEFAULT 0,
  Detail   nvarchar(max)  NULL,
  CONSTRAINT PK_TicketSteps PRIMARY KEY (TicketId, Position)
);

-- Claude-Sessions, die an einem Ticket arbeiten (Zuordnung für Hooks und Oberfläche)
CREATE TABLE board.TicketSessions (
  TicketId  nvarchar(20)  NOT NULL CONSTRAINT FK_TicketSessions_Tickets REFERENCES board.Tickets(Id) ON DELETE CASCADE,
  SessionId nvarchar(100) NOT NULL,
  AddedAt   datetime2(3)  NOT NULL,
  CONSTRAINT PK_TicketSessions PRIMARY KEY (TicketId, SessionId)
);
CREATE INDEX IX_TicketSessions_Session ON board.TicketSessions(SessionId);

-- Wann ein Agent den Kommentar in seine Session bekommen hat; NULL = noch nicht eingespeist
ALTER TABLE board.Comments ADD ReadByAgentAt datetime2(3) NULL;

INSERT INTO board.SchemaVersion (Version) VALUES (3);
COMMIT TRANSACTION;
GO
