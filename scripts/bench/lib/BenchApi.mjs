// Standardzeit, die eine Anfrage höchstens braucht
const TIMEOUT_MS = 30000;

// Anfragen an den gemessenen Server. Schreibende Anfragen brauchen `X-Harness: 1` und `application/json`
// (siehe RequestGuard im Server); ab Status 400 gibt es einen Fehler mit dem Antworttext.
export class BenchApi {
  constructor(url, { timeoutMs = TIMEOUT_MS } = {}) {
    this.url = url;
    this.timeoutMs = timeoutMs;
  }

  // Lesende Anfrage; liefert die geparste Antwort
  async get(path) {
    return this.request('GET', path, undefined, {});
  }

  // Schreibende Anfrage mit Pflichtkopf; liefert die geparste Antwort (null bei 204)
  async send(method, path, body, headers = {}) {
    return this.request(method, path, body ?? {}, { 'content-type': 'application/json', 'x-harness': '1', ...headers });
  }

  // Eine Anfrage samt vollständig gelesener Antwort
  async request(method, path, body, headers) {
    const res = await fetch(`${this.url}${path}`, {
      method,
      headers,
      body: body === undefined ? undefined : JSON.stringify(body),
      signal: AbortSignal.timeout(this.timeoutMs),
    });
    const text = await res.text();
    if (res.status >= 400) throw new Error(`${method} ${path} antwortet mit ${res.status}: ${text.slice(0, 300)}`);
    return text ? JSON.parse(text) : null;
  }
}
