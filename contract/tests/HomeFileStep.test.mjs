import { mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { HomeFileStep } from '../lib/HomeFileStep.mjs';
import { Variables } from '../lib/Variables.mjs';

describe('HomeFileStep', () => {
  let home;
  let context;

  beforeEach(() => {
    home = mkdtempSync(join(tmpdir(), 'harness-contract-home-'));
    context = { home, variables: new Variables({ lauf: 'BEN-0001-20200302-090000', NOW: '2026-10-28T08:00:00.000Z' }) };
  });

  afterEach(() => {
    rmSync(home, { recursive: true, force: true });
  });

  // Führt einen Schritt der Art aus
  const run = (raw) => new HomeFileStep().run({ raw }, context);

  // Liest eine geschriebene Datei relativ zum Home
  const read = (path) => readFileSync(join(home, ...path.split('/')), 'utf8');

  it('nennt seine eigenen Felder', () => {
    expect(new HomeFileStep().keys).toEqual(['path', 'content']);
  });

  it('schreibt einen Text so, wie er dasteht', async () => {
    await run({ path: 'dispatch-logs/a.log', content: 'erste Zeile\n' });
    expect(read('dispatch-logs/a.log')).toBe('erste Zeile\n');
  });

  it('schreibt jeden anderen Wert als JSON und legt fehlende Ordner an', async () => {
    await run({ path: 'dispatch/runs/r1.result.json', content: { code: 0, timedOut: false } });
    expect(read('dispatch/runs/r1.result.json')).toBe('{\n  "code": 0,\n  "timedOut": false\n}\n');
  });

  it('setzt Variablen im Pfad und im Inhalt ein', async () => {
    const result = await run({ path: 'dispatch/runs/{{lauf}}.result.json', content: { endedAt: '{{NOW}}' } });
    expect(result.request).toEqual({
      kind: 'homeFile',
      path: 'dispatch/runs/BEN-0001-20200302-090000.result.json',
      content: { endedAt: '2026-10-28T08:00:00.000Z' },
    });
    expect(JSON.parse(read('dispatch/runs/BEN-0001-20200302-090000.result.json'))).toEqual({ endedAt: '2026-10-28T08:00:00.000Z' });
  });

  it('zeichnet den geschriebenen Pfad als Antwort auf', async () => {
    const result = await run({ path: 'dispatch/state.json', content: { paused: false } });
    expect(result.response).toEqual({ json: { path: join(home, 'dispatch', 'state.json') } });
  });

  it('meldet einen Pfad außerhalb des Homes', async () => {
    await expect(run({ path: '../board.db', content: 'x' })).rejects.toThrow(/path muss ein Pfad im Home sein/);
  });

  it('meldet einen Pfad mit Rückwärtsschritt in der Mitte', async () => {
    await expect(run({ path: 'dispatch/../../x.json', content: 'x' })).rejects.toThrow(/path muss ein Pfad im Home sein/);
  });

  it('meldet einen Pfad mit Backslash', async () => {
    await expect(run({ path: 'dispatch\\state.json', content: 'x' })).rejects.toThrow(/path muss ein Pfad im Home sein/);
  });

  it('meldet einen fehlenden Pfad', async () => {
    await expect(run({ content: 'x' })).rejects.toThrow(/path muss ein Pfad im Home sein/);
  });

  it('meldet fehlenden Inhalt', async () => {
    await expect(run({ path: 'dispatch/state.json' })).rejects.toThrow(/content fehlt/);
  });
});
