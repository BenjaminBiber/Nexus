-- Erweiterung „dispatch-run-details“ des Boards: Läufe bekommen die Werte, die das Optimierungs-Dashboard braucht – Schritt- und
-- Zeitlimit, Grund der Profilwahl, Version der Dispatcher-Einstellung und abgelehnte Werkzeugaufrufe – dazu die Tabelle der
-- Konfig-Versionen. In AiHarness_Test und in AiHarness ausführen, nach 005. Vorher im Abfragefenster die richtige Datenbank
-- wählen. Ein einziger Batch in einer Transaktion: fehlt note-structure oder ist die Erweiterung schon eingespielt, bricht das
-- Skript ab und ändert nichts.
--
-- Die Schema-Version bleibt 3. Alle neuen Spalten sind NULL-bar ohne Default, ältere Harness-Stände schreiben Läufe wie bisher.

SET XACT_ABORT ON;
BEGIN TRANSACTION;

IF NOT EXISTS (SELECT 1 FROM board.SchemaFeatures WHERE Name = 'note-structure')
  THROW 50001, N'Erweiterung note-structure fehlt: zuerst 005_note_structure.sql ausführen.', 1;
IF COL_LENGTH(N'board.DispatchRuns', N'MaxTurns') IS NOT NULL
  THROW 50002, N'Erweiterung dispatch-run-details ist bereits eingespielt.', 1;

-- Schritt- und Zeitlimit, die für den Lauf galten
ALTER TABLE board.DispatchRuns ADD MaxTurns int NULL;
ALTER TABLE board.DispatchRuns ADD TimeoutMinutes int NULL;
-- Warum das Basisprofil gewählt wurde, und bei einer Regel deren 1-basierte Nummer
ALTER TABLE board.DispatchRuns ADD ProfileReason varchar(10) NULL CONSTRAINT CK_DispatchRuns_ProfileReason CHECK (ProfileReason IN ('default', 'label', 'plan', 'rule', 'setting'));
ALTER TABLE board.DispatchRuns ADD RuleIndex int NULL;
-- Version der Dispatcher-Einstellung, mit der der Lauf gestartet wurde
ALTER TABLE board.DispatchRuns ADD ConfigHash varchar(64) NULL;
-- Abgelehnte Werkzeugaufrufe: Anzahl und die Werkzeuge kommagetrennt
ALTER TABLE board.DispatchRuns ADD DeniedCount int NULL;
ALTER TABLE board.DispatchRuns ADD DeniedTools nvarchar(500) NULL;

-- Läufe einer Konfig-Fassung; als Zeichenkette ausgeführt, weil ConfigHash im selben Batch angelegt wird und der Übersetzer
-- die Spalte sonst noch nicht kennt
EXEC(N'CREATE INDEX IX_DispatchRuns_Config ON board.DispatchRuns(ConfigHash)');

-- Eine Zeile je gesehener Fassung der Dispatcher-Einstellung; Inhalt als JSON, damit das Dashboard Änderungen vergleichen kann
CREATE TABLE board.DispatchConfigVersions (
  Hash        varchar(64)   NOT NULL CONSTRAINT PK_DispatchConfigVersions PRIMARY KEY,
  Dispatcher  nvarchar(100) NOT NULL,
  FirstSeenAt datetime2(3)  NOT NULL,
  Content     nvarchar(max) NOT NULL
);
CREATE INDEX IX_DispatchConfigVersions_FirstSeen ON board.DispatchConfigVersions(FirstSeenAt);

INSERT INTO board.SchemaFeatures (Name, Script) VALUES ('dispatch-run-details', N'006_dispatch_run_details.sql');
COMMIT TRANSACTION;
GO
