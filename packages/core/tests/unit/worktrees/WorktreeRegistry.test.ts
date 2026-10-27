import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { mkdirSync, mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { HarnessLayout } from '../../../src/project/HarnessLayout.js';
import { WorktreeRegistry } from '../../../src/worktrees/WorktreeRegistry.js';

describe('WorktreeRegistry', () => {
  let dir: string;
  let registry: WorktreeRegistry;

  beforeEach(() => {
    dir = mkdtempSync(join(tmpdir(), 'harness-reg-'));
    mkdirSync(join(dir, 'harness')); // legt sonst `harness init` an
    registry = new WorktreeRegistry(new HarnessLayout(dir));
  });
  afterEach(() => rmSync(dir, { recursive: true, force: true }));

  it('ist anfangs leer', async () => {
    expect(await registry.read()).toEqual({});
  });

  it('fügt Einträge und Tickets hinzu', async () => {
    await registry.add('ticket/T-0001-a', '/pfad/wt', 'T-0001');
    await registry.addTicket('ticket/T-0001-a', 'T-0002');
    expect(await registry.read()).toEqual({
      'ticket/T-0001-a': { path: '/pfad/wt', tickets: ['T-0001', 'T-0002'], readyToMerge: false },
    });
  });

  it('setzt "bereit zum Merge"', async () => {
    await registry.add('b', '/p', 'T-0001');
    await registry.setReady('b', true);
    expect((await registry.read()).b.readyToMerge).toBe(true);
  });

  it('verliert bei gleichzeitigen Schreibzugriffen nichts', async () => {
    await registry.add('b', '/p', 'T-0000');
    await Promise.all(Array.from({ length: 15 }, (_, i) => registry.addTicket('b', `T-${i + 1}`)));
    expect((await registry.read()).b.tickets).toHaveLength(16);
  });
});
