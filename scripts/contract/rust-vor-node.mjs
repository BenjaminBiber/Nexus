// Startet für den Vertragstest den Rust-Server vorn und den Node-Server dahinter.
//
// Der Rust-Server beantwortet, was in Rust schon umgesetzt ist (heute /api/health und die
// Oberfläche), und leitet alles andere an den Node-Server weiter. Beide teilen sich das
// HARNESS_HOME des Szenarios, damit der Zustand derselbe ist.
//
// Aufruf (aus der Projektwurzel):
//   node contract/run.mjs --server-cmd "node scripts/contract/rust-vor-node.mjs --port {port}"
//
// Voraussetzung: `npm run build` (Node-Server und Oberfläche) und
// `cargo build -p nexus-server --bin nexus-server`.

import { spawn } from 'node:child_process';
import { createServer } from 'node:net';
import { fileURLToPath } from 'node:url';
import { join } from 'node:path';

const ROOT = fileURLToPath(new URL('../..', import.meta.url));

// Port aus `--port <port>`
function portFrom(args) {
  const at = args.indexOf('--port');
  const port = Number(at >= 0 ? args[at + 1] : process.env.HARNESS_PORT);
  if (!Number.isInteger(port) || port < 1 || port > 65535) {
    throw new Error('Bitte den Port angeben: node scripts/contract/rust-vor-node.mjs --port <port>');
  }
  return port;
}

// Freier Port auf 127.0.0.1 für den Node-Server dahinter
function freePort() {
  return new Promise((done, fail) => {
    const probe = createServer();
    probe.on('error', fail);
    probe.listen(0, '127.0.0.1', () => {
      const { port } = probe.address();
      probe.close(() => done(port));
    });
  });
}

// Wartet höchstens 30 s, bis `GET /api/health` des Servers hinter dem Proxy antwortet
async function waitForHealth(target, child) {
  const until = Date.now() + 30_000;
  while (Date.now() < until) {
    if (child.exitCode !== null || child.signalCode !== null) {
      throw new Error(`Der Node-Server dahinter hat sich beendet (Code ${child.exitCode}).`);
    }
    try {
      const response = await fetch(`http://127.0.0.1:${target}/api/health`);
      if (response.ok) {
        await response.arrayBuffer();
        return;
      }
    } catch {
      /* noch nicht da */
    }
    await new Promise((done) => setTimeout(done, 100));
  }
  throw new Error(`Der Node-Server auf Port ${target} antwortet nicht.`);
}

const port = portFrom(process.argv.slice(2));
const upstreamPort = await freePort();

// Erst der Node-Server dahinter …
const node = spawn(process.execPath, [join(ROOT, 'contract', 'servers', 'ts-server.mjs'), '--port', String(upstreamPort)], {
  cwd: ROOT,
  env: { ...process.env, HARNESS_PORT: String(upstreamPort) },
  stdio: ['ignore', 'inherit', 'inherit'],
});

// Warten, bis der Node-Server antwortet: der Vertragslauf prüft nur den Server vorn, und der
// wäre sonst schon bereit, während die ersten Anfragen den Upstream noch nicht erreichen.
await waitForHealth(upstreamPort, node);

// … dann der Rust-Server davor. Er schreibt server.json als Letzter, damit dort der Port
// steht, über den die Oberfläche und die Hooks hereinkommen.
const rust = spawn(join(ROOT, 'target', 'debug', 'nexus-server'), ['--port', String(port), '--upstream', `http://127.0.0.1:${upstreamPort}`], {
  cwd: ROOT,
  env: {
    ...process.env,
    HARNESS_PORT: String(port),
    // Im Vertragsmodus meldet jede Umsetzung des Servers dieselbe Version
    NEXUS_SERVER_VERSION: 'contract',
    NEXUS_WEB_DIR: join(ROOT, 'packages', 'web', 'dist'),
  },
  stdio: ['ignore', 'inherit', 'inherit'],
});

// Beide Prozesse gehören zusammen: endet einer, endet auch der andere
const stop = (signal = 'SIGTERM') => {
  for (const child of [rust, node]) {
    if (child.exitCode === null && child.signalCode === null) child.kill(signal);
  }
};
process.on('SIGTERM', () => stop());
process.on('SIGINT', () => stop());
rust.on('exit', (code) => {
  stop();
  process.exit(code ?? 0);
});
node.on('exit', (code) => {
  if (rust.exitCode === null && rust.signalCode === null) {
    console.error(`Der Node-Server dahinter hat sich beendet (Code ${code}).`);
  }
  stop();
});
