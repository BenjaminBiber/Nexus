
-- Ersatz für rowversion: ein datenbankweiter Zähler (genau eine Zeile)
CREATE TABLE ChangeCounter (
  Id    INTEGER PRIMARY KEY CONSTRAINT CK_ChangeCounter_Single CHECK (Id = 1),
  Value INTEGER NOT NULL
);
INSERT INTO ChangeCounter (Id, Value) VALUES (1, 0);

-- Je Tabelle: nach INSERT und nach jedem UPDATE, das RowVer nicht selbst ändert, Zähler erhöhen und in RowVer schreiben.
-- Die WHEN-Bedingung verhindert, dass das UPDATE im Trigger den Update-Trigger erneut auslöst (auch mit recursive_triggers).
CREATE TRIGGER TR_Boards_RowVer_Insert AFTER INSERT ON Boards
BEGIN
  UPDATE ChangeCounter SET Value = Value + 1 WHERE Id = 1;
  UPDATE Boards SET RowVer = (SELECT Value FROM ChangeCounter WHERE Id = 1) WHERE rowid = NEW.rowid;
END;
CREATE TRIGGER TR_Boards_RowVer_Update AFTER UPDATE ON Boards
WHEN NEW.RowVer = OLD.RowVer
BEGIN
  UPDATE ChangeCounter SET Value = Value + 1 WHERE Id = 1;
  UPDATE Boards SET RowVer = (SELECT Value FROM ChangeCounter WHERE Id = 1) WHERE rowid = NEW.rowid;
END;

CREATE TRIGGER TR_Tickets_RowVer_Insert AFTER INSERT ON Tickets
BEGIN
  UPDATE ChangeCounter SET Value = Value + 1 WHERE Id = 1;
  UPDATE Tickets SET RowVer = (SELECT Value FROM ChangeCounter WHERE Id = 1) WHERE rowid = NEW.rowid;
END;
CREATE TRIGGER TR_Tickets_RowVer_Update AFTER UPDATE ON Tickets
WHEN NEW.RowVer = OLD.RowVer
BEGIN
  UPDATE ChangeCounter SET Value = Value + 1 WHERE Id = 1;
  UPDATE Tickets SET RowVer = (SELECT Value FROM ChangeCounter WHERE Id = 1) WHERE rowid = NEW.rowid;
END;

CREATE TRIGGER TR_Comments_RowVer_Insert AFTER INSERT ON Comments
BEGIN
  UPDATE ChangeCounter SET Value = Value + 1 WHERE Id = 1;
  UPDATE Comments SET RowVer = (SELECT Value FROM ChangeCounter WHERE Id = 1) WHERE rowid = NEW.rowid;
END;
CREATE TRIGGER TR_Comments_RowVer_Update AFTER UPDATE ON Comments
WHEN NEW.RowVer = OLD.RowVer
BEGIN
  UPDATE ChangeCounter SET Value = Value + 1 WHERE Id = 1;
  UPDATE Comments SET RowVer = (SELECT Value FROM ChangeCounter WHERE Id = 1) WHERE rowid = NEW.rowid;
END;

CREATE TRIGGER TR_Notes_RowVer_Insert AFTER INSERT ON Notes
BEGIN
  UPDATE ChangeCounter SET Value = Value + 1 WHERE Id = 1;
  UPDATE Notes SET RowVer = (SELECT Value FROM ChangeCounter WHERE Id = 1) WHERE rowid = NEW.rowid;
END;
CREATE TRIGGER TR_Notes_RowVer_Update AFTER UPDATE ON Notes
WHEN NEW.RowVer = OLD.RowVer
BEGIN
  UPDATE ChangeCounter SET Value = Value + 1 WHERE Id = 1;
  UPDATE Notes SET RowVer = (SELECT Value FROM ChangeCounter WHERE Id = 1) WHERE rowid = NEW.rowid;
END;

CREATE TRIGGER TR_Questions_RowVer_Insert AFTER INSERT ON Questions
BEGIN
  UPDATE ChangeCounter SET Value = Value + 1 WHERE Id = 1;
  UPDATE Questions SET RowVer = (SELECT Value FROM ChangeCounter WHERE Id = 1) WHERE rowid = NEW.rowid;
END;
CREATE TRIGGER TR_Questions_RowVer_Update AFTER UPDATE ON Questions
WHEN NEW.RowVer = OLD.RowVer
BEGIN
  UPDATE ChangeCounter SET Value = Value + 1 WHERE Id = 1;
  UPDATE Questions SET RowVer = (SELECT Value FROM ChangeCounter WHERE Id = 1) WHERE rowid = NEW.rowid;
END;
