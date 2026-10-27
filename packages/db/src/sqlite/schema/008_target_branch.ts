import type { SqliteSchemaStep } from './SqliteSchemaStep.js';

// Erweiterung target-branch (entspricht packages/db/sql/007_target_branch.sql): Zielbranch eines Tickets für Start und Merge;
// null = der im Hauptordner ausgecheckte Branch. Version bleibt 3
export const step008TargetBranch: SqliteSchemaStep = {
  id: '008_target_branch',
  feature: 'target-branch',
  sql: `
ALTER TABLE Tickets ADD COLUMN TargetBranch TEXT NULL;
`,
};
