import { defineConfig } from 'vitest/config';
import { fileURLToPath } from 'node:url';

// Gemeinsame Auflösung: @harness/core zeigt in Tests direkt auf den Quellcode
const coreEntry = fileURLToPath(new URL('./packages/core/src/index.ts', import.meta.url));
const dbEntry = fileURLToPath(new URL('./packages/db/src/index.ts', import.meta.url));

export default defineConfig({
  resolve: { alias: { '@harness/core': coreEntry, '@harness/db': dbEntry } },
  esbuild: { jsx: 'automatic' },
  test: {
    // Eigenes HARNESS_HOME und unbenutzter Port je Testdatei; nie der echte ~/.harness oder Server
    setupFiles: ['./vitest.setup.ts'],
    include: [
      'packages/*/tests/**/*.test.{ts,tsx}',
      // Dazu die Einheitentests des Vertragstests (plain Node, deshalb .mjs)
      'contract/tests/**/*.test.mjs',
      // Dazu die Tests der Skripte (reines JavaScript, keine Paket-Importe – deshalb ohne Alias)
      'scripts/**/tests/**/*.test.mjs',
    ],
    // Prozessintensive Tests laufen getrennt und seriell (vitest.process.config.ts)
    exclude: [
      '**/node_modules/**',
      '**/concurrency/**',
      'packages/cli/tests/integration/board.test.ts',
      'packages/cli/tests/integration/board-db.test.ts',
      'packages/cli/tests/integration/board-question.test.ts',
      'packages/cli/tests/integration/board-work.test.ts',
      'packages/cli/tests/integration/board-spec.test.ts',
      // Tests gegen die gemeinsame Test-DB leeren deren Tabellen und laufen deshalb seriell (Prozess-Konfiguration)
      'packages/db/tests/**',
      'packages/cli/tests/integration/hook.test.ts',
      'packages/plugin/tests/**',
    ],
    // Integrationstests starten echte Prozesse (tsx, Server); unter Last braucht das Zeit
    testTimeout: 90000,
    hookTimeout: 90000,
    maxWorkers: 4,
  },
});
