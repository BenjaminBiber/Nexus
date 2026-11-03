import { HttpStub } from './HttpStub.mjs';

const ACTIONS = new Set(['start', 'requests']);
const STUB_NAME = /^[A-Z][A-Z0-9_]*$/;

// Schritt-Art `httpStub`: startet eine Attrappe eines fremden Dienstes (`action: "start"`) und zeichnet
// später auf, was der Server an sie geschickt hat (`action: "requests"`). Gedacht für ntfy und die
// Spracherkennung.
export class HttpStubStep {
  // Eigene Felder der Art; der Lader meldet damit Tippfehler im Szenario
  get keys() {
    return ['action', 'stub', 'status', 'body'];
  }

  // Führt den Schritt aus; `context.stubs` hält die Attrappen des Szenarios (der Runner beendet sie)
  async run(step, context) {
    const { action = 'start', stub, status = 200, body } = step.raw;
    if (!ACTIONS.has(action)) throw new Error(`action erlaubt nur ${[...ACTIONS].join(', ')}.`);
    if (typeof stub !== 'string' || !STUB_NAME.test(stub)) throw new Error('stub muss ein Name in Großbuchstaben sein (z. B. NTFY).');
    if (!Number.isInteger(status) || status < 100 || status > 599) throw new Error('status muss ein HTTP-Status sein.');
    if (body !== undefined && action !== 'start') throw new Error('body gehört zu action: "start".');
    return action === 'start' ? this.start(stub, status, body, context) : this.requests(stub, context);
  }

  // Startet die Attrappe; ihre Adresse steht als `json.url` für `save` bereit
  async start(name, status, body, context) {
    if (context.stubs.has(name)) throw new Error(`Die Attrappe ${name} läuft schon.`);
    const stub = await HttpStub.start(status, body);
    context.stubs.set(name, stub);
    const request = { stub: name, action: 'start', status };
    if (body !== undefined) request.body = body;
    return { request, response: { json: { url: stub.url } } };
  }

  // Zeichnet auf, was die Attrappe bis hierher empfangen hat
  requests(name, context) {
    const stub = context.stubs.get(name);
    if (!stub) throw new Error(`Die Attrappe ${name} läuft nicht – vorher ein Schritt mit action: "start".`);
    return { request: { stub: name, action: 'requests' }, response: { json: stub.requests } };
  }
}
