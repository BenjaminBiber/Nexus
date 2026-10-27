import { BoardServices, HarnessError } from '@harness/core';
import { SqliteBoardDatabase } from '../../../src/sqlite/SqliteBoardDatabase.js';

// Hilfsprozess für den Mehrprozess-Test; Ergebnis als JSON auf stdout.
// Aufruf: multiProcessWorker.ts create <datei> <boardId> <anzahl> <präfix>
//         multiProcessWorker.ts update <datei> <ticketId> <rev> <titel>
const actor = { userId: '11111111-1111-1111-1111-111111111111', name: 'worker' };

// Code eines Fehlers: Harness-Code oder „unknown: <Meldung>“
const codeOf = (error: unknown) => (error instanceof HarnessError ? error.code : `unknown: ${(error as Error).message}`);

// Legt nacheinander <anzahl> Tickets an und sammelt Ids und Fehlercodes
async function create(services: BoardServices, boardId: number, count: number, prefix: string): Promise<unknown> {
  const ids: string[] = [];
  const errors: string[] = [];
  for (let i = 1; i <= count; i++) {
    try {
      const ticket = await services.tickets.create(boardId, { title: `${prefix} ${i}` }, actor);
      ids.push(ticket.id);
    } catch (error) {
      errors.push(codeOf(error));
    }
  }
  return { ids, errors };
}

// Ändert den Titel auf der angegebenen Rev; Ergebnis ist 'ok' oder der Fehlercode
async function update(services: BoardServices, id: string, rev: number, title: string): Promise<unknown> {
  try {
    await services.tickets.update(id, { title }, actor, { rev });
    return { result: 'ok' };
  } catch (error) {
    return { result: codeOf(error) };
  }
}

async function main(): Promise<void> {
  const [command, file, ...rest] = process.argv.slice(2);
  const db = await SqliteBoardDatabase.open(file);
  const services = new BoardServices(db);
  const output = command === 'create'
    ? await create(services, Number(rest[0]), Number(rest[1]), rest[2])
    : await update(services, rest[0], Number(rest[1]), rest[2]);
  await db.close();
  console.log(JSON.stringify(output));
}

await main();
