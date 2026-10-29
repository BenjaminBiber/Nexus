-- Legt die Datenbanken für das Board an und gibt dem gemeinsamen SQL-Login Lese- und Schreibrechte.
-- Einmalig als Administrator ausführen (SSMS). Vorher <LOGIN> durch den Namen des gemeinsamen Logins ersetzen.
-- Der Login muss auf dem Server bereits existieren (CREATE LOGIN macht der DBA).

IF DB_ID(N'AiHarness') IS NULL CREATE DATABASE [AiHarness];
GO
IF DB_ID(N'AiHarness_Test') IS NULL CREATE DATABASE [AiHarness_Test];
GO

USE [AiHarness];
GO
IF USER_ID(N'<LOGIN>') IS NULL CREATE USER [<LOGIN>] FOR LOGIN [<LOGIN>];
ALTER ROLE db_datareader ADD MEMBER [<LOGIN>];
ALTER ROLE db_datawriter ADD MEMBER [<LOGIN>];
-- Additive Erweiterungen (00N_….sql ohne Versionssprung) spielt der Harness beim Verbinden selbst ein; dafür braucht er DDL.
-- Ohne dieses Recht laufen alle Befehle weiter, die die neuen Spalten nicht brauchen – nur diese melden das fehlende Skript.
ALTER ROLE db_ddladmin ADD MEMBER [<LOGIN>];
GO

USE [AiHarness_Test];
GO
IF USER_ID(N'<LOGIN>') IS NULL CREATE USER [<LOGIN>] FOR LOGIN [<LOGIN>];
ALTER ROLE db_datareader ADD MEMBER [<LOGIN>];
ALTER ROLE db_datawriter ADD MEMBER [<LOGIN>];
-- Additive Erweiterungen (00N_….sql ohne Versionssprung) spielt der Harness beim Verbinden selbst ein; dafür braucht er DDL.
-- Ohne dieses Recht laufen alle Befehle weiter, die die neuen Spalten nicht brauchen – nur diese melden das fehlende Skript.
ALTER ROLE db_ddladmin ADD MEMBER [<LOGIN>];
GO

-- Zeilenversionierung für Lesezugriffe: Schreiber sperren ihre Zeilen per UPDLOCK, Leser sehen die
-- zuletzt bestätigte Version statt zu warten. Das verhindert Deadlocks zwischen gleichzeitigen Schreibern
-- und Lesern. Auch auf bereits angelegten Datenbanken ausführen (WITH ROLLBACK IMMEDIATE trennt offene Verbindungen).
ALTER DATABASE [AiHarness] SET READ_COMMITTED_SNAPSHOT ON WITH ROLLBACK IMMEDIATE;
GO
ALTER DATABASE [AiHarness_Test] SET READ_COMMITTED_SNAPSHOT ON WITH ROLLBACK IMMEDIATE;
GO
