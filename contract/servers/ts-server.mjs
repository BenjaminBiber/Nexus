import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { HarnessHome, ServerInfoFile } from '@harness/core';
import { SqliteWarningFilter } from '@harness/db';
import { ServerApp } from '../../packages/server/dist/ServerApp.js';

const REPO_ROOT = fileURLToPath(new URL('../..', import.meta.url));

// Antwort eines Claude-Aufrufs, der nie wirklich startet
const claudeResult = (result) => ({
  code: 0,
  timedOut: false,
  output: JSON.stringify({ type: 'result', subtype: 'success', is_error: false, result, total_cost_usd: 0 }),
});

// Startet den TypeScript-Server im Vertragsmodus: echter Port, feste Attrappen statt Dialog, Explorer,
// Claude-Aufrufen und Worker-Start. Aufruf: node contract/servers/ts-server.mjs --port <port>
async function main() {
  SqliteWarningFilter.install();
  const port = portFrom(process.argv.slice(2));
  const home = new HarnessHome(process.env);
  const poll = Number(process.env.CONTRACT_POLL_MS ?? 2500);
  const app = await ServerApp.create({
    port,
    host: '127.0.0.1',
    home,
    webDir: join(REPO_ROOT, 'packages', 'web', 'dist'),
    version: 'contract',
    heartbeatMs: Number(process.env.CONTRACT_HEARTBEAT_MS ?? 15000),
    dbPollMs: poll,
    docsPollMs: Number(process.env.CONTRACT_POLL_MS ?? 10000),
    // keine Meldungen über neue Fragen und kein Takt des Dispatchers
    dbQuestionPollMs: 0,
    dispatchTickMs: 0,
    // nie einen Worker starten
    dispatchLauncher: { launch: () => null },
    // nie einen echten Ordnerdialog öffnen
    folderPicker: { pick: async () => null },
    // nie Explorer oder VS Code starten
    fileOpener: { open: async () => undefined },
    // nie die echte Anmeldung lesen oder Anthropic fragen
    claudeUsage: {
      current: async () => ({ available: false, reason: 'no-login', plan: null, fiveHour: null, sevenDay: null, fetchedAt: null }),
    },
    // nie das echte Claude starten: Vorschläge und Ticket-Entwürfe sind leer, das Umschreiben liefert „Ersatztext“
    suggestionClaude: { run: async () => claudeResult('[]') },
    ticketDraftClaude: { run: async () => claudeResult('[]') },
    reviseClaude: { run: async () => claudeResult('Ersatztext') },
    log: (line) => process.stderr.write(`${line}\n`),
  });
  try {
    await app.listen({ port, host: '127.0.0.1' });
  } catch (error) {
    console.error(error.code === 'EADDRINUSE' ? `Port ${port} ist belegt.` : error.message);
    process.exit(1);
  }
  const actual = app.server.address().port;
  app.guard.allowPort(actual);
  const info = new ServerInfoFile(home);
  await info.write({ pid: process.pid, port: actual, startedAt: new Date().toISOString() });
  console.log(`Vertragsserver auf http://127.0.0.1:${actual} (HARNESS_HOME=${home.dir})`);

  const stop = async () => {
    await app.close();
    await info.remove();
    process.exit(0);
  };
  process.on('SIGINT', stop);
  process.on('SIGTERM', stop);
}

// Port aus `--port <port>`, sonst aus HARNESS_PORT
function portFrom(args) {
  const at = args.indexOf('--port');
  const raw = at >= 0 ? args[at + 1] : process.env.HARNESS_PORT;
  const port = Number(raw);
  if (!Number.isInteger(port) || port < 1 || port > 65535) {
    throw new Error('Bitte den Port angeben: node contract/servers/ts-server.mjs --port <port>');
  }
  return port;
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : error);
  process.exit(1);
});
