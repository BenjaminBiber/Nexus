import { execFileSync } from 'node:child_process';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterAll, beforeAll, describe, expect, it } from 'vitest';
import { compare, main, medianOfRuns, parseArgs } from '../compare.mjs';

// Messung mit einer Messgröße, deren p95 gegeben ist
const messung = (p95, machine = { os: 'linux', arch: 'x64' }) => ({ machine, metrics: { boardLoadMs: { p50: p95 / 2, p95 } } });

describe('parseArgs', () => {
  it('liest Ausgangswerte, Messwerte und Toleranz', () => {
    const options = parseArgs(['--baseline', 'alt.json', '--measured', 'a.json', '--measured', 'b.json', '--tolerance', '5']);
    expect(options).toEqual({ baseline: 'alt.json', measured: ['a.json', 'b.json'], ignore: [], tolerance: 5, help: false });
  });

  it('nimmt 10 Prozent als Toleranz, wenn keine genannt ist', () => {
    expect(parseArgs(['--baseline', 'alt.json', '--measured', 'a.json']).tolerance).toBe(10);
  });

  it('weist unbekannte Argumente ab', () => {
    expect(() => parseArgs(['--was-auch-immer'])).toThrow(/Unbekanntes Argument/);
  });

  it('verlangt zu jedem Argument einen Wert', () => {
    expect(() => parseArgs(['--baseline'])).toThrow(/braucht einen Wert/);
  });
});

describe('medianOfRuns', () => {
  it('nimmt je Messgröße und Feld den Median über die Wiederholungen', () => {
    const merged = medianOfRuns([messung(10), messung(30), messung(20)]);
    expect(merged.metrics.boardLoadMs).toEqual({ p50: 10, p95: 20 });
  });

  it('übergeht Felder, die keine Zahl sind', () => {
    const merged = medianOfRuns([{ metrics: { a: { value: 5, runs: 'viele' } } }]);
    expect(merged.metrics.a).toEqual({ value: 5 });
  });
});

describe('compare', () => {
  it('endet mit 0, wenn ein Wert 9 Prozent über dem Ausgangswert liegt', () => {
    expect(compare(messung(100), messung(109), 10, () => {})).toBe(0);
  });

  it('endet mit 1, wenn ein Wert 11 Prozent über dem Ausgangswert liegt', () => {
    expect(compare(messung(100), messung(111), 10, () => {})).toBe(1);
  });

  it('nennt den Rückschritt mit altem Wert, neuem Wert und Prozent', () => {
    const zeilen = [];
    compare(messung(100), messung(150), 10, (line) => zeilen.push(line));
    expect(zeilen.join('\n')).toContain('Rückschritt: boardLoadMs alt 100, neu 150 (+50 %)');
  });

  it('lässt Messgrößen ohne Ausgangswert nur eine Meldung sein', () => {
    const zeilen = [];
    const code = compare({ metrics: { boardLoadMs: { p95: null } } }, messung(100), 10, (line) => zeilen.push(line));
    expect(code).toBe(0);
    expect(zeilen.join('\n')).toContain('keinen Ausgangswert');
  });

  it('lässt eine übergangene Messgröße melden, aber nicht scheitern', () => {
    const zeilen = [];
    const code = compare(messung(100), messung(300), 10, (line) => zeilen.push(line), ['boardLoadMs']);
    expect(code).toBe(0);
    expect(zeilen.join('\n')).toContain('Übergangen: „boardLoadMs“ weicht um +200 % ab');
  });

  it('warnt, wenn die Messungen von verschiedenen Rechnern stammen', () => {
    const zeilen = [];
    compare(messung(100, { os: 'darwin', arch: 'arm64' }), messung(100), 10, (line) => zeilen.push(line));
    expect(zeilen.join('\n')).toContain('verschiedenen Rechnern');
  });
});

describe('main', () => {
  let ordner;
  const datei = (name, inhalt) => {
    const pfad = join(ordner, name);
    writeFileSync(pfad, JSON.stringify(inhalt));
    return pfad;
  };

  beforeAll(() => {
    ordner = mkdtempSync(join(tmpdir(), 'bench-compare-'));
  });
  afterAll(() => {
    rmSync(ordner, { recursive: true, force: true });
  });

  it('vergleicht den Median mehrerer Läufe mit den Ausgangswerten', () => {
    const alt = datei('alt.json', messung(100));
    const code = main(['--baseline', alt, '--measured', datei('a.json', messung(104)),
      '--measured', datei('b.json', messung(108)), '--measured', datei('c.json', messung(300))], () => {});
    expect(code).toBe(0);
  });

  it('verlangt Ausgangswerte und Messwerte', () => {
    expect(() => main(['--measured', 'a.json'], () => {})).toThrow(/--baseline fehlt/);
    expect(() => main(['--baseline', 'a.json'], () => {})).toThrow(/--measured fehlt/);
  });

  it('meldet eine unlesbare Datei', () => {
    expect(() => main(['--baseline', join(ordner, 'gibtsnicht.json'), '--measured', 'a.json'], () => {}))
      .toThrow(/lässt sich nicht lesen/);
  });

  it('scheitert als Programm mit Code 1, wenn eine Messgröße 11 Prozent schlechter ist', () => {
    const alt = datei('schwelle-alt.json', messung(100));
    const neu = datei('schwelle-neu.json', messung(111));
    const skript = join(import.meta.dirname, '..', 'compare.mjs');
    expect(() => execFileSync(process.execPath, [skript, '--baseline', alt, '--measured', neu], { stdio: 'pipe' }))
      .toThrow(/Command failed/);
    const code = execFileSync(process.execPath, [skript, '--baseline', alt, '--measured', datei('schwelle-ok.json', messung(109))], { stdio: 'pipe' });
    expect(code.toString()).toBe('');
  });
});
