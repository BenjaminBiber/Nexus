import { mkdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { TestServer } from '../helpers/TestServer.js';

describe('CustomizationRoutes', () => {
  let server: TestServer;

  beforeEach(async () => { server = await TestServer.start(); });
  afterEach(async () => server.dispose());

  it('meldet ohne Übersicht den Stand unbekannt', async () => {
    const res = await server.get('/api/customizations');
    expect(res.status).toBe(200);
    expect(res.json).toEqual({ state: 'unknown' });
  });

  it('liefert die Übersicht ohne Diffs', async () => {
    const entry = { point: 'skill', name: 'harness-grill', kind: 'replace', state: 'active', base: 'outdated', problems: [], warnings: [{ code: 'base-outdated', message: 'm' }] };
    const overview = { version: 1, createdAt: '2026-10-14T10:00:00.000Z', cloneDir: '/h', customDir: '/h/custom', harnessVersion: '0.2.0', workflow: 'standard', entries: [{ ...entry, diff: '+neu' }] };
    mkdirSync(server.home.customizationsDir, { recursive: true });
    writeFileSync(join(server.home.customizationsDir, 'overview.json'), JSON.stringify(overview));
    expect((await server.get('/api/customizations')).json).toEqual({ ...overview, entries: [entry] });
  });
});
