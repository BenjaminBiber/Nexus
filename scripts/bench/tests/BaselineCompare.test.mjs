import { describe, expect, it } from 'vitest';
import { compareMetrics, formatComparison, machineWarning, metricValue, worseRows } from '../lib/BaselineCompare.mjs';

// Eine Zeile des Vergleichs nach Namen
const row = (rows, name) => rows.find((entry) => entry.name === name);

describe('metricValue', () => {
  it('nimmt p95 vor median', () => {
    expect(metricValue({ median: 10, p95: 20 })).toBe(20);
  });

  it('nimmt median, wenn p95 fehlt', () => {
    expect(metricValue({ median: 10 })).toBe(10);
  });

  it('nimmt value, wenn p95 und median fehlen', () => {
    expect(metricValue({ value: 178.2 })).toBe(178.2);
  });

  it('liefert null, wenn kein Wert vergleichbar ist', () => {
    expect(metricValue({ value: null })).toBeNull();
    expect(metricValue(undefined)).toBeNull();
    expect(metricValue('schnell')).toBeNull();
  });
});

describe('compareMetrics', () => {
  it('erklärt gleiche Werte für in Ordnung', () => {
    const rows = compareMetrics({ boardLoadMs: { p95: 20 } }, { boardLoadMs: { p95: 20 } }, 10);
    expect(row(rows, 'boardLoadMs')).toMatchObject({ alt: 20, neu: 20, deltaPct: 0, schlechter: false });
    expect(worseRows(rows)).toEqual([]);
  });

  it('nennt 11 % mehr bei 10 % Toleranz schlechter', () => {
    const rows = compareMetrics({ boardLoadMs: { p95: 100 } }, { boardLoadMs: { p95: 111 } }, 10);
    expect(row(rows, 'boardLoadMs')).toMatchObject({ deltaPct: 11, schlechter: true });
    expect(worseRows(rows)).toHaveLength(1);
  });

  it('lässt 10 % bei 10 % Toleranz gelten', () => {
    const rows = compareMetrics({ boardLoadMs: { p95: 100 } }, { boardLoadMs: { p95: 110 } }, 10);
    expect(row(rows, 'boardLoadMs').schlechter).toBe(false);
  });

  it('erklärt bessere Werte für in Ordnung', () => {
    const rows = compareMetrics({ hookSessionStartMs: { p95: 400 } }, { hookSessionStartMs: { p95: 120 } }, 10);
    expect(row(rows, 'hookSessionStartMs')).toMatchObject({ deltaPct: -70, schlechter: false });
  });

  it('macht aus einem fehlenden Wert eine Warnung statt eines Fehlers', () => {
    const rows = compareMetrics(
      { boardLoadMs: { p95: 20 }, serverRssIdleMb: { value: 100 } },
      { boardLoadMs: { p95: 20 }, serverRssIdleMb: { value: null }, sseSessionsLatencyMs: { p95: 3 } },
      10,
    );
    expect(row(rows, 'serverRssIdleMb')).toMatchObject({ fehlt: 'neu', deltaPct: null, schlechter: false });
    expect(row(rows, 'sseSessionsLatencyMs')).toMatchObject({ fehlt: 'alt', schlechter: false });
    expect(worseRows(rows)).toEqual([]);
  });

  it('vergleicht alle Messgrößen eines Laufs, die neuen zuerst', () => {
    const rows = compareMetrics({ alt: { p95: 1 } }, { neu1: { p95: 1 }, neu2: { p95: 1 } }, 10);
    expect(rows.map((entry) => entry.name)).toEqual(['neu1', 'neu2', 'alt']);
  });

  it('kommt ohne Messgrößen aus', () => {
    expect(compareMetrics(undefined, undefined, 10)).toEqual([]);
  });

  it('nimmt 10 % als Toleranz, wenn keine genannt ist', () => {
    const rows = compareMetrics({ a: { p95: 100 } }, { a: { p95: 109 } });
    expect(row(rows, 'a').schlechter).toBe(false);
  });

  it('nennt einen Anstieg von 0 aus schlechter, ohne durch 0 zu teilen', () => {
    const rows = compareMetrics({ a: { p95: 0 } }, { a: { p95: 5 } }, 10);
    expect(row(rows, 'a')).toMatchObject({ deltaPct: Infinity, schlechter: true });
  });
});

describe('formatComparison', () => {
  it('schreibt je Messgröße eine Zeile mit alt, neu und Abweichung', () => {
    const rows = compareMetrics({ boardLoadMs: { p95: 100 } }, { boardLoadMs: { p95: 111 } }, 10);
    const text = formatComparison(rows, 10);
    expect(text.split('\n')).toHaveLength(3);
    expect(text).toContain('Messgröße');
    expect(text).toMatch(/boardLoadMs\s+100\s+111\s+\+11 %\s+schlechter \(über 10 %\)/);
  });

  it('zeigt fehlende Werte als Strich mit Hinweis', () => {
    const rows = compareMetrics({}, { neu: { p95: 1 } }, 10);
    expect(formatComparison(rows, 10)).toMatch(/neu\s+–\s+1\s+–\s+nur im neuen Lauf/);
  });
});

describe('machineWarning', () => {
  it('warnt bei verschiedenen Rechnern', () => {
    const warning = machineWarning({ os: 'darwin', cpu: 'Apple M5' }, { os: 'linux', cpu: 'Xeon' });
    expect(warning).toContain('verschiedenen Rechnern');
  });

  it('schweigt bei gleichem Rechner', () => {
    const machine = { os: 'darwin', arch: 'arm64', cpu: 'Apple M5', node: 'v26.10.0' };
    expect(machineWarning(machine, { ...machine })).toBeNull();
  });

  it('schweigt, wenn die alte Messung keinen Rechner nennt', () => {
    expect(machineWarning(undefined, { os: 'darwin' })).toBeNull();
  });
});
