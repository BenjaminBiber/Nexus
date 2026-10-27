import { describe, expect, it } from 'vitest';
import { DispatchStatusFormatter } from '../../src/board/DispatchStatusFormatter.js';
import type { DispatchStatusView } from '../../src/board/DispatchClient.js';

const BASE: DispatchStatusView = {
  enabled: true, state: 'running', message: 'läuft · 1/1', pausedUntil: null, wip: 1, active: 1,
  runs: [{ id: 'ARC-0009-20261015-090000', kind: 'work', ticketId: 'ARC-0009', title: 'Rabatte', profile: 'standard', model: 'opus', effort: 'high', startedAt: '2026-10-15T07:00:00.000Z', log: 'ARC-0009-20261015-090000.log' }],
  merges: [{ ticketId: 'ARC-0003', phase: 'waiting', branch: 'ticket/ARC-0003-x' }],
  boardId: 1, boardInScope: true, tickets: [{ id: 'ARC-0010', title: 'Neu', reason: 'bereit – wartet auf freien Platz' }, { id: 'ARC-0011', title: 'Ohne', reason: 'Kein Plan' }],
  ticketsError: null, lastTick: '2026-10-15T07:01:00.000Z', lastErrors: ['ARC-0012: claude fehlt'], configFile: 'C:/h/dispatch.yaml', configError: null, agentPrefix: 'junior-',
};

describe('DispatchStatusFormatter', () => {
  it('fasst Zustand, Läufe, Merges, Tickets und Fehler zusammen', () => {
    const lines = new DispatchStatusFormatter().lines(BASE);
    expect(lines[0]).toBe('Dispatcher: läuft · 1/1');
    expect(lines).toContain('  ARC-0009  Umsetzung, Profil standard (opus/high), Protokoll ARC-0009-20261015-090000.log');
    expect(lines).toContain('  ARC-0003  wartet auf weitere Tickets (ticket/ARC-0003-x)');
    expect(lines).toContain('  ARC-0010  bereit – wartet auf freien Platz');
    expect(lines).toContain('  ARC-0011  Kein Plan');
    expect(lines).toContain('  ARC-0012: claude fehlt');
    expect(lines.at(-1)).toBe('Einstellung: C:/h/dispatch.yaml');
  });

  it('merge nennt das Ergebnis für das Ticket statt der Übersicht; ohne Ergebnis (älterer Server) die Übersicht', () => {
    const formatter = new DispatchStatusFormatter();
    expect(formatter.mergeLines({ ...BASE, result: { outcome: 'merged', text: 'ARC-0012 gemergt in main (abc1234).' } })).toEqual(['ARC-0012 gemergt in main (abc1234).']);
    expect(formatter.mergeLines({ ...BASE, result: { outcome: 'blocked', text: 'ARC-0012: Merge blockiert – lokale Änderungen im Hauptordner: a.txt' } }))
      .toEqual(['ARC-0012: Merge blockiert – lokale Änderungen im Hauptordner: a.txt']);
    expect(formatter.mergeLines(BASE)[0]).toBe('Dispatcher: läuft · 1/1');
  });

  it('zeigt Recherche-Läufe als „Recherche <Frage>“ und meldet den Start einer Recherche', () => {
    const run = { id: 'ARC-F0007-20261015-090000-research', kind: 'research' as const, ticketId: 'ARC-F0007', questionId: 'ARC-F0007', title: 'Brutto?', profile: 'standard', model: 'opus', effort: null, startedAt: '2026-10-15T07:00:00.000Z', log: 'ARC-F0007-20261015-090000-research.log' };
    const formatter = new DispatchStatusFormatter();
    const lines = formatter.lines({ ...BASE, runs: [...BASE.runs, run], research: { auto: true, wip: 1, active: 1 } });
    expect(lines).toContain('  Recherche ARC-F0007, Profil standard (opus/Standard), Protokoll ARC-F0007-20261015-090000-research.log');
    expect(lines).toContain('Recherchen: 1/1 laufen, Automatik an');
    const failing = formatter.lines({ ...BASE, research: { auto: true, wip: 1, active: 0, maxPerDay: 10, today: 3, retryAt: '2026-10-15T10:30:00.000Z', error: 'Git Bash nicht gefunden' } });
    expect(failing).toContain('Recherchen: 0/1 laufen, Automatik an, heute 3/10 automatisch');
    expect(failing).toContain('  Automatik-Fehler (Neuversuch ab 2026-10-15T10:30:00.000Z): Git Bash nicht gefunden');
    expect(formatter.researchLines({ ...BASE, started: run })[0]).toBe('Recherche ARC-F0007 gestartet – Profil standard (opus/Standard), Protokoll ARC-F0007-20261015-090000-research.log.');
  });

  it('zeigt den Zustand der Planungs-Automatik samt Fehler', () => {
    const formatter = new DispatchStatusFormatter();
    expect(formatter.lines({ ...BASE, planningAuto: { auto: false, wip: 1, active: 0 } })).toContain('Planungen: 0/1 laufen, Automatik aus');
    const failing = formatter.lines({ ...BASE, planningAuto: { auto: true, wip: 2, active: 1, retryAt: '2026-10-15T10:30:00.000Z', error: 'Festplatte voll' } });
    expect(failing).toContain('Planungen: 1/2 laufen, Automatik an');
    expect(failing).toContain('  Automatik-Fehler (Neuversuch ab 2026-10-15T10:30:00.000Z): Festplatte voll');
    // Ohne das Feld (älterer Server) bleibt die Zeile weg
    expect(formatter.lines(BASE).some((l) => l.startsWith('Planungen:'))).toBe(false);
  });

  it('nennt bei Merges den Zielbranch nach dem Branch; ohne Zielbranch nichts zusätzlich', () => {
    const lines = new DispatchStatusFormatter().lines({
      ...BASE,
      merges: [
        { ticketId: 'ARC-0003', phase: 'pending', branch: 'ticket/ARC-0003-x', target: 'release/1' },
        { ticketId: 'ARC-0004', phase: 'session', branch: 'ticket/ARC-0004-y', target: null },
      ],
    });
    expect(lines).toContain('  ARC-0003  Merge steht an (ticket/ARC-0003-x → release/1)');
    expect(lines).toContain('  ARC-0004  Merge-Session läuft (ticket/ARC-0004-y)');
  });

  it('nennt ein Board außerhalb der Auswahl und leere Listen knapp', () => {
    const lines = new DispatchStatusFormatter().lines({ ...BASE, runs: [], merges: [], tickets: [], lastErrors: [], boardInScope: false });
    expect(lines).toContain('Board 1 gehört nicht zu den Boards des Dispatchers.');
    expect(lines).toContain('Keine Tickets auf Geplant.');
  });
});
