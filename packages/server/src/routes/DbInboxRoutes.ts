import {
  HarnessError, INBOX_KEY_PATTERN, InboxOrder, InboxOrderFile, MAX_INBOX_KEY_LENGTH, MAX_INBOX_KEYS, type HarnessHome,
} from '@harness/core';
import type { FastifyInstance } from 'fastify';
import type { BoardRuntimeHolder } from '../dbboard/BoardRuntimeHolder.js';
import { DbInput } from '../dbboard/DbInput.js';

// /api/db/inbox: „Meine Aufgaben“ über alle eigenen Boards, mit der lokal gespeicherten Reihenfolge des Benutzers.
// Aktionen (Abnehmen, erneut Mergen, Notiz übernehmen) laufen über die vorhandenen Endpunkte.
export class DbInboxRoutes {
  private readonly input = new DbInput();
  private readonly order = new InboxOrder();
  private readonly file: InboxOrderFile;

  constructor(private readonly holder: BoardRuntimeHolder, home: HarnessHome) {
    this.file = new InboxOrderFile(home);
  }

  register(app: FastifyInstance): void {
    app.get('/api/db/inbox', async (request) => {
      const { board } = request.query as { board?: string };
      const boardId = board === undefined || board === '' ? undefined : this.input.id(board, 'Board');
      return this.holder.run(async (b) => {
        const items = await b.services.inbox.list(b.actor().userId, { boardId });
        return b.services.inbox.summarize(this.order.apply(items, await this.file.read()));
      });
    });
    app.put('/api/db/inbox/order', async (request) => {
      const keys = this.keys(this.input.body(request.body).keys);
      await this.file.write(keys);
      return { keys };
    });
    app.delete('/api/db/inbox/order', async () => {
      await this.file.clear();
      return { keys: [] };
    });
  }

  // Schlüssel der Reihenfolge: Liste mit höchstens MAX_INBOX_KEYS gültigen Schlüsseln
  private keys(raw: unknown): string[] {
    if (!Array.isArray(raw) || raw.length > MAX_INBOX_KEYS) {
      throw new HarnessError('input-invalid', `Feld „keys“ muss eine Liste mit höchstens ${MAX_INBOX_KEYS} Schlüsseln sein.`);
    }
    for (const key of raw) {
      if (typeof key !== 'string' || key.length > MAX_INBOX_KEY_LENGTH || !INBOX_KEY_PATTERN.test(key)) {
        throw new HarnessError('input-invalid', `„${String(key).slice(0, MAX_INBOX_KEY_LENGTH)}“ ist kein gültiger Schlüssel (z. B. ticket:AIH-0001).`);
      }
    }
    return raw as string[];
  }
}
