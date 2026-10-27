import { BenchApi } from './BenchApi.mjs';

// Spalten, auf die die Tickets verteilt werden; `null` heißt „bleibt im Backlog“.
// Ein Board mit Tickets in allen Spalten misst realistischer als 500 Karten in einer einzigen Spalte.
const COLUMNS = [null, 'planned', 'in_progress', 'blocked', 'done'];
// Labels, Prioritäten und Beschreibungstexte wechseln sich ab, damit die Ansicht nicht aus Kopien besteht
const PRIORITIES = ['low', 'normal', 'high', 'urgent'];
const LABELS = [['backend'], ['oberflaeche', 'ux'], ['messung'], [], ['migration', 'rust']];
// Jedes so vielte Ticket bekommt Kommentare
const COMMENT_EVERY = 5;
const COMMENTS_PER_TICKET = 2;

// Legt über die HTTP-API ein Board mit vielen Tickets an: Board-Konfig (SQLite), Projekt, Board, Tickets, Spalten, Kommentare.
// Schreibende Anfragen brauchen `X-Harness: 1` und `application/json` (siehe RequestGuard im Server).
export class BoardSeeder {
  constructor(url, { dbFile, userId, userName, log = () => {} }) {
    this.api = new BenchApi(url);
    this.url = url;
    this.dbFile = dbFile;
    this.userId = userId;
    this.userName = userName;
    this.log = log;
  }

  // Richtet das Board ein und liefert Nummer des Boards, Zahl der Tickets und das erste Ticket (für die SSE-Messung)
  async seed(count) {
    await this.send('PUT', '/api/board-config', {
      provider: 'sqlite',
      dbFile: this.dbFile,
      userId: this.userId,
      userName: this.userName,
    });
    await this.send('POST', '/api/db/projects', { key: 'BEN', name: 'Bench' });
    const board = await this.send('POST', '/api/db/boards', { project: 'BEN', name: 'Bench' });
    if (!Number.isInteger(board?.id)) throw new Error(`Das Board wurde ohne Nummer angelegt: ${JSON.stringify(board)}`);
    const firstTicket = await this.fill(board.id, count);
    return { boardId: board.id, tickets: count, firstTicket };
  }

  // Legt `count` Tickets an, verteilt sie auf die Spalten und kommentiert einige davon; liefert das erste Ticket
  async fill(boardId, count) {
    const every = Math.max(1, Math.floor(count / 10));
    let first = null;
    for (let index = 0; index < count; index += 1) {
      const ticket = await this.send('POST', `/api/db/boards/${boardId}/tickets`, {
        title: `Bench-Ticket ${index + 1}: ${this.titleTail(index)}`,
        description: `Messticket ${index + 1} von ${count}. Angelegt vom Messskript, damit die Board-Ansicht eine realistische Menge Karten enthält.`,
        priority: PRIORITIES[index % PRIORITIES.length],
        labels: LABELS[index % LABELS.length],
        acceptance: 'Das Ticket steht in der Ansicht.',
        needsPlanning: index % 3 === 0,
      });
      const status = COLUMNS[index % COLUMNS.length];
      if (status) {
        await this.send('POST', `/api/db/tickets/${ticket.id}/move`, { status, acceptance: 'Vom Messskript gesetzt.' });
      }
      if (index % COMMENT_EVERY === 0) {
        for (let nr = 1; nr <= COMMENTS_PER_TICKET; nr += 1) {
          await this.send('POST', `/api/db/tickets/${ticket.id}/comments`, { body: `Kommentar ${nr} des Messskripts zu ${ticket.id}.` });
        }
      }
      if (!first) first = ticket.id;
      if ((index + 1) % every === 0 || index + 1 === count) this.log(`  ${index + 1}/${count} Tickets angelegt`);
    }
    return first;
  }

  // Wechselnde Titelenden, damit Suche und Sortierung nicht auf identischen Texten arbeiten
  titleTail(index) {
    const parts = ['Kaltstart prüfen', 'Ansicht laden', 'Speicher beobachten', 'Vertrag festhalten', 'Messreihe wiederholen'];
    return parts[index % parts.length];
  }

  // Schickt eine schreibende Anfrage und liefert die geparste Antwort; ab Status 400 ein Fehler mit Antworttext
  async send(method, path, body) {
    return this.api.send(method, path, body);
  }
}
