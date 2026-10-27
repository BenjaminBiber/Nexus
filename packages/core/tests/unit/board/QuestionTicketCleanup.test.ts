import { describe, expect, it } from 'vitest';
import type { BoardUnitOfWork } from '../../../src/board/BoardDatabase.js';
import { QuestionTicketCleanup } from '../../../src/board/QuestionTicketCleanup.js';

// Zeichnet die Zugriffe auf Fragen in Reihenfolge auf
function recordingUnit(referencing: string[]): { u: BoardUnitOfWork; calls: string[] } {
  const calls: string[] = [];
  const u = {
    questions: {
      idsReferencingTicket: async (ticketId: string) => {
        calls.push(`read ${ticketId}`);
        return referencing;
      },
      byId: async (id: string, forUpdate?: boolean) => {
        calls.push(`${forUpdate ? 'lock' : 'get'} ${id}`);
        return { id, rev: 1, updatedAt: 'x' };
      },
      removeTicket: async (ticketId: string) => {
        calls.push(`remove ${ticketId}`);
        return [...referencing].sort();
      },
      update: async (q: { id: string }) => {
        calls.push(`update ${q.id}`);
      },
    },
    questionHistory: { insert: async () => undefined },
  };
  return { u: u as unknown as BoardUnitOfWork, calls };
}

describe('QuestionTicketCleanup', () => {
  it('sperrt die betroffenen Fragen sortiert, bevor es die Bezüge entfernt (wie beim Setzen der Tickets)', async () => {
    const { u, calls } = recordingUnit(['ARC-F0002', 'ARC-F0001']);
    const actor = { userId: 'u-1', name: 'Florian' } as Parameters<QuestionTicketCleanup['remove']>[2];
    const affected = await new QuestionTicketCleanup().remove(u, { id: 'ARC-0001', title: 'Login' }, actor, '2026-01-01T00:00:00.000Z');
    expect(affected).toEqual(['ARC-F0001', 'ARC-F0002']);
    expect(calls.slice(0, 4)).toEqual(['read ARC-0001', 'lock ARC-F0001', 'lock ARC-F0002', 'remove ARC-0001']);
    // Danach frisch gelesen (ohne den entfernten Bezug), damit update die Kindzeilen nicht zurückschreibt
    expect(calls.slice(4)).toEqual(['lock ARC-F0001', 'update ARC-F0001', 'lock ARC-F0002', 'update ARC-F0002']);
  });

  it('ohne betroffene Fragen sperrt es nichts und schreibt keinen Verlauf', async () => {
    const { u, calls } = recordingUnit([]);
    const actor = { userId: 'u-1', name: 'Florian' } as Parameters<QuestionTicketCleanup['remove']>[2];
    expect(await new QuestionTicketCleanup().remove(u, { id: 'ARC-0001', title: 'Login' }, actor, 'x')).toEqual([]);
    expect(calls).toEqual(['read ARC-0001', 'remove ARC-0001']);
  });
});
