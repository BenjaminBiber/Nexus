import { BenchApi } from './BenchApi.mjs';
import { SseStream } from './SseStream.mjs';

// Zeit, die das Öffnen des Stroms („hello“) höchstens braucht
const HELLO_TIMEOUT_MS = 10000;
// Zeit, die ein Ereignis über den EventHub höchstens braucht
const EVENT_TIMEOUT_MS = 10000;
// Abfrage-Takt des BoardChangeWatcher im Server; er bestimmt die Latenz der dbboard-Meldung
const POLL_MS = 2500;
// Zeit, die eine dbboard-Meldung höchstens braucht (ein Takt plus Luft)
const DB_TIMEOUT_MS = POLL_MS * 3;

// Wartet eine Weile
const delay = (ms) => new Promise((done) => setTimeout(done, ms));

// Latenz des Ereignisstroms: von der Änderung bis zum Ereignis im Browser.
// `sessions` geht sofort über den EventHub, `dbboard` erst beim nächsten Durchlauf des BoardChangeWatcher.
export class SseBench {
  constructor({ url, projectId, repo, token, boardId, log = () => {} }) {
    this.api = new BenchApi(url);
    this.url = url;
    this.projectId = projectId;
    this.repo = repo;
    this.token = token;
    this.boardId = boardId;
    this.log = log;
  }

  // Adresse des Ereignisstroms des Projekts
  events(query = '') {
    return `${this.url}/api/projects/${this.projectId}/events${query}`;
  }

  // sessions: Hook-Ereignis melden und warten, bis `{"type":"sessions"}` im Strom ankommt
  async sessions(runs) {
    const stream = await SseStream.open(this.events());
    try {
      await stream.next('hello', HELLO_TIMEOUT_MS);
      const values = [];
      for (let run = 1; run <= runs; run += 1) {
        stream.drain();
        const arrival = stream.next('sessions', EVENT_TIMEOUT_MS);
        const begin = performance.now();
        await this.sendHook(`bench-sse-${run}`);
        values.push((await arrival) - begin);
      }
      this.log(`  ${values.length} sessions-Meldungen gemessen`);
      return values;
    } finally {
      stream.close();
    }
  }

  // dbboard: Ticket ändern und warten, bis `{"type":"dbboard"}` ankommt. Vor jeder Messung ein zufälliger Abstand,
  // damit die Änderungen nicht immer an derselben Stelle des Abfrage-Takts liegen.
  async dbBoard(runs, ticketId) {
    const ticket = await this.api.get(`/api/db/tickets/${ticketId}`);
    let rev = ticket?.rev;
    if (!Number.isInteger(rev)) throw new Error(`Das Ticket ${ticketId} hat keine Version (rev): ${JSON.stringify(ticket)}`);
    const stream = await SseStream.open(this.events(`?dbBoard=${this.boardId}`));
    try {
      await stream.next('hello', HELLO_TIMEOUT_MS);
      const values = [];
      for (let run = 1; run <= runs; run += 1) {
        await delay(Math.floor(Math.random() * POLL_MS));
        stream.drain();
        const arrival = stream.next('dbboard', DB_TIMEOUT_MS);
        const begin = performance.now();
        const changed = await this.api.send('PATCH', `/api/db/tickets/${ticketId}`, { rev, title: `Bench-Ticket, Lauf ${run} der SSE-Messung` });
        rev = changed.rev;
        values.push((await arrival) - begin);
      }
      this.log(`  ${values.length} dbboard-Meldungen gemessen (Abfrage-Takt des Servers: ${POLL_MS} ms)`);
      return values;
    } finally {
      stream.close();
    }
  }

  // Meldet ein Hook-Ereignis so, wie es die Hooks tun: Pfad des Projekts, Token im Kopf (siehe HookRoutes im Server)
  async sendHook(sessionId) {
    await this.api.send(
      'POST',
      '/api/hook',
      { project: this.repo, event: { kind: 'session-start', sessionId, cwd: this.repo } },
      { 'x-harness-token': this.token },
    );
  }
}
