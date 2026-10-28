import { createServer } from 'node:http';

// Köpfe, die aufgezeichnet werden; alle anderen (host, user-agent, accept-encoding) hängen am HTTP-Client
// und gehören nicht zum Vertrag
const RECORDED_HEADERS = ['content-type', 'authorization'];

// Attrappe eines fremden Dienstes (ntfy): nimmt jede Anfrage an, antwortet mit festem Status
// und zeichnet auf, was ankam.
export class HttpStub {
  // `server`: laufender Server, `requests`: die aufgezeichneten Anfragen in ihrer Reihenfolge
  constructor(server, requests) {
    this.server = server;
    this.requests = requests;
  }

  // Startet die Attrappe auf einem freien Port; `status` ist die Antwort auf jede Anfrage
  static async start(status = 200) {
    const requests = [];
    const server = createServer((request, response) => {
      const chunks = [];
      request.on('data', (chunk) => chunks.push(chunk));
      request.on('end', () => {
        requests.push(HttpStub.received(request, Buffer.concat(chunks).toString('utf8')));
        response.writeHead(status).end();
      });
    });
    await new Promise((resolve, reject) => {
      server.once('error', reject);
      server.listen(0, '127.0.0.1', resolve);
    });
    return new HttpStub(server, requests);
  }

  // Adresse der Attrappe (`http://127.0.0.1:<port>`)
  get url() {
    return `http://127.0.0.1:${this.server.address().port}`;
  }

  // Beendet die Attrappe; offene Verbindungen des Clients werden gekappt, sonst wartet `close` auf sie
  async close() {
    this.server.closeAllConnections();
    await new Promise((resolve) => this.server.close(resolve));
  }

  // Eine empfangene Anfrage: Methode, Pfad, die festen Köpfe und der Körper als JSON oder Text
  static received(request, body) {
    const headers = {};
    for (const name of RECORDED_HEADERS) {
      if (request.headers[name] !== undefined) headers[name] = request.headers[name];
    }
    const entry = { method: request.method, path: request.url, headers };
    try {
      return { ...entry, json: JSON.parse(body) };
    } catch {
      return { ...entry, text: body };
    }
  }
}
