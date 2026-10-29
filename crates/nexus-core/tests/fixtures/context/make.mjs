// Erzeugt die Vergleichstexte des Hook-Kontexts mit dem gebauten TypeScript-Paket.
// Einmal ausführen, die .txt-Dateien einchecken; der Rust-Test
// `tests/kontext_mit_ts.rs` baut dieselben Zustände und vergleicht Zeichen für Zeichen.
//
//   npx tsc -b packages/core
//   node crates/nexus-core/tests/fixtures/context/make.mjs
//
// Die Zeitzone steht fest auf UTC, damit die Kommentar-Zeitpunkte
// (`toLocaleString('de-DE')`) überall denselben Text ergeben.
process.env.TZ = 'UTC';

import { writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const dist = join(here, '..', '..', '..', '..', '..', 'packages', 'core', 'dist', 'index.js');
const { BoardRepoPaths, BoardServices, ContextBuilder, InMemoryBoardDatabase, MemoryBoardRepoStore } =
  await import(dist);

const USER = { id: '11111111-1111-1111-1111-111111111111', name: 'Florian' };
const NOW = new Date('2026-10-13T10:00:00.000Z');
const CLAUDE = { userId: USER.id, name: 'claude' };
const HUMAN = { userId: USER.id, name: 'Florian' };
const AGENT = { userId: USER.id, name: 'claude', kind: 'session', session: 's-1' };

// Board mit Benutzer, Projekt ARC und dem Board „Sprint“ auf einer Datenbank im Speicher
async function board() {
  let now = NOW;
  const clock = () => now;
  const db = new InMemoryBoardDatabase();
  const repoPaths = new BoardRepoPaths(new MemoryBoardRepoStore(), USER.id);
  await db.transaction((u) => u.users.upsert(USER));
  const services = new BoardServices(db, clock, repoPaths);
  await services.projects.create('ARC', 'Arches', CLAUDE);
  const boardId = (await services.boards.create('ARC', 'Sprint', CLAUDE)).id;
  return {
    boardId,
    tickets: services.tickets,
    comments: services.comments,
    questions: services.questions,
    builder: new ContextBuilder(services, null, clock),
    advance: (minutes) => (now = new Date(now.getTime() + minutes * 60_000)),
  };
}

// Der Zustand „kein Ticket“: nur Leitfaden und Ablauf
async function ohneTicket() {
  const b = await board();
  return (await b.builder.build('session-start', [])).text;
}

// Ein Ticket mit zwei ungelesenen Kommentaren eines Menschen
async function mitKommentaren() {
  const b = await board();
  await b.tickets.create(b.boardId, { title: 'Login', spec: '001-x', steps: [{ text: 'A', done: true }, { text: 'B', done: false }] }, CLAUDE);
  await b.tickets.update('ARC-0001', { status: 'in_progress' }, CLAUDE);
  await b.comments.add('ARC-0001', 'Bitte prüfen', HUMAN);
  b.advance(5);
  await b.comments.add('ARC-0001', 'Zweite Zeile\nmit Umbruch', HUMAN);
  return (await b.builder.build('session-start', ['ARC-0001'])).text;
}

// Ein Ticket mit einer offenen Frage
async function mitFrage() {
  const b = await board();
  await b.tickets.create(b.boardId, { title: 'Login' }, CLAUDE);
  await b.questions.create('ARC', { title: 'Welcher Speicher?', ticketIds: ['ARC-0001'] }, AGENT);
  return (await b.builder.build('session-start', ['ARC-0001'])).text;
}

for (const [name, build] of [
  ['ohne-ticket', ohneTicket],
  ['mit-kommentaren', mitKommentaren],
  ['mit-frage', mitFrage],
]) {
  const text = await build();
  writeFileSync(join(here, `${name}.txt`), text, 'utf8');
  console.log(`${name}.txt: ${text.length} Zeichen`);
}
