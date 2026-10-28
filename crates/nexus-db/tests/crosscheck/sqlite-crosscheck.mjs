// Gegenstück der Gegenprobe auf der TypeScript-Seite: legt den festen Datensatz aus
// dataset.json über die Repositories von packages/db an und liest ihn wieder aus.
//
// Aufruf (immer mit Node >= 22.13, wegen node:sqlite):
//   node sqlite-crosscheck.mjs seed  <datei>
//   node sqlite-crosscheck.mjs dump  <datei>
//   node sqlite-crosscheck.mjs init  <datei>
//   node sqlite-crosscheck.mjs burst <datei> <anzahl> <präfix>
//   node sqlite-crosscheck.mjs bump  <datei> <ticketId> <rev>
//
// Nur stdout trägt das Ergebnis (kanonisches JSON, Schlüssel sortiert); Warnungen von Node
// („ExperimentalWarning“) gehen nach stderr. Das Skript arbeitet ausschließlich über die
// öffentliche Schnittstelle von packages/db/dist.

import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

const DIST = fileURLToPath(new URL('../../../../packages/db/dist/index.js', import.meta.url));
const DATASET = fileURLToPath(new URL('dataset.json', import.meta.url));

const { SqliteBoardDatabase } = await import(DIST);
const data = JSON.parse(readFileSync(DATASET, 'utf8'));

// Projekt und Board, mit denen init, burst und bump arbeiten
const BURST_PROJECT = { key: 'ARC', name: 'Arches', createdAt: '2026-10-13T10:00:00.000Z' };
const BURST_USER = data.users[0];

// JSON mit sortierten Schlüsseln: beide Seiten geben denselben Text aus
function canonical(value) {
  if (Array.isArray(value)) return value.map(canonical);
  if (value && typeof value === 'object') {
    return Object.fromEntries(Object.keys(value).sort().map((key) => [key, canonical(value[key])]));
  }
  return value;
}

// Legt den festen Datensatz an; jeder Abschnitt in seiner eigenen Transaktion, Reihenfolge wie in dataset.json
async function seed(db) {
  await db.transaction(async (u) => {
    for (const user of data.users) await u.users.upsert(user);
    for (const project of data.projects) {
      const angelegt = await u.projects.insert(project);
      const erwartet = data.projects.indexOf(project) + 1;
      if (angelegt.id !== erwartet) throw new Error(`Projekt ${project.key} bekam Id ${angelegt.id}, erwartet ${erwartet}`);
    }
    for (const board of data.boards) {
      const angelegt = await u.boards.insert(board);
      const erwartet = data.boards.indexOf(board) + 1;
      if (angelegt.id !== erwartet) throw new Error(`Board ${board.name} bekam Id ${angelegt.id}, erwartet ${erwartet}`);
    }
  });

  await db.transaction(async (u) => {
    for (const ticket of data.tickets) await u.tickets.insert(ticket);
    for (const { ticketId, steps } of data.steps) await u.steps.replace(ticketId, steps);
    for (const { ticketId, position, done } of data.stepsDone) await u.steps.setDone(ticketId, position, done);
    for (const { ticketId, sessionId, at } of data.sessions) await u.ticketSessions.add(ticketId, sessionId, at);
    for (const comment of data.comments) await u.comments.insert(comment);
    await u.history.insert(data.history);
    for (const note of data.notes) await u.notes.insert(note);
    for (const deletion of data.deletions) await u.deletions.insert(deletion);
  });

  await db.transaction(async (u) => {
    for (const question of data.questions) await u.questions.insert(question);
    for (const research of data.questionResearch) await u.questionResearch.insert(research);
    for (const comment of data.questionComments) await u.questionComments.insert(comment);
    await u.questionHistory.insert(data.questionHistory);
    for (const run of data.dispatchRuns) await u.dispatchRuns.insert(run);
    for (const version of data.configVersions) await u.dispatchRuns.recordConfigVersion(version);
  });
}

// Liest alles über die Repositories; die Reihenfolge der Listen ist die der Repositories
async function dump(db) {
  const ticketIds = data.tickets.map((t) => t.id);
  const questionIds = data.questions.map((q) => q.id);
  const boardIds = data.boards.map((_, i) => i + 1);
  const projectIds = data.projects.map((_, i) => i + 1);
  const filter = data.dispatchFilter;

  const inhalt = await db.transaction(async (u) => {
    const ergebnis = {
      users: await u.users.list(),
      userById: await u.users.byId(data.users[0].id),
      projects: await u.projects.list(),
      projectByKey: await u.projects.byKey(data.projects[0].key),
      boards: await u.boards.list(),
      boardsOfProject: await u.boards.list({ projectId: projectIds[0] }),
      boardsOfOwner: await u.boards.list({ ownerId: data.users[1].id }),
      tickets: await u.tickets.list(),
      ticketsWithoutTexts: await u.tickets.list({ withoutTexts: true }),
      ticketsOfBoard: await u.tickets.list({ boardIds: [boardIds[0]] }),
      ticketById: {},
      dependents: {},
      maxSortOrder: await u.tickets.maxSortOrder(boardIds[0], 'in_progress'),
      openCountByBoard: await u.tickets.openCountByBoard(boardIds),
      comments: {},
      commentCountsByBoard: await u.comments.countByBoard(boardIds[0]),
      unreadComments: await u.comments.unreadByAgent(ticketIds),
      steps: {},
      stepProgress: await u.steps.progress(ticketIds),
      sessionsByTicket: await u.ticketSessions.byTickets(ticketIds),
      ticketsOfSession: await u.ticketSessions.ticketsOf('sess-a'),
      history: {},
      notesByBoard: {},
      noteById: await u.notes.byId(1),
      deletions: await u.deletions.list(),
      questions: await u.questions.list(),
      questionById: {},
      questionsOfTicket: await u.questions.list({ ticketIds: [ticketIds[0]] }),
      openQuestions: await u.questions.list({ states: ['open'] }),
      questionIdsReferencingTicket: await u.questions.idsReferencingTicket(ticketIds[0]),
      research: {},
      researchSummaries: await u.questionResearch.summaries(questionIds),
      questionComments: {},
      questionHistory: {},
      dispatchRuns: await u.dispatchRuns.list(filter),
      dispatchRunsAll: await u.dispatchRuns.listAll(filter.from, filter.to),
      firstStartedDispatcher: await u.dispatchRuns.firstStarted(filter.projectId, 'dispatcher'),
      firstStartedComments: await u.dispatchRuns.firstStarted(filter.projectId, 'comments'),
      existingRuns: await u.dispatchRuns.existing([...data.dispatchRuns.map((r) => r.id), 'gibt-es-nicht']),
      configVersions: await u.dispatchRuns.listConfigVersions(filter.from, filter.to),
      doneTicketIds: await u.dispatchStats.doneTicketIds(filter),
      statusHistory: await u.dispatchStats.statusHistory(ticketIds),
      dispatchCommentCounts: await u.dispatchStats.dispatchCommentCounts(ticketIds),
      dispatchComments: await u.dispatchStats.dispatchComments(filter.projectId),
      search: await u.search.find(data.search),
    };
    for (const id of ticketIds) {
      ergebnis.ticketById[id] = await u.tickets.byId(id);
      ergebnis.dependents[id] = await u.tickets.dependents(id);
      ergebnis.comments[id] = await u.comments.list(id);
      ergebnis.steps[id] = await u.steps.list(id);
      ergebnis.history[id] = await u.history.list(id);
    }
    for (const id of boardIds) ergebnis.notesByBoard[id] = await u.notes.list({ boardId: id });
    for (const id of questionIds) {
      ergebnis.questionById[id] = await u.questions.byId(id);
      ergebnis.research[id] = await u.questionResearch.list(id);
      ergebnis.questionComments[id] = await u.questionComments.list(id);
      ergebnis.questionHistory[id] = await u.questionHistory.list(id);
    }
    return ergebnis;
  });

  inhalt.changeMarker = {};
  for (const id of boardIds) inhalt.changeMarker[id] = await db.changeMarker(id);
  inhalt.questionMarker = {};
  for (const id of projectIds) inhalt.questionMarker[id] = await db.questionMarker(id);
  return inhalt;
}

// Benutzer, Projekt ARC und ein Board – die Grundlage für burst und bump
async function init(db) {
  const boardId = await db.transaction(async (u) => {
    await u.users.upsert(BURST_USER);
    const projekt = await u.projects.insert({ ...BURST_PROJECT, createdBy: BURST_USER.id });
    const board = await u.boards.insert({
      projectId: projekt.id, ownerId: BURST_USER.id, name: 'Sprint', repoPath: null, createdAt: BURST_PROJECT.createdAt,
    });
    return board.id;
  });
  return { boardId };
}

// Legt <anzahl> Tickets an, jedes in einer eigenen Transaktion (wie der Dienst)
async function burst(db, count, prefix) {
  const ids = [];
  const errors = [];
  for (let i = 1; i <= count; i++) {
    try {
      const id = await db.transaction(async (u) => {
        const nummer = await u.projects.takeTicketNumber(1);
        const ticket = ticketFor(nummer, `${prefix} ${i}`);
        await u.tickets.insert(ticket);
        return ticket.id;
      });
      ids.push(id);
    } catch (error) {
      errors.push(error?.code ?? `unknown: ${error?.message}`);
    }
  }
  return { ids, errors };
}

// Ein Ticket mit Mindestangaben für burst
function ticketFor(nummer, titel) {
  return {
    id: `${BURST_PROJECT.key}-${String(nummer).padStart(4, '0')}`,
    projectId: 1, boardId: 1, number: nummer, title: titel, description: '',
    status: 'backlog', priority: 'normal', labels: [], dependsOn: [], sortOrder: nummer,
    assignee: null, needsPlanning: true, acceptance: '', plan: '', handbook: '', diagram: '',
    spec: null, specTask: null, branch: null, worktree: null, targetBranch: null,
    touches: [], consumes: [], produces: [], blockedReason: null,
    plannedAt: null, plannedBy: null, planningClaimBy: null, planningClaimAt: null,
    createdAt: BURST_PROJECT.createdAt, createdById: BURST_USER.id, createdByName: BURST_USER.name,
    updatedAt: BURST_PROJECT.createdAt, rev: 1,
  };
}

// Ein Update auf die angegebene Rev; Ergebnis 'ok' oder der Fehlercode
async function bump(db, id, rev) {
  try {
    await db.transaction(async (u) => {
      const ticket = await u.tickets.byId(id, true);
      if (!ticket) throw new Error(`Ticket ${id} fehlt`);
      await u.tickets.update({ ...ticket, title: 'Von TypeScript', rev });
    });
    return { result: 'ok' };
  } catch (error) {
    return { result: error?.code ?? `unknown: ${error?.message}` };
  }
}

const [command, file, ...rest] = process.argv.slice(2);
const db = await SqliteBoardDatabase.open(file);
let ausgabe;
try {
  if (command === 'seed') {
    await seed(db);
    ausgabe = { result: 'ok' };
  } else if (command === 'dump') {
    ausgabe = await dump(db);
  } else if (command === 'init') {
    ausgabe = await init(db);
  } else if (command === 'burst') {
    ausgabe = await burst(db, Number(rest[0]), rest[1]);
  } else if (command === 'bump') {
    ausgabe = await bump(db, rest[0], Number(rest[1]));
  } else {
    throw new Error(`Unbekannter Befehl ${command}`);
  }
} finally {
  await db.close();
}
process.stdout.write(`${JSON.stringify(canonical(ausgabe))}\n`);
