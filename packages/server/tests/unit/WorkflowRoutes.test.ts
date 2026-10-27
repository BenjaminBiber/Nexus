import { mkdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { TestServer } from '../helpers/TestServer.js';
describe('WorkflowRoutes', () => {
  let server: TestServer;
  beforeEach(async () => { server = await TestServer.start(); });
  afterEach(async () => server.dispose());
  it('liefert ohne Anpassung den Standard', async () => {
    const res = await server.get('/api/workflow');
    expect(res.status).toBe(200);
    expect(res.json.source).toBe('standard');
    expect(res.json.phases.map((p: { id: string }) => p.id)).toEqual(['planen', 'umsetzen']);
  });
  it('liefert den angewendeten Workflow', async () => {
    const phases = [{ id: 'planen', title: 'P', hint: 'h', steps: [] }, { id: 'umsetzen', title: 'U', hint: 'h', steps: [] }];
    mkdirSync(server.home.customizationsDir, { recursive: true });
    writeFileSync(join(server.home.customizationsDir, 'workflow.json'), JSON.stringify(phases));
    expect((await server.get('/api/workflow')).json).toEqual({ source: 'custom', phases });
  });
});
