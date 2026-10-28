import { existsSync, mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { HomeFiles } from '../lib/HomeFiles.mjs';
import { Variables } from '../lib/Variables.mjs';

describe('HomeFiles', () => {
  let home;
  let variables;

  beforeEach(() => {
    home = mkdtempSync(join(tmpdir(), 'harness-contract-home-'));
    variables = new Variables({ HOME: home, NOW: '2026-10-28T08:00:00.000Z' });
  });

  afterEach(() => {
    rmSync(home, { recursive: true, force: true });
  });

  // Liest eine geschriebene Datei relativ zum Home
  const read = (path) => readFileSync(join(home, ...path.split('/')), 'utf8');

  it('schreibt einen Text so, wie er dasteht', () => {
    new HomeFiles(home).write({ 'dispatch-logs/a.log': 'erste Zeile\nzweite Zeile\n' }, variables);
    expect(read('dispatch-logs/a.log')).toBe('erste Zeile\nzweite Zeile\n');
  });

  it('schreibt ein Objekt als JSON mit zwei Leerzeichen und Zeilenende', () => {
    new HomeFiles(home).write({ 'dispatch/state.json': { paused: true, pausedUntil: null } }, variables);
    expect(read('dispatch/state.json')).toBe('{\n  "paused": true,\n  "pausedUntil": null\n}\n');
  });

  it('schreibt eine Liste als JSON', () => {
    new HomeFiles(home).write({ 'dispatch/liste.json': [1, 2] }, variables);
    expect(read('dispatch/liste.json')).toBe('[\n  1,\n  2\n]\n');
  });

  it('legt fehlende Ordner an und liefert die Pfade', () => {
    const written = new HomeFiles(home).write({ 'dispatch/runs/stats/r1.json': { id: 'r1' } }, variables);
    expect(written).toEqual([join(home, 'dispatch', 'runs', 'stats', 'r1.json')]);
    expect(existsSync(join(home, 'dispatch', 'runs', 'stats', 'r1.json'))).toBe(true);
  });

  it('setzt Variablen im Inhalt ein – im Text und im JSON-Wert', () => {
    new HomeFiles(home).write(
      { 'dispatch-logs/b.log': 'Home: {{HOME}}\n', 'dispatch/runs/r2.json': { startedAt: '{{NOW}}' } },
      variables,
    );
    expect(read('dispatch-logs/b.log')).toBe(`Home: ${home}\n`);
    expect(JSON.parse(read('dispatch/runs/r2.json'))).toEqual({ startedAt: '2026-10-28T08:00:00.000Z' });
  });

  it('ohne Dateien passiert nichts', () => {
    expect(new HomeFiles(home).write({}, variables)).toEqual([]);
  });
});
