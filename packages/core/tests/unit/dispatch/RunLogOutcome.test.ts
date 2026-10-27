import { describe, expect, it } from 'vitest';
import { RunLogOutcome } from '../../../src/dispatch/state/RunLogOutcome.js';

describe('RunLogOutcome', () => {
  const o = new RunLogOutcome();

  it('liest Exit-Code 0 und Endzeit', () => {
    expect(o.read('Anweisung\nAusgabe\n\n[dispatch] 2026-10-16T09:20:00.000Z Ende: Exit-Code 0\n'))
      .toEqual({ state: 'finished', exitCode: 0, timedOut: false, endedAt: '2026-10-16T09:20:00.000Z' });
  });

  it('liest Exit-Code ungleich 0', () => {
    expect(o.read('[dispatch] 2026-10-16T09:20:00.000Z Ende: Exit-Code 1\n')).toMatchObject({ state: 'finished', exitCode: 1, timedOut: false });
  });

  it('erkennt das Zeitlimit', () => {
    const text = '\n[dispatch] Zeitlimit erreicht – Claude wird beendet\n\n[dispatch] 2026-10-16T10:00:00.000Z Ende: Exit-Code null, Zeitlimit\n';
    expect(o.read(text)).toEqual({ state: 'finished', exitCode: null, timedOut: true, endedAt: '2026-10-16T10:00:00.000Z' });
  });

  it('meldet einen laufenden Lauf ohne Ende', () => {
    expect(o.read('Anweisung\nAusgabe')).toEqual({ state: 'running', exitCode: null, timedOut: false, endedAt: null });
    expect(o.read('[dispatch] Zeitlimit erreicht – Claude wird beendet\n')).toMatchObject({ state: 'running', timedOut: true });
  });

  it('nimmt das letzte Ende, wenn ein Protokoll mehrere hat', () => {
    const text = '[dispatch] 2026-10-16T09:00:00.000Z Ende: Exit-Code 1\n[dispatch] 2026-10-16T09:30:00.000Z Ende: Exit-Code 0\n';
    expect(o.read(text)).toMatchObject({ exitCode: 0, endedAt: '2026-10-16T09:30:00.000Z' });
  });
});
