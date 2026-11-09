import { defineConfig } from 'vitest/config';

export default defineConfig({
  esbuild: { jsx: 'automatic' },
  test: {
    // Eigenes HARNESS_HOME und unbenutzter Port je Testdatei; nie der echte ~/.harness oder Server
    setupFiles: ['./vitest.setup.ts'],
    // Einmal je Lauf: lauf-eigene Ordner für den Plugin-Bau bereitstellen
    globalSetup: ['./vitest.globalSetup.ts'],
    include: [
      'packages/web/tests/**/*.test.{ts,tsx}',
      'packages/theme-sdk/tests/**/*.test.{ts,tsx}',
      'packages/plugin/tests/**/*.test.ts',
      // Dazu die Einheitentests des Vertragstests (plain Node, deshalb .mjs)
      'contract/tests/**/*.test.mjs',
      // Dazu die Tests der Skripte (reines JavaScript, keine Paket-Importe)
      'scripts/**/tests/**/*.test.mjs',
    ],
    exclude: ['**/node_modules/**'],
    // Die Plugin-Tests bauen das Plugin und starten echte Prozesse; unter Last braucht das Zeit
    testTimeout: 120000,
    hookTimeout: 120000,
    maxWorkers: 4,
  },
});
