import { describe, expect, it } from 'vitest';
import { LiveServer } from '../helpers/LiveServer.js';
import { MemoryBoardBackend } from '../helpers/MemoryBoardBackend.js';

describe('Live-Meldungen der Fragen auf der Board-Datenbank', () => {
  it('meldet eine Änderung an Fragen des beobachteten Projekts als dbquestions', async () => {
    const backend = await MemoryBoardBackend.create();
    const live = await LiveServer.start(15000, { boardBackend: async () => backend, dbPollMs: 50 });
    try {
      const { id } = await live.server.addProject();
      const project = await backend.services.projects.create('ARC', 'Arches', backend.actor());
      const stream = await live.listen(id, `?dbProject=${project.id}`);
      expect(await LiveServer.until(() => stream.events.some((e) => e.type === 'hello'))).toBe(true);
      await new Promise((r) => setTimeout(r, 150));
      await backend.services.questions.create('ARC', { title: 'Neu' }, { ...backend.actor(), kind: 'human', session: null });
      expect(await LiveServer.until(() => stream.events.some((e) => e.type === 'dbquestions' && e.projectId === project.id))).toBe(true);
      await stream.stop();
    } finally {
      await live.server.dispose();
    }
  });
});
