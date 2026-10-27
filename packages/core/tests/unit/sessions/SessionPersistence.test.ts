import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { HarnessHome } from '../../../src/registry/HarnessHome.js';
import { SessionPersistence } from '../../../src/sessions/SessionPersistence.js';
import type { Session } from '../../../src/sessions/Session.js';

const sample: Session = {
  id: 's1', projectId: 'p', tickets: ['T-0001'], cwd: 'D:/wt', state: 'idle',
  startedAt: '2026-10-10T10:00:00.000Z', lastEventAt: '2026-10-10T10:00:00.000Z', subagents: [],
};

describe('SessionPersistence', () => {
  let dir: string;
  let persistence: SessionPersistence;

  beforeEach(() => {
    dir = mkdtempSync(join(tmpdir(), 'harness-sess-'));
    persistence = new SessionPersistence(new HarnessHome({ HARNESS_HOME: join(dir, 'neu') }));
  });
  afterEach(() => rmSync(dir, { recursive: true, force: true }));

  it('liefert ohne Datei eine leere Liste', async () => {
    expect(await persistence.read()).toEqual([]);
  });

  it('schreibt und liest Sessions, auch bei noch nicht vorhandenem Ordner', async () => {
    await persistence.write([sample]);
    expect(await persistence.read()).toEqual([sample]);
    expect(JSON.parse(readFileSync(join(dir, 'neu', 'sessions.json'), 'utf8')).sessions).toHaveLength(1);
  });

  it('meldet eine beschädigte Datei verständlich', async () => {
    await persistence.write([sample]);
    writeFileSync(join(dir, 'neu', 'sessions.json'), '{kaputt');
    await expect(persistence.read()).rejects.toThrow(/sessions\.json/);
  });

  it('übersteht viele gleichzeitige Schreibzugriffe mit gültigem Ergebnis', async () => {
    await Promise.all(Array.from({ length: 20 }, (_, i) => persistence.write([{ ...sample, id: `s${i}` }])));
    expect(await persistence.read()).toHaveLength(1);
  });
});
