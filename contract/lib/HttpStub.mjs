import { createServer } from 'node:http';

// Köpfe, die aufgezeichnet werden; alle anderen (host, user-agent, accept-encoding) hängen am HTTP-Client
// und gehören nicht zum Vertrag
const RECORDED_HEADERS = ['content-type', 'authorization'];

// Attrappe eines fremden Dienstes (ntfy, Spracherkennung): nimmt jede Anfrage an, antwortet mit festem
// Status und festem Körper und zeichnet auf, was ankam.
export class HttpStub {
  // `server`: laufender Server, `requests`: die aufgezeichneten Anfragen in ihrer Reihenfolge
  constructor(server, requests) {
    this.server = server;
    this.requests = requests;
  }

  // Startet die Attrappe auf einem freien Port; `status` ist die Antwort auf jede Anfrage, `body` ihr Körper
  // (`undefined` = leer, ein Text kommt wie er ist, jeder andere Wert als JSON)
  static async start(status = 200, body = undefined) {
    const requests = [];
    const answer = HttpStub.answer(body);
    const server = createServer((request, response) => {
      const chunks = [];
      request.on('data', (chunk) => chunks.push(chunk));
      request.on('end', () => {
        requests.push(HttpStub.received(request, Buffer.concat(chunks).toString('utf8')));
        response.writeHead(status, answer.headers).end(answer.text);
      });
    });
    await new Promise((resolve, reject) => {
      server.once('error', reject);
      server.listen(0, '127.0.0.1', resolve);
    });
    return new HttpStub(server, requests);
  }

  // Körper und Inhaltstyp der Antwort: ohne `body` bleibt sie leer, ein Text geht als `text/plain`,
  // jeder andere Wert als JSON
  static answer(body) {
    if (body === undefined) return { headers: {}, text: undefined };
    if (typeof body === 'string') return { headers: { 'content-type': 'text/plain; charset=utf-8' }, text: body };
    return { headers: { 'content-type': 'application/json' }, text: JSON.stringify(body) };
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
