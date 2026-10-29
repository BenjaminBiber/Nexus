// Startet den Rust-Server für den Vertragstest: `nexus serve` mit dem Port des Szenarios und
// der gebauten Oberfläche. Der Vertragsmodus (`NEXUS_SERVER_VERSION=contract`) hält Claude,
// Worker und die Takte still; die Takte des Szenarios kommen über CONTRACT_*.
// Aufruf: node contract/servers/rust-server.mjs --port <port>
// Voraussetzung: `cargo build -p nexus-cli --bin nexus` und `npm run build`.
import { spawn } from 'node:child_process';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = fileURLToPath(new URL('../..', import.meta.url));
const BINARY = join(ROOT, 'target', 'debug', process.platform === 'win32' ? 'nexus.exe' : 'nexus');

// Port aus `--port <port>`, sonst aus HARNESS_PORT
function portFrom(args) {
  const at = args.indexOf('--port');
  const port = Number(at >= 0 ? args[at + 1] : process.env.HARNESS_PORT);
  if (!Number.isInteger(port) || port < 1 || port > 65535) {
    throw new Error('Bitte den Port angeben: node contract/servers/rust-server.mjs --port <port>');
  }
  return port;
}

const port = portFrom(process.argv.slice(2));

const server = spawn(
  BINARY,
  ['serve', '--port', String(port), '--web-dir', join(ROOT, 'packages', 'web', 'dist')],
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

const stop = (signal = 'SIGTERM') => {
  if (server.exitCode === null && server.signalCode === null) server.kill(signal);
};
process.on('SIGTERM', () => stop());
process.on('SIGINT', () => stop());
server.on('exit', (code, signal) => process.exit(code ?? (signal ? 1 : 0)));
