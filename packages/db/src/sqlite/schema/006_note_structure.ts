import type { SqliteSchemaStep } from './SqliteSchemaStep.js';

// Erweiterung note-structure (entspricht packages/db/sql/005_note_structure.sql): Titel neben dem Inhalt einer Notiz und
// wie oft derselbe Befund aufgefallen ist; Version bleibt 3
export const step006NoteStructure: SqliteSchemaStep = {
  id: '006_note_structure',
  feature: 'note-structure',
  sql: `
-- Titel: ein Satz, was auffällt (leer bei älteren Notizen)
ALTER TABLE Notes ADD COLUMN Title TEXT NOT NULL DEFAULT '';
-- Wie oft der Befund aufgefallen ist, und bei welchen weiteren Tickets (kommagetrennt)
ALTER TABLE Notes ADD COLUMN SeenCount INTEGER NOT NULL DEFAULT 1;
ALTER TABLE Notes ADD COLUMN SeenIn TEXT NOT NULL DEFAULT '';
`,
};
