-- Erweiterung „target-branch“ des Boards: Zielbranch am Ticket (Branch, von dem der Ticket-Branch abzweigt und in den der
-- Dispatcher mergt). In AiHarness_Test und in AiHarness ausführen, nach 004. Vorher im Abfragefenster die richtige
-- Datenbank wählen. Ein einziger Batch in einer Transaktion: fehlt 004 (dispatch-runs) oder ist die Erweiterung schon
-- eingespielt, bricht das Skript ab und ändert nichts.
--
-- Die Schema-Version bleibt 3: ältere Harness-Stände prüfen MAX(board.SchemaVersion) und würden eine 4 ablehnen. Die neue
-- Spalte ist nullable, ältere Stände lesen und schreiben Tickets weiter ohne sie; neuere Stände verlangen den Eintrag in
-- board.SchemaFeatures.

SET XACT_ABORT ON;
BEGIN TRANSACTION;

IF OBJECT_ID(N'board.SchemaFeatures') IS NULL
  THROW 50001, N'Erweiterung dispatch-runs fehlt: zuerst 004_dispatch_runs.sql ausführen.', 1;
IF EXISTS (SELECT 1 FROM board.SchemaFeatures WHERE Name = 'target-branch')
  THROW 50002, N'Erweiterung target-branch ist bereits eingespielt.', 1;

-- Zielbranch des Tickets; leer (NULL) heißt: der Standardbranch des Projekts gilt wie bisher
ALTER TABLE board.Tickets ADD TargetBranch nvarchar(400) NULL;

INSERT INTO board.SchemaFeatures (Name, Script) VALUES ('target-branch', N'007_target_branch.sql');
COMMIT TRANSACTION;
GO
