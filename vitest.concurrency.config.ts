import { defineConfig } from 'vitest/config';
import { fileURLToPath } from 'node:url';

// Nebenläufigkeitstests mit Kindprozessen, bewusst getrennt und mit langem Zeitlimit
const coreEntry = fileURLToPath(new URL('./packages/core/src/index.ts', import.meta.url));
const dbEntry = fileURLToPath(new URL('./packages/db/src/index.ts', import.meta.url));

export default defineConfig({
  resolve: { alias: { '@harness/core': coreEntry, '@harness/db': dbEntry } },
  test: {
    // Eigenes HARNESS_HOME und unbenutzter Port je Testdatei; nie der echte ~/.harness oder Server
    setupFiles: ['./vitest.setup.ts'],
    include: ['packages/*/tests/concurrency/**/*.test.ts'],
    testTimeout: 120000,
  },
});
