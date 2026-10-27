// Einstieg des Sidecars: „serve“ startet den Server, --dispatch-worker einen Worker, alles andere ist die CLI.
// Das Bündel ist CommonJS, weil die Single Executable Application (SEA) nur CommonJS als Einstieg kennt –
// deshalb kein Top-Level-await, sondern .catch(). Die beiden Zweige sind dynamische Importe, damit esbuild
// sie verzögert auswertet und nur der gewählte Teil hochläuft.

// Im SEA ist process.argv = [binary, aufrufzeichenkette, ...argumente]; slice(2) in Server und CLI passt also
// unverändert. Fehlt die zweite Stelle (anderer Einbettungsweg), wird sie ergänzt.
if (process.argv.length < 2) process.argv.splice(1, 0, process.execPath);

const args = process.argv.slice(2);
const serve = args[0] === 'serve';
// „serve“ ist nur die Wahl des Teils und darf den Server nicht als Argument erreichen
if (serve) process.argv.splice(2, 1);

const part = serve || args.includes('--dispatch-worker')
  ? import('../../packages/server/dist/main.js')
  : import('../../packages/cli/dist/main.js');

part.catch((error) => {
  console.error(error instanceof Error ? error.message : String(error));
  process.exit(1);
});
