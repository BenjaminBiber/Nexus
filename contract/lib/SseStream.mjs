import { HttpStep } from './HttpStep.mjs';
import { SseEvents } from './SseEvents.mjs';
import { SseParser } from './SseParser.mjs';

// Abstand zwischen zwei Blicken in die gesammelten Einträge
const WAIT_MS = 10;

// Eine offene Verbindung zum Ereignisstrom eines Projekts: liest im Hintergrund mit, sammelt die Einträge
// und gibt sie bis einschließlich des ersten Treffers von `until` heraus.
export class SseStream {
  constructor(name, response, controller) {
    this.name = name;
    this.response = response;
    this.controller = controller;
    this.parser = new SseParser();
    this.entries = [];
    this.ended = false;
  }

  // Öffnet den Strom und liefert `{ stream, record }`: `record` hält Status, Inhaltstyp und Cache-Vorgabe.
  // Antwortet der Server nicht mit einem Ereignisstrom (z. B. 404 für ein unbekanntes Projekt), bleibt
  // `stream` null und der Körper wird wie bei einer normalen Anfrage aufgezeichnet.
  static async open(name, url) {
    const controller = new AbortController();
    const response = await fetch(url, { signal: controller.signal, headers: { accept: 'text/event-stream' } });
    const cacheControl = response.headers.get('cache-control');
    const type = response.headers.get('content-type');
    // Inhaltstyp wie bei `http` ohne Parameter (`; charset=…` entfällt)
    const contentType = type === null ? null : type.split(';')[0].trim();
    if (contentType !== 'text/event-stream' || !response.body) {
      const record = { ...(await HttpStep.read(response)), cacheControl };
      controller.abort();
      return { stream: null, record };
    }
    const stream = new SseStream(name, response, controller);
    void stream.read();
    return { stream, record: { status: response.status, contentType, cacheControl } };
  }

  // Liest im Hintergrund weiter, bis der Server den Strom beendet oder `close` ihn abbricht
  async read() {
    try {
      const reader = this.response.body.getReader();
      const decoder = new TextDecoder();
      for (;;) {
        const { done, value } = await reader.read();
        if (done) break;
        this.entries.push(...this.parser.push(decoder.decode(value, { stream: true })));
      }
    } catch {
      // Abbruch durch `close` oder Ende des Servers: beides ist kein Fehler des Szenarios
    }
    this.ended = true;
  }

  // Alle Einträge seit dem letzten Aufruf bis einschließlich des ersten Treffers von `until`;
  // ohne Treffer innerhalb der Frist ein Fehler mit allem, was bisher angekommen ist
  async expect(until, timeoutMs) {
    const deadline = Date.now() + timeoutMs;
    for (;;) {
      const at = this.entries.findIndex((entry) => SseEvents.matches(entry, until));
      if (at >= 0) return this.entries.splice(0, at + 1);
      const wanted = JSON.stringify(until);
      if (this.ended) throw new Error(`Der Strom „${this.name}“ ist zu Ende, ohne ${wanted} zu melden.${this.seen()}`);
      if (Date.now() >= deadline) throw new Error(`Der Strom „${this.name}“ hat ${wanted} nicht innerhalb von ${timeoutMs} ms gemeldet.${this.seen()}`);
      await new Promise((done) => setTimeout(done, WAIT_MS));
    }
  }

  // Bricht die Verbindung ab; ein offener Strom hält sonst den Prozess von `run.mjs` am Leben
  close() {
    this.controller.abort();
  }

  // Was bisher angekommen und noch nicht herausgegeben ist (für Fehlermeldungen)
  seen() {
    if (!this.entries.length) return ' Angekommen ist nichts.';
    return ` Angekommen: ${this.entries.map((entry) => JSON.stringify(entry)).join(', ')}.`;
  }
}
