import { SseEvents } from './SseEvents.mjs';
import { SseStream } from './SseStream.mjs';

// Frist eines `sseExpect`, wenn das Szenario keine nennt
const DEFAULT_TIMEOUT_MS = 3000;
// Name eines Stroms im Szenario (`as`, `stream`)
const STREAM_NAME = /^[A-Za-z][A-Za-z0-9_]*$/;

// Schritt-Art `sseOpen`: öffnet den Ereignisstrom eines Projekts und merkt ihn unter dem Namen aus `as`.
// Aufgezeichnet werden Status, Inhaltstyp und Cache-Vorgabe; ein unbekanntes Projekt ergibt einen
// ganz normalen Eintrag mit 404 und Fehlerkörper.
export class SseOpenStep {
  // Die Felder, die diese Art im Szenario kennt
  get keys() {
    return ['path', 'as'];
  }

  // Prüft die Felder des Schritts
  parse(value, index, fail) {
    if (typeof value.path !== 'string' || !value.path.startsWith('/')) fail(`steps[${index}].path muss mit / beginnen.`);
    if (typeof value.as !== 'string' || !STREAM_NAME.test(value.as)) {
      fail(`steps[${index}].as muss ein Name des Stroms sein (Buchstaben, Ziffern, _ – z. B. s1).`);
    }
    return { path: value.path, as: value.as };
  }

  // Öffnet den Strom; er bleibt offen, bis `sseClose` oder das Ende des Szenarios ihn abbricht
  async run(step, context) {
    if (context.streams.has(step.as)) throw new Error(`Der Strom „${step.as}“ ist schon offen.`);
    const path = context.variables.apply(step.path);
    const { stream, record } = await SseStream.open(step.as, `${context.base}${path}`);
    if (stream) context.streams.set(step.as, stream);
    return { request: { method: 'GET', path }, response: record };
  }
}

// Schritt-Art `sseExpect`: wartet auf das erste Ereignis, das auf `until` passt, und zeichnet alle
// Ereignisse seit dem letzten `sseExpect` dieses Stroms auf.
export class SseExpectStep {
  // Die Felder, die diese Art im Szenario kennt
  get keys() {
    return ['stream', 'until', 'timeoutMs'];
  }

  // Prüft die Felder des Schritts
  parse(value, index, fail) {
    if (typeof value.stream !== 'string' || !STREAM_NAME.test(value.stream)) {
      fail(`steps[${index}].stream muss der Name eines offenen Stroms sein (z. B. s1).`);
    }
    if (value.until === null || typeof value.until !== 'object' || Array.isArray(value.until) || !Object.keys(value.until).length) {
      fail(`steps[${index}].until muss ein Objekt mit mindestens einem Feld sein (z. B. { "type": "hello" }).`);
    }
    if (value.timeoutMs !== undefined && (!Number.isInteger(value.timeoutMs) || value.timeoutMs < 1)) {
      fail(`steps[${index}].timeoutMs muss eine ganze Zahl ab 1 sein.`);
    }
    return { stream: value.stream, until: value.until, timeoutMs: value.timeoutMs ?? DEFAULT_TIMEOUT_MS };
  }

  // Sammelt die Ereignisse bis zum Treffer; Herzschläge zählt der Takt, deshalb fallen sie weg,
  // solange nicht gerade auf einen Herzschlag gewartet wird
  async run(step, context) {
    const stream = streamOf(step.stream, context);
    const until = context.variables.apply(step.until);
    const entries = await stream.expect(until, step.timeoutMs);
    const kept = SseEvents.isComment(until) ? entries : SseEvents.withoutComments(entries);
    return { request: { stream: step.stream, until }, response: { events: SseEvents.fold(kept) } };
  }
}

// Schritt-Art `sseClose`: beendet die Verbindung. Dass der Server den Empfänger wirklich abmeldet,
// zeigt das Szenario danach selbst – es löst ein Ereignis aus, das ein anderer Strom noch bekommt.
export class SseCloseStep {
  // Die Felder, die diese Art im Szenario kennt
  get keys() {
    return ['stream'];
  }

  // Prüft die Felder des Schritts
  parse(value, index, fail) {
    if (typeof value.stream !== 'string' || !STREAM_NAME.test(value.stream)) {
      fail(`steps[${index}].stream muss der Name eines offenen Stroms sein (z. B. s1).`);
    }
    return { stream: value.stream };
  }

  // Bricht die Verbindung ab und gibt den Namen wieder frei
  async run(step, context) {
    streamOf(step.stream, context).close();
    context.streams.delete(step.stream);
    return { request: { stream: step.stream }, response: { closed: true } };
  }
}

// Der offene Strom zu einem Namen; fehlt er, ist das Szenario falsch gebaut
function streamOf(name, context) {
  const stream = context.streams.get(name);
  if (!stream) {
    throw new Error(`Es gibt keinen offenen Strom „${name}“ (sseOpen mit "as": "${name}" fehlt, war nicht erfolgreich oder sseClose kam schon).`);
  }
  return stream;
}
