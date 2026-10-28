// Startet für den Vertragstest den Rust-Server vorn und den Node-Server dahinter – auf genau dem
// Weg, den auch `nexus serve` im Betrieb nimmt.
//
// `nexus serve` bindet den Port des Szenarios, startet den Vertragsserver (`contract/servers/
// ts-server.mjs`) als Sidecar auf einem freien internen Port und leitet alles dorthin, was Rust
// noch nicht selbst beantwortet. Beide teilen sich das HARNESS_HOME des Szenarios.
//
// Aufruf (aus der Projektwurzel):
//   node contract/run.mjs --server-cmd "node scripts/contract/rust-vor-node.mjs --port {port}"
//
// Voraussetzung: `npm run build` (Node-Server und Oberfläche) und
// `cargo build -p nexus-cli --bin nexus`.

import { spawn } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { join } from 'node:path';

const ROOT = fileURLToPath(new URL('../..', import.meta.url));

// Port aus `--port <port>`
function portFrom(args) {
  const at = args.indexOf('--port');
  const port = Number(at >= 0 ? args[at + 1] : process.env.HARNESS_PORT);
  if (!Number.isInteger(port) || port < 1 || port > 65535) {
    throw new Error('Bitte den Port angeben: node scripts/contract/rust-vor-node.mjs --port {port}');
  }
  return port;
}

const port = portFrom(process.argv.slice(2));

const rust = spawn(
  join(ROOT, 'target', 'debug', 'nexus'),
  [
    'serve',
    '--port',
    String(port),
    '--node-sidecar',
    join(ROOT, 'contract', 'servers', 'ts-server.mjs'),
    '--web-dir',
    join(ROOT, 'packages', 'web', 'dist'),
  ],
  {
    cwd: ROOT,
    env: {
      ...process.env,
      HARNESS_PORT: String(port),
      // Im Vertragsmodus meldet jede Umsetzung des Servers dieselbe Version
      NEXUS_SERVER_VERSION: 'contract',
    },
    stdio: ['ignore', 'inherit', 'inherit'],
  },
);

// Der Rust-Server nimmt den Node-Server beim Beenden selbst mit
const stop = (signal = 'SIGTERM') => {
  if (rust.exitCode === null && rust.signalCode === null) rust.kill(signal);
};
process.on('SIGTERM', () => stop());
process.on('SIGINT', () => stop());
rust.on('exit', (code) => process.exit(code ?? 0));
