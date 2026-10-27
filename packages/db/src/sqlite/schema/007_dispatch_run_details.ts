import type { SqliteSchemaStep } from './SqliteSchemaStep.js';

// Erweiterung dispatch-run-details (entspricht packages/db/sql/006_dispatch_run_details.sql): Limits, Grund der Profilwahl,
// Konfig-Version und abgelehnte Werkzeuge je Lauf, dazu die Tabelle der Konfig-Versionen; Version bleibt 3
export const step007DispatchRunDetails: SqliteSchemaStep = {
  id: '007_dispatch_run_details',
  feature: 'dispatch-run-details',
  sql: `
-- Schritt- und Zeitlimit, die für den Lauf galten
ALTER TABLE DispatchRuns ADD COLUMN MaxTurns INTEGER NULL;
ALTER TABLE DispatchRuns ADD COLUMN TimeoutMinutes INTEGER NULL;
-- Warum das Basisprofil gewählt wurde, und bei einer Regel deren 1-basierte Nummer
ALTER TABLE DispatchRuns ADD COLUMN ProfileReason TEXT NULL CONSTRAINT CK_DispatchRuns_ProfileReason CHECK (ProfileReason IN ('default', 'label', 'plan', 'rule', 'setting'));
ALTER TABLE DispatchRuns ADD COLUMN RuleIndex INTEGER NULL;
-- Version der Dispatcher-Einstellung, mit der der Lauf gestartet wurde
ALTER TABLE DispatchRuns ADD COLUMN ConfigHash TEXT NULL;
-- Abgelehnte Werkzeugaufrufe: Anzahl und die Werkzeuge kommagetrennt
ALTER TABLE DispatchRuns ADD COLUMN DeniedCount INTEGER NULL;
ALTER TABLE DispatchRuns ADD COLUMN DeniedTools TEXT NULL;
CREATE INDEX IX_DispatchRuns_Config ON DispatchRuns(ConfigHash);

-- Eine Zeile je gesehener Fassung der Dispatcher-Einstellung; Inhalt als JSON, damit das Dashboard Änderungen vergleichen kann
CREATE TABLE DispatchConfigVersions (
  Hash        TEXT NOT NULL CONSTRAINT PK_DispatchConfigVersions PRIMARY KEY,
  Dispatcher  TEXT NOT NULL,
  FirstSeenAt TEXT NOT NULL,
  Content     TEXT NOT NULL
);
CREATE INDEX IX_DispatchConfigVersions_FirstSeen ON DispatchConfigVersions(FirstSeenAt);
`,
};
