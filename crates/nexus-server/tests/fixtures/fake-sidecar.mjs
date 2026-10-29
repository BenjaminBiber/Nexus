// Ersatz für den Node-Server in den Tests von NodeSidecar: ein kleiner HTTP-Server, der sich wie
// der echte Sidecar verhält – freier Port, Port-Zeile auf stdout, `/api/health` mit dem eigenen
// Namen, `/api/echo` als Nachweis, dass der Rust-Server wirklich hierher weiterleitet, und Ende,
// sobald die stdin endet (also sobald der Rust-Prozess weg ist).
// Schalter (nur für die Tests):
//   --silent     keine Port-Zeile ausgeben (Zeitlimit des Rust-Servers prüfen)
//   --no-health  Port melden, aber auf /api/health mit 500 antworten
import { createServer } from 'node:http';

const silent = process.argv.includes('--silent');
const noHealth = process.argv.includes('--no-health');

const server = createServer((request, response) => {
  if (request.url === '/api/health' && noHealth) {
    response.writeHead(500, { 'content-type': 'application/json' });
    response.end('{"code":"nicht-bereit"}');
    return;
  }
  const body =
    request.url === '/api/health'
      ? { name: 'ai-harness', version: 'fake' }
      : {
          url: request.url,
          method: request.method,
          port: server.address().port,
          sidecar: process.env.HARNESS_SIDECAR ?? null,
          publicPort: process.env.HARNESS_PUBLIC_PORT ?? null,
          home: process.env.HARNESS_HOME ?? null,
          dbQuestionPollMs: process.env.HARNESS_DB_QUESTION_POLL_MS ?? null,
          dispatchTickMs: process.env.HARNESS_DISPATCH_TICK_MS ?? null,
          args: process.argv.slice(2),
        };
  const text = JSON.stringify(body);
  response.writeHead(200, { 'content-type': 'application/json', 'content-length': Buffer.byteLength(text) });
  response.end(text);
});

// Der echte Sidecar nimmt seinen Port aus --port; 0 heißt „freier Port“
const at = process.argv.indexOf('--port');
server.listen(Number(at >= 0 ? process.argv[at + 1] : 0), '127.0.0.1', () => {
  if (!silent) process.stdout.write(`HARNESS_SIDECAR_PORT=${server.address().port}\n`);
});

// Endet die Pipe des Rust-Prozesses, endet auch dieser Prozess
const stop = () => process.exit(0);
process.stdin.on('end', stop);
process.stdin.on('close', stop);
process.stdin.resume();
process.on('SIGTERM', stop);
