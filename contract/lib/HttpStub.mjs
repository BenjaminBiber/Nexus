import { createServer } from 'node:http';

// Köpfe, die aufgezeichnet werden; alle anderen (host, user-agent, accept-encoding) hängen am HTTP-Client
// und gehören nicht zum Vertrag
const RECORDED_HEADERS = ['content-type', 'authorization'];

// In jeder Adresse der Bot-API von Telegram steht das Bot-Token (`/bot123456:AA…/getMe`). Es darf in keiner
// Golden-Datei stehen, deshalb wird es im aufgezeichneten Pfad zu `<TOKEN>`.
const SECRET_IN_PATH = /\/bot\d+:[A-Za-z0-9_-]{20,}/g;

// Attrappe eines fremden Dienstes (ntfy, Spracherkennung, Telegram): nimmt jede Anfrage an, antwortet mit
// festem Status und festem Körper und zeichnet auf, was ankam.
//
// Dienste mit mehreren Aufrufen (Telegram: `getMe`, `getUpdates`, `sendMessage`) brauchen je Aufruf eine
// eigene Antwort; dafür gibt es `routes`.
export class HttpStub {
  // `server`: laufender Server, `requests`: die aufgezeichneten Anfragen in ihrer Reihenfolge
  constructor(server, requests) {
    this.server = server;
    this.requests = requests;
  }

  // Startet die Attrappe auf einem freien Port; `status` ist die Antwort auf jede Anfrage, `body` ihr Körper
  // (`undefined` = leer, ein Text kommt wie er ist, jeder andere Wert als JSON).
  //
  // `routes` beantwortet einzelne Aufrufe eigens: Schlüssel ist das letzte Stück des Pfades (`getMe`), Wert
  // ein Objekt mit `status`, `body` oder `bodies` (Antworten in ihrer Reihenfolge, die letzte wiederholt sich)
  // und `delayMs` (so lange wartet die Attrappe – für das Long Polling von `getUpdates`, damit der Server
  // nicht ohne Pause fragt).
  static async start(status = 200, body = undefined, routes = {}) {
    const requests = [];
    const base = { status, answers: [HttpStub.answer(body)], delayMs: 0, used: 0 };
    const planned = HttpStub.plan(status, routes);
    const server = createServer((request, response) => {
      const chunks = [];
      request.on('data', (chunk) => chunks.push(chunk));
      request.on('end', () => {
        const received = HttpStub.received(request, Buffer.concat(chunks).toString('utf8'));
        requests.push(received);
        const route = planned.get(HttpStub.callOf(request.url)) ?? base;
        const answer = route.answers[Math.min(route.used, route.answers.length - 1)];
        route.used += 1;
        const send = () => response.writeHead(route.status, answer.headers).end(answer.text);
        if (route.delayMs > 0) setTimeout(send, route.delayMs).unref();
        else send();
      });
    });
    await new Promise((resolve, reject) => {
      server.once('error', reject);
      server.listen(0, '127.0.0.1', resolve);
    });
    return new HttpStub(server, requests);
  }

  // Die Antworten je Aufruf, vorbereitet wie die Standardantwort
  static plan(status, routes) {
    const planned = new Map();
    for (const [name, route] of Object.entries(routes)) {
      if (route === null || typeof route !== 'object' || Array.isArray(route)) {
        throw new Error(`routes["${name}"] muss ein Objekt sein.`);
      }
      if (route.body !== undefined && route.bodies !== undefined) throw new Error(`routes["${name}"] nennt body und bodies.`);
      if (route.bodies !== undefined && (!Array.isArray(route.bodies) || route.bodies.length === 0)) {
        throw new Error(`routes["${name}"].bodies muss eine nicht leere Liste sein.`);
      }
      const bodies = route.bodies ?? [route.body];
      planned.set(name, {
        status: route.status ?? status,
        answers: bodies.map((entry) => HttpStub.answer(entry)),
        delayMs: route.delayMs ?? 0,
        used: 0,
      });
    }
    return planned;
  }

  // Der aufgerufene Dienst: das letzte Stück des Pfades ohne Abfrageteil (`/bot…/getMe?x=1` → `getMe`)
  static callOf(url) {
    const path = (url ?? '').split('?')[0];
    const parts = path.split('/').filter((part) => part !== '');
    return parts.length ? parts[parts.length - 1] : '';
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

  // Wartet, bis die Attrappe `count` Anfragen an `call` gesehen hat (ohne Angabe an irgendeinen Pfad).
  // Damit wird ein Szenario unabhängig davon, wie schnell eine Hintergrundaufgabe des Servers anläuft.
  async waitFor(call = null, count = 1, timeoutMs = 10_000) {
    const matches = () => this.requests.filter((entry) => call === null || HttpStub.callOf(entry.path) === call).length;
    const until = Date.now() + timeoutMs;
    while (matches() < count) {
      if (Date.now() > until) {
        throw new Error(`Die Attrappe sah ${matches()} von ${count} Anfragen an ${call ?? 'irgendeinen Pfad'}.`);
      }
      await new Promise((resolve) => setTimeout(resolve, 20));
    }
    return matches();
  }

  // Beendet die Attrappe; offene Verbindungen des Clients werden gekappt, sonst wartet `close` auf sie
  async close() {
    this.server.closeAllConnections();
    await new Promise((resolve) => this.server.close(resolve));
  }

  // Eine empfangene Anfrage: Methode, Pfad (ohne Geheimnis), die festen Köpfe und der Körper als JSON oder Text
  static received(request, body) {
    const headers = {};
    for (const name of RECORDED_HEADERS) {
      if (request.headers[name] !== undefined) headers[name] = request.headers[name];
    }
    const entry = { method: request.method, path: request.url.replace(SECRET_IN_PATH, '/bot<TOKEN>'), headers };
    try {
      return { ...entry, json: JSON.parse(body) };
    } catch {
      return { ...entry, text: body };
    }
  }
}
