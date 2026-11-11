// Board-Stand des Servers: tolerant annehmen, Unsinn verwerfen
import { describe, expect, it } from 'vitest';
import { parseBoardView } from '../src/data/types.js';

const card = (over: Record<string, unknown> = {}) => ({
  id: 'NOVA-0001',
  title: 'Warenkorb',
  column: 'in_progress',
  blockedReason: null,
  stepsDone: 1,
  stepsTotal: 3,
  priority: 'high',
  assignee: 'junior-NOVA-0001',
  description: 'wird nicht gebraucht',
  ...over,
});

describe('parseBoardView', () => {
  it('übernimmt Spalten und Karten mit den gebrauchten Feldern', () => {
    const view = parseBoardView({ boardId: 7, columns: [{ id: 'in_progress', title: 'In Arbeit', tickets: [card()] }] });
    expect(view).toEqual({
      boardId: 7,
      columns: [
        {
          id: 'in_progress',
          title: 'In Arbeit',
          tickets: [
            {
              id: 'NOVA-0001',
              title: 'Warenkorb',
              column: 'in_progress',
              blockedReason: null,
              stepsDone: 1,
              stepsTotal: 3,
              priority: 'high',
              assignee: 'junior-NOVA-0001',
            },
          ],
        },
      ],
    });
  });

  it('lässt unbekannte Spalten und kaputte Karten weg', () => {
    const view = parseBoardView({
      boardId: 1,
      columns: [
        { id: 'archiv', title: 'Archiv', tickets: [card()] },
        { id: 'blocked', title: 'Blockiert', tickets: [card({ id: 42 }), null, 'x', card({ title: undefined }), card({ id: 'NOVA-0002', column: 'blocked', blockedReason: 'Zugang fehlt' })] },
        { id: 'done', title: 'Erledigt' },
        null,
      ],
    });
    expect(view?.columns.map((c) => c.id)).toEqual(['blocked', 'done']);
    expect(view?.columns[0]!.tickets.map((t) => t.id)).toEqual(['NOVA-0002']);
    expect(view?.columns[0]!.tickets[0]!.blockedReason).toBe('Zugang fehlt');
    expect(view?.columns[1]!.tickets).toEqual([]);
  });

  it('füllt fehlende oder falsche Nebenfelder mit sinnvollen Werten', () => {
    const view = parseBoardView({
      boardId: 1,
      columns: [{ id: 'planned', title: 'Geplant', tickets: [card({ column: 'quatsch', stepsDone: 'zwei', stepsTotal: -1, priority: 3, assignee: 5, blockedReason: 7 })] }],
    });
    expect(view?.columns[0]!.tickets[0]).toMatchObject({ column: 'planned', stepsDone: 0, stepsTotal: 0, priority: 'normal', assignee: null, blockedReason: null });
  });

  it('liefert bei Unsinn null', () => {
    for (const json of [null, undefined, 'x', 3, [], {}, { boardId: 'a', columns: [] }, { boardId: 1 }, { boardId: 1, columns: {} }]) {
      expect(parseBoardView(json), JSON.stringify(json)).toBeNull();
    }
  });
});
