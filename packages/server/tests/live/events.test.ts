import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { LiveServer } from '../helpers/LiveServer.js';

describe('Live-Meldungen (Server-Sent Events)', () => {
  let live: LiveServer;

  beforeEach(async () => (live = await LiveServer.start(200)));
  afterEach(async () => {
    await live.server.dispose();
  });

  it('meldet beim Verbinden "hello"', async () => {
    const { id } = await live.server.addProject();
    const stream = await live.listen(id);
    expect(await LiveServer.until(() => stream.events.some((e) => e.type === 'hello'))).toBe(true);
    await stream.stop();
  });

  it('meldet Änderungen an Sessions als "sessions", nur im betroffenen Projekt', async () => {
    const a = await live.server.addProject();
    const b = await live.server.addProject();
    const streamA = await live.listen(a.id);
    const streamB = await live.listen(b.id);
    await LiveServer.until(() => streamA.events.length > 0 && streamB.events.length > 0);
    live.server.app.sessions.apply(a.id, { kind: 'session-start', sessionId: 's1', cwd: 'D:/x' });
    expect(await LiveServer.until(() => streamA.events.some((e: any) => e.type === 'sessions'), 3000)).toBe(true);
    expect(streamB.events.some((e: any) => e.type === 'sessions')).toBe(false);
    await streamA.stop();
    await streamB.stop();
  });

  it('sendet regelmäßig einen Herzschlag', async () => {
    const { id } = await live.server.addProject();
    const stream = await live.listen(id);
    expect(await LiveServer.until(() => stream.raw.includes(': ping'), 2000)).toBe(true);
    await stream.stop();
  });

  it('räumt auf, wenn der Browser die Verbindung schließt', async () => {
    const { id } = await live.server.addProject();
    const stream = await live.listen(id);
    await LiveServer.until(() => stream.events.length > 0);
    expect(live.server.app.events.subscribers(id)).toBe(1);
    await stream.stop();
    expect(await LiveServer.until(() => live.server.app.events.subscribers(id) === 0)).toBe(true);
  });

  it('weist ein unbekanntes Projekt mit 404 ab', async () => {
    const res = await fetch(`${live.url}/api/projects/00000000/events`);
    expect(res.status).toBe(404);
  });

  it('beendet offene Verbindungen beim Schließen des Servers', async () => {
    const { id } = await live.server.addProject();
    const stream = await live.listen(id);
    await LiveServer.until(() => stream.events.length > 0);
    await expect(live.server.app.close()).resolves.toBeUndefined();
  });

  it('schickt Meldungen nur an Tabs mit ?notify=1 und meldet sie beim Schließen ab', async () => {
    const { id } = await live.server.addProject();
    const plain = await live.listen(id);
    const tab = await live.listen(id, '?notify=1');
    await LiveServer.until(() => plain.events.length > 0 && tab.events.length > 0);
    const notice = { type: 'notice' as const, kind: 'wartet' as const, tag: 'attention', title: 'T', body: 'B', url: '/p/x/sessions' };
    expect(live.server.app.noticeTabs.deliver(notice)).toBe(true);
    expect(await LiveServer.until(() => tab.events.some((e: any) => e.type === 'notice'))).toBe(true);
    expect(plain.events.some((e: any) => e.type === 'notice')).toBe(false);
    await tab.stop();
    expect(await LiveServer.until(() => !live.server.app.noticeTabs.deliver(notice))).toBe(true);
    await plain.stop();
  });
});
