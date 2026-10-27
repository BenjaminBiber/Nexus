import { describe, expect, it } from 'vitest';
import { DependencyRules } from '../../../src/board/DependencyRules.js';
import type { TicketStatus } from '../../../src/board/BoardTypes.js';
import { ticketFixture } from './helpers/BoardDatabaseContract.js';

const t = (id: string, status: TicketStatus, dependsOn: string[] = []) =>
  ticketFixture({ id, projectId: 1, boardId: 1, number: Number(id.slice(4)), status, dependsOn });

describe('DependencyRules', () => {
  const rules = new DependencyRules();

  it('normalisiert: trimmt, vereinheitlicht, entfernt Doppelte, behält die Reihenfolge', () => {
    expect(rules.normalize([' arc-2 ', 'ARC-0001', 'ARC-0002', ''])).toEqual(['ARC-0002', 'ARC-0001']);
  });

  it('lehnt Selbstbezug ab', () => {
    const graph = new Map([['ARC-0001', [] as string[]]]);
    expect(() => rules.validate('ARC-0001', ['ARC-0001'], graph)).toThrow(expect.objectContaining({ code: 'dependency-self' }));
  });

  it('lehnt unbekannte IDs ab', () => {
    const graph = new Map([['ARC-0001', [] as string[]]]);
    expect(() => rules.validate('ARC-0001', ['ARC-0099'], graph)).toThrow(expect.objectContaining({ code: 'dependency-unknown' }));
  });

  it('lehnt einen Kreis mit Pfad in der Meldung ab', () => {
    const graph = new Map([['ARC-0001', ['ARC-0002']], ['ARC-0002', [] as string[]]]);
    expect(() => rules.validate('ARC-0002', ['ARC-0001'], graph)).toThrow('Abhängigkeiten liefen im Kreis: ARC-0002 -> ARC-0001 -> ARC-0002.');
  });

  it('nimmt eine gültige Liste an', () => {
    const graph = new Map([['ARC-0001', [] as string[]], ['ARC-0002', [] as string[]]]);
    expect(() => rules.validate('ARC-0002', ['ARC-0001'], graph)).not.toThrow();
  });

  it('erledigt, nicht mehr nötig und gelöscht gelten als erfüllt', () => {
    const a = t('ARC-0001', 'done');
    const b = t('ARC-0002', 'not_needed');
    const c = t('ARC-0003', 'in_progress');
    const d = t('ARC-0004', 'backlog', ['ARC-0001', 'ARC-0002', 'ARC-0003', 'ARC-0099']);
    const byId = new Map([a, b, c, d].map((x) => [x.id, x]));
    expect(rules.openDependencies(d, byId)).toEqual(['ARC-0003']);
    expect(rules.isWaiting(d, byId)).toBe(true);
  });

  it('wartet nur in Backlog, In Planung und Geplant', () => {
    const open = t('ARC-0001', 'backlog');
    const working = t('ARC-0002', 'in_progress', ['ARC-0001']);
    const blocked = t('ARC-0003', 'blocked', ['ARC-0001']);
    const byId = new Map([open, working, blocked].map((x) => [x.id, x]));
    expect(rules.isWaiting(working, byId)).toBe(false);
    expect(rules.isWaiting(blocked, byId)).toBe(false);
  });
});
