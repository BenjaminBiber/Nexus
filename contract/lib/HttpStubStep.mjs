import { HttpStub } from './HttpStub.mjs';

const ACTIONS = new Set(['start', 'requests', 'await']);
const STUB_NAME = /^[A-Z][A-Z0-9_]*$/;

// Schritt-Art `httpStub`: startet eine Attrappe eines fremden Dienstes (`action: "start"`), wartet auf
// Anfragen an sie (`action: "await"`) und zeichnet auf, was der Server an sie geschickt hat
// (`action: "requests"`). Gedacht für ntfy, die Spracherkennung und die Bot-API von Telegram.
export class HttpStubStep {
  // Eigene Felder der Art; der Lader meldet damit Tippfehler im Szenario
  get keys() {
    return ['action', 'stub', 'status', 'body', 'routes', 'call', 'count'];
  }

  // Führt den Schritt aus; `context.stubs` hält die Attrappen des Szenarios (der Runner beendet sie)
  async run(step, context) {
    const { action = 'start', stub, status = 200, body, routes, call, count } = step.raw;
    if (!ACTIONS.has(action)) throw new Error(`action erlaubt nur ${[...ACTIONS].join(', ')}.`);
    if (typeof stub !== 'string' || !STUB_NAME.test(stub)) throw new Error('stub muss ein Name in Großbuchstaben sein (z. B. NTFY).');
    if (!Number.isInteger(status) || status < 100 || status > 599) throw new Error('status muss ein HTTP-Status sein.');
    if (body !== undefined && action !== 'start') throw new Error('body gehört zu action: "start".');
    if (routes !== undefined && action !== 'start') throw new Error('routes gehört zu action: "start".');
    if ((call !== undefined || count !== undefined) && action !== 'await') throw new Error('call und count gehören zu action: "await".');
    if (action === 'start') return this.start(stub, status, body, routes, context);
    if (action === 'await') return this.await(stub, call, count, context);
    return this.requests(stub, context);
  }

  // Startet die Attrappe; ihre Adresse steht als `json.url` für `save` bereit. `routes` beantwortet
  // einzelne Aufrufe eigens; die Werte gehen vorher durch die Variablen des Szenarios
  async start(name, status, body, routes, context) {
    if (context.stubs.has(name)) throw new Error(`Die Attrappe ${name} läuft schon.`);
    const applied = routes === undefined ? undefined : context.variables.apply(routes);
    const stub = await HttpStub.start(status, body === undefined ? undefined : context.variables.apply(body), applied);
    context.stubs.set(name, stub);
    const request = { stub: name, action: 'start', status };
    if (body !== undefined) request.body = body;
    if (routes !== undefined) request.routes = Object.keys(applied);
    return { request, response: { json: { url: stub.url } } };
  }

  // Wartet, bis die Attrappe die erwarteten Anfragen gesehen hat, und zeichnet ihre Zahl auf
  async await(name, call = null, count = 1, context) {
    const stub = this.stub(name, context);
    if (call !== undefined && call !== null && typeof call !== 'string') throw new Error('call muss der Name eines Aufrufs sein (z. B. getMe).');
    if (!Number.isInteger(count) || count < 1) throw new Error('count muss eine ganze Zahl ab 1 sein.');
    // Aufgezeichnet wird nur, dass gewartet wurde: wie viele Anfragen bis dahin wirklich ankamen,
    // hängt am Takt des Servers und wäre in jedem Lauf anders
    await stub.waitFor(call ?? null, count);
    return { request: { stub: name, action: 'await', call: call ?? null, count }, response: { json: { ok: true } } };
  }

  // Zeichnet auf, was die Attrappe bis hierher empfangen hat
  requests(name, context) {
    const stub = this.stub(name, context);
    return { request: { stub: name, action: 'requests' }, response: { json: stub.requests } };
  }

  // Die laufende Attrappe mit diesem Namen
  stub(name, context) {
    const stub = context.stubs.get(name);
    if (!stub) throw new Error(`Die Attrappe ${name} läuft nicht – vorher ein Schritt mit action: "start".`);
    return stub;
  }
}
