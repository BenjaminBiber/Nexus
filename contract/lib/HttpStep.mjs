// Schritt-Art `http`: eine Anfrage an den Server, aufgezeichnet als Anfrage und Antwort.
export class HttpStep {
  // Eigene Felder der Art; der Lader meldet damit Tippfehler im Szenario
  get keys() {
    return ['method', 'path', 'body', 'headers'];
  }

  // Führt den Schritt aus; `variables` setzt `{{NAME}}` ein, `base` ist die Adresse des Servers
  async run(step, context) {
    const path = context.variables.apply(step.path);
    const body = step.body === undefined ? undefined : context.variables.apply(step.body);
    const declared = context.variables.apply(step.headers ?? {});
    const headers = HttpStep.headers(step.method, declared);
    const sends = body !== undefined && step.method !== 'GET' && step.method !== 'HEAD';
    const response = await fetch(`${context.base}${path}`, {
      method: step.method,
      headers,
      body: sends ? JSON.stringify(body) : undefined,
    });
    const request = { method: step.method, path };
    if (body !== undefined) request.body = body;
    // Nur die im Szenario genannten Köpfe werden aufgezeichnet; `null` zeigt, dass ein Standardkopf fehlen soll
    if (Object.keys(declared).length) request.headers = declared;
    return { request, response: await HttpStep.read(response) };
  }

  // Standardköpfe schreibender Anfragen; ein Kopf mit dem Wert `null` wird weggelassen
  static headers(method, declared) {
    const headers = method === 'GET' || method === 'HEAD' ? {} : { 'x-harness': '1', 'content-type': 'application/json' };
    for (const [name, value] of Object.entries(declared)) {
      if (value === null) delete headers[name.toLowerCase()];
      else headers[name.toLowerCase()] = String(value);
    }
    return headers;
  }

  // Liest die Antwort: Status, Inhaltstyp ohne Parameter, Körper als JSON oder Text
  static async read(response) {
    const type = response.headers.get('content-type');
    const record = { status: response.status, contentType: type === null ? null : type.split(';')[0].trim() };
    const text = await response.text();
    if (text === '') return { ...record, text: '' };
    try {
      return { ...record, json: JSON.parse(text) };
    } catch {
      // Zeilenenden vereinheitlichen, damit die Golden-Datei auf allen Plattformen gleich ist
      return { ...record, text: text.replace(/\r\n/g, '\n') };
    }
  }
}
