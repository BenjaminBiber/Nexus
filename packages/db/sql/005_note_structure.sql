-- Erweiterung „note-structure“ des Boards: Notizen bekommen einen Titel neben dem Inhalt und zählen, wie oft derselbe Befund
-- aufgefallen ist. In AiHarness_Test und in AiHarness ausführen, nach 004. Vorher im Abfragefenster die richtige Datenbank
-- wählen. Ein einziger Batch in einer Transaktion: fehlt dispatch-runs oder ist die Erweiterung schon eingespielt, bricht das
-- Skript ab und ändert nichts.
--
-- Die Schema-Version bleibt 3. Die Spalten haben Defaults, ältere Harness-Stände schreiben und lesen Notizen wie bisher.
-- Bestehende Notizen behalten ihren Text im Inhalt und einen leeren Titel; angezeigt wird dann die erste Zeile des Inhalts.

SET XACT_ABORT ON;
BEGIN TRANSACTION;

IF NOT EXISTS (SELECT 1 FROM board.SchemaFeatures WHERE Name = 'dispatch-runs')
  THROW 50001, N'Erweiterung dispatch-runs fehlt: zuerst 004_dispatch_runs.sql ausführen.', 1;
IF COL_LENGTH(N'board.Notes', N'Title') IS NOT NULL
  THROW 50002, N'Erweiterung note-structure ist bereits eingespielt.', 1;

-- Titel: ein Satz, was auffällt (leer bei älteren Notizen)
ALTER TABLE board.Notes ADD Title nvarchar(200) NOT NULL CONSTRAINT DF_Notes_Title DEFAULT N'';
-- Wie oft der Befund aufgefallen ist, und bei welchen weiteren Tickets (kommagetrennt)
ALTER TABLE board.Notes ADD SeenCount int NOT NULL CONSTRAINT DF_Notes_SeenCount DEFAULT 1;
ALTER TABLE board.Notes ADD SeenIn nvarchar(1000) NOT NULL CONSTRAINT DF_Notes_SeenIn DEFAULT N'';

INSERT INTO board.SchemaFeatures (Name, Script) VALUES ('note-structure', N'005_note_structure.sql');
COMMIT TRANSACTION;
GO
