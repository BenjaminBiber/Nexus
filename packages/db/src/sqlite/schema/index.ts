import { step001Initial } from './001_initial.js';
import { step002Questions } from './002_questions.js';
import { step003Harness } from './003_harness.js';
import { step004DispatchRuns } from './004_dispatch_runs.js';
import { step005RowVer } from './005_rowver.js';
import { step006NoteStructure } from './006_note_structure.js';
import { step007DispatchRunDetails } from './007_dispatch_run_details.js';
import { step008TargetBranch } from './008_target_branch.js';
import type { SqliteSchemaStep } from './SqliteSchemaStep.js';

export type { SqliteSchemaStep } from './SqliteSchemaStep.js';

// Alle Schema-Stufen der SQLite-Datei in Ausführungsreihenfolge; neue Stufen nur hinten anfügen
export const SQLITE_SCHEMA_STEPS: readonly SqliteSchemaStep[] = [step001Initial, step002Questions, step003Harness, step004DispatchRuns, step005RowVer, step006NoteStructure, step007DispatchRunDetails, step008TargetBranch];
