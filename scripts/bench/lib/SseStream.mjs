// Liest einen Ereignisstrom (Server-Sent Events) und merkt sich, wann jedes Ereignis angekommen ist.
// Blöcke werden an `\n\n` getrennt, Zeilen mit `data:` als JSON gelesen, Kommentare (`: ping`) übergangen –
// wie in packages/server/tests/helpers/LiveServer.ts.
export class SseStream {
  constructor(controller, reader) {
    this.controller = controller;
    this.reader = reader;
    this.rest = '';
    // Angekommene, noch nicht abgeholte Ereignisse: { at, event }
    this.received = [];
    // Wartende Abholer: { type, done, fail, timer }
    this.waiters = [];
  }

  // Öffnet den Strom und beginnt zu lesen
  static async open(url) {
    const controller = new AbortController();
    const res = await fetch(url, { signal: controller.signal, headers: { accept: 'text/event-stream' } });
    if (!res.ok || !res.body) throw new Error(`Der Ereignisstrom ${url} antwortet mit ${res.status}.`);
    const stream = new SseStream(controller, res.body.getReader());
    void stream.pump();
    return stream;
  }

  // Wartet auf das nächste Ereignis dieses Typs und liefert den Zeitpunkt seines Eintreffens (performance.now).
  // Vor dem Auslösen einer Änderung aufrufen, damit kein Ereignis zwischen Auslösen und Warten verloren geht.
  next(type, timeoutMs) {
    const ready = this.received.findIndex((entry) => entry.event?.type === type);
    if (ready !== -1) return Promise.resolve(this.received.splice(ready, 1)[0].at);
    return new Promise((done, fail) => {
      const waiter = { type, done, fail, timer: null };
      waiter.timer = setTimeout(() => {
        this.waiters = this.waiters.filter((other) => other !== waiter);
        fail(new Error(`Kein Ereignis „${type}“ innerhalb von ${timeoutMs} ms.`));
      }, timeoutMs);
      this.waiters.push(waiter);
    });
  }

  // Verwirft alles, was bisher angekommen ist (vor jeder Messung: sonst zählt ein älteres Ereignis)
  drain() {
    this.received = [];
  }

  // Schließt den Strom; wartende Abholer bleiben ohne Folgen
  close() {
    this.waiters.forEach((waiter) => clearTimeout(waiter.timer));
    this.waiters = [];
    this.controller.abort();
  }

  // Liest fortlaufend; jedes Ereignis bekommt den Zeitpunkt des Datenpakets, in dem es ankam
  async pump() {
    const decoder = new TextDecoder();
    try {
      for (;;) {
        const { done, value } = await this.reader.read();
        if (done) return;
        const at = performance.now();
        this.rest += decoder.decode(value, { stream: true });
        const blocks = this.rest.split('\n\n');
        // Der letzte Teil kann unvollständig sein und wartet auf das nächste Paket
        this.rest = blocks.pop() ?? '';
        for (const block of blocks) this.accept(block, at);
      }
    } catch {
      /* abgebrochen oder Server beendet */
    }
  }

  // Nimmt die `data:`-Zeilen eines Blocks an; unlesbares JSON wird übergangen
  accept(block, at) {
    for (const line of block.split('\n')) {
      if (!line.startsWith('data:')) continue;
      let event;
      try {
        event = JSON.parse(line.slice(5));
      } catch {
        continue;
      }
      this.deliver({ at, event });
    }
  }

  // Stellt ein Ereignis einem passenden Wartenden zu, sonst legt es sich ab
  deliver(entry) {
    const index = this.waiters.findIndex((waiter) => waiter.type === entry.event?.type);
    if (index === -1) {
      this.received.push(entry);
      return;
    }
    const [waiter] = this.waiters.splice(index, 1);
    clearTimeout(waiter.timer);
    waiter.done(entry.at);
  }
}
