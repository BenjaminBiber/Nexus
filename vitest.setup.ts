import { mkdtempSync } from 'node:fs';
import { createServer } from 'node:net';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach } from 'vitest';

// Eigene Nexus-Umgebung für jeden Testlauf: Kein Test (und kein Kindprozess, der die Umgebung erbt) darf den echten
// `~/.harness` oder einen laufenden Server auf dem Standardport erreichen. Tests, die eigene Werte brauchen, setzen sie
// danach selbst (diese Datei läuft vor jeder Testdatei).
// Dazu eine eigene, leere Konfiguration von Claude Code (CLAUDE_CONFIG_DIR), damit `nexus init` und der Start eines
// Worktrees in Tests nie die echte ~/.claude.json des Rechners ändern. Gestartete CLI-Prozesse erben die Variablen.

const DEFAULT_PORT = 4710; // DEFAULT_SERVER_PORT; bewusst nicht importiert, damit die Einrichtung nichts vom Quellcode lädt

// Freier hoher Port, auf dem niemand lauscht: Meldungen an „den Server“ laufen ins Leere statt zum echten Server
const freePort = (): Promise<number> =>
  new Promise((done, fail) => {
    const server = createServer();
    server.on('error', fail);
    server.listen(0, '127.0.0.1', () => {
      const { port } = server.address() as { port: number };
      server.close(() => done(port));
    });
  });

process.env.HARNESS_HOME = mkdtempSync(join(tmpdir(), 'harness-test-home-'));
process.env.HARNESS_PORT = String(await freePort());
process.env.CLAUDE_CONFIG_DIR = mkdtempSync(join(tmpdir(), 'harness-claude-config-'));
// Aus einer Claude-Code-Session geerbte Zuordnung gilt nicht für Tests
delete process.env.HARNESS_SESSION;
delete process.env.HARNESS_TICKET;

// Oberflächentests: findBy… und waitFor warten 5 s statt 1 s. Auf langsamen CI-Runnern braucht der
// erste Aufbau einer Seite (Laden, Markdown, nachgeladene Daten) sonst gelegentlich zu lange.
if (typeof document !== 'undefined') {
  const { configure } = await import('@testing-library/react');
  configure({ asyncUtilTimeout: 5000 });
}

// Sicherheitsnetz im Prozess: Anfragen an den Standardport werden nie ausgeführt, sondern lassen den Test scheitern.
// Der Fehler ist kein ECONNREFUSED, damit der Hook den Port als „fremd“ ansieht und keinen Server startet.
const blocked: string[] = [];
const realFetch = globalThis.fetch;
globalThis.fetch = (async (input: Parameters<typeof fetch>[0], init?: RequestInit) => {
  const url = typeof input === 'string' ? input : input instanceof URL ? input.href : input.url;
  if (new URL(url).port === String(DEFAULT_PORT)) {
    blocked.push(url);
    throw new TypeError('fetch failed', { cause: { code: 'HARNESS_TEST_BLOCKED' } });
  }
  return realFetch(input, init);
}) as typeof fetch;

afterEach(() => {
  if (!blocked.length) return;
  const urls = blocked.splice(0);
  throw new Error(`Test hat den echten Nexus-Server auf Port ${DEFAULT_PORT} angesprochen: ${urls.join(', ')}`);
});
