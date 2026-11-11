import { afterEach, describe, expect, it, vi } from 'vitest';
import { moveDbCard } from '../src/dbboard/moveDbCard.js';
import { dbCard, dbView } from './helpers/fakeApi.js';

afterEach(() => vi.useRealTimers());

// Karten einer Spalte der Ansicht
const column = (view: ReturnType<typeof dbView>, id: string) => view.columns.find((c) => c.id === id)?.tickets ?? [];

describe('moveDbCard', () => {
  it('setzt beim Verschieben nach „Erledigt“ das Erledigt-Datum auf jetzt', () => {
    vi.useFakeTimers({ toFake: ['Date'] });
    vi.setSystemTime(new Date('2026-11-11T11:00:00.000Z'));
    const view = moveDbCard(dbView([dbCard('ARC-0001', 'Login bauen', 'in_progress')]), 'ARC-0001', 'done');
    expect(column(view, 'done').map((card) => [card.id, card.status, card.doneAt])).toEqual([['ARC-0001', 'done', '2026-11-11T11:00:00.000Z']]);
    expect(column(view, 'in_progress')).toEqual([]);
  });

  it('leert das Erledigt-Datum beim Verschieben aus „Erledigt“ heraus', () => {
    const view = moveDbCard(dbView([dbCard('ARC-0001', 'Login bauen', 'done', { doneAt: '2026-11-01T08:00:00.000Z' })]), 'ARC-0001', 'in_progress');
    expect(column(view, 'in_progress').map((card) => [card.id, card.doneAt])).toEqual([['ARC-0001', null]]);
  });

  it('lässt die Ansicht unverändert, wenn es die Karte nicht gibt', () => {
    const view = dbView([dbCard('ARC-0001', 'Login bauen', 'backlog')]);
    expect(moveDbCard(view, 'ARC-0099', 'done')).toBe(view);
  });
});
