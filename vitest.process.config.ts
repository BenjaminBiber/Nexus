import { defineConfig } from 'vitest/config';
import { fileURLToPath } from 'node:url';

// Prozessintensive Tests (echter Server, gebautes Plugin): Dateien laufen nacheinander, damit sie
// sich nicht gegenseitig ausbremsen und möglichst wenige Kindprozesse gleichzeitig entstehen.
const coreEntry = fileURLToPath(new URL('./packages/core/src/index.ts', import.meta.url));
const dbEntry = fileURLToPath(new URL('./packages/db/src/index.ts', import.meta.url));

export const PROCESS_TESTS = [
  'packages/cli/tests/integration/board.test.ts',
  'packages/cli/tests/integration/board-db.test.ts',
  'packages/cli/tests/integration/board-question.test.ts',
  'packages/cli/tests/integration/board-work.test.ts',
  // Gemeinsame Test-DB: Dateien dürfen sich beim Leeren der Tabellen nicht überschneiden
  'packages/db/tests/**/*.test.ts',
  'packages/cli/tests/integration/hook.test.ts',
  'packages/plugin/tests/**/*.test.ts',
];

export default defineConfig({
  resolve: { alias: { '@harness/core': coreEntry, '@harness/db': dbEntry } },
  test: {
    // Eigenes HARNESS_HOME und unbenutzter Port je Testdatei; nie der echte ~/.harness oder Server
    setupFiles: ['./vitest.setup.ts'],
    // Einmal je Lauf: Vorbedingungen prüfen (Build, Test-DB) und lauf-eigene Ordner bereitstellen
    globalSetup: ['./vitest.process.setup.ts'],
    include: PROCESS_TESTS,
    fileParallelism: false,
    testTimeout: 120000,
    hookTimeout: 120000,
  },
});
