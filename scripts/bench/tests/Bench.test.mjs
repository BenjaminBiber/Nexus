import { describe, expect, it } from 'vitest';
import { machine, merge, parseArgs } from '../bench.mjs';

describe('parseArgs', () => {
  it('nimmt ohne Argumente die volle Messung', () => {
    expect(parseArgs([])).toMatchObject({ tickets: 500, runs: 10, loads: 100, hookRuns: 30, sseRuns: 20, dbRuns: 10, out: null, quick: false });
  });

  it('setzt mit --quick die kleine Messung', () => {
    expect(parseArgs(['--quick'])).toMatchObject({ tickets: 50, runs: 3, loads: 10, hookRuns: 5, sseRuns: 5, dbRuns: 3, quick: true });
  });

  it('lässt ausdrücklich genannte Zahlen auch neben --quick gelten', () => {
    expect(parseArgs(['--quick', '--tickets', '7'])).toMatchObject({ tickets: 7, runs: 3, loads: 10 });
  });

  it('liest Ausgabedatei und Startbefehl', () => {
    const options = parseArgs(['--out', 'contract/baseline.json', '--server-cmd', 'cargo run -- --port {port}']);
    expect(options.out).toBe('contract/baseline.json');
    expect(options.serverCmd).toBe('cargo run -- --port {port}');
  });

  it('nimmt den TypeScript-Server als Standardbefehl', () => {
    expect(parseArgs([]).serverCmd).toContain('{port}');
  });

  it('wirft bei fehlendem Wert', () => {
    expect(() => parseArgs(['--out'])).toThrow(/braucht einen Wert/);
  });

  it('wirft bei einer Zahl, die keine ganze Zahl ab 1 ist', () => {
    expect(() => parseArgs(['--runs', '0'])).toThrow(/ganze Zahl ab 1/);
    expect(() => parseArgs(['--loads', 'viele'])).toThrow(/ganze Zahl ab 1/);
  });

  it('wirft bei unbekannten Argumenten', () => {
    expect(() => parseArgs(['--schnell'])).toThrow(/Unbekanntes Argument/);
  });

  it('liest Vergleichsdatei und Toleranz', () => {
    const options = parseArgs(['--compare', 'contract/baseline.json', '--tolerance', '5']);
    expect(options).toMatchObject({ compare: 'contract/baseline.json', tolerance: 5 });
  });

  it('nimmt 10 % als Toleranz, wenn keine genannt ist', () => {
    expect(parseArgs(['--compare', 'contract/baseline.json']).tolerance).toBe(10);
  });

  it('wirft bei einer Toleranz, die keine Zahl ab 0 ist', () => {
    expect(() => parseArgs(['--tolerance', '-1'])).toThrow(/Zahl ab 0/);
    expect(() => parseArgs(['--tolerance', 'wenig'])).toThrow(/Zahl ab 0/);
  });

  it('liest Hook-Befehl, Zahl der Hook-Läufe und --hyperfine', () => {
    const options = parseArgs(['--hook-cmd', 'nexus hook', '--hook-runs', '7', '--hyperfine']);
    expect(options).toMatchObject({ hookCmd: 'nexus hook', hookRuns: 7, hyperfine: true });
  });

  it('misst ohne --hyperfine selbst und ohne --hook-cmd das Plugin-Bündel', () => {
    expect(parseArgs([])).toMatchObject({ hyperfine: false, hookCmd: null, compare: null });
  });
});

describe('merge', () => {
  const frisch = {
    schema: 1,
    createdAt: '2026-10-27T10:00:00.000Z',
    machine: { os: 'darwin' },
    server: { cmd: 'node …', tickets: 500 },
    metrics: {
      serverColdStartMs: { median: 1 }, boardFirstLoadMs: { median: 2 }, boardLoadMs: { p50: 3 }, serverRssIdleMb: { value: 4 },
      hookSessionStartMs: { median: 5 }, hookUserPromptMs: { median: 6 }, sseSessionsLatencyMs: { median: 7 }, sseDbBoardLatencyMs: { median: 8 },
    },
  };

  it('behält fremde Messgrößen und ersetzt die eigenen', () => {
    const vorher = { schema: 1, metrics: { uiRssMb: { value: 9 }, serverColdStartMs: { median: 999 } } };
    const nachher = merge(vorher, frisch);
    expect(nachher.metrics.uiRssMb).toEqual({ value: 9 });
    expect(nachher.metrics.serverColdStartMs).toEqual({ median: 1 });
  });

  it('ersetzt auch Hook-Laufzeit und SSE-Latenz eines früheren Laufs', () => {
    const vorher = { metrics: { hookSessionStartMs: { median: 999 }, sseDbBoardLatencyMs: { median: 999 } } };
    const nachher = merge(vorher, frisch);
    expect(nachher.metrics.hookSessionStartMs).toEqual({ median: 5 });
    expect(nachher.metrics.sseDbBoardLatencyMs).toEqual({ median: 8 });
  });

  it('behält fremde Felder außerhalb der Messgrößen', () => {
    expect(merge({ notes: 'von Hand ergänzt' }, frisch).notes).toBe('von Hand ergänzt');
  });

  it('kommt ohne vorhandene Datei aus', () => {
    expect(merge(null, frisch)).toEqual(frisch);
  });

  it('übergeht eine Datei, die kein Objekt enthält', () => {
    expect(merge([1, 2, 3], frisch)).toEqual(frisch);
  });
});

describe('machine', () => {
  it('nennt Betriebssystem, Kerne, Speicher und Node-Version', () => {
    const angaben = machine();
    expect(angaben.os).toBe(process.platform);
    expect(angaben.node).toBe(process.version);
    expect(angaben.cpus).toBeGreaterThan(0);
    expect(angaben.memGb).toBeGreaterThan(0);
    expect(typeof angaben.cpu).toBe('string');
  });
});
