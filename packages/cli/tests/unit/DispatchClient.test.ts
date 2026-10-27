import { createServer, type IncomingMessage, type Server } from 'node:http';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { DispatchClient } from '../../src/board/DispatchClient.js';
import { ServerLauncher } from '../../src/board/ServerLauncher.js';

describe('DispatchClient', () => {
  let server: Server;
  let port: number;
  const calls: Array<{ method: string; url: string; body: string; harness: string | undefined }> = [];

  // Liest den Body einer Anfrage
  const body = (req: IncomingMessage) => new Promise<string>((resolve) => {
    let text = '';
    req.on('data', (c) => { text += c; });
    req.on('end', () => resolve(text));
  });

  beforeEach(async () => {
    calls.length = 0;
    server = createServer(async (req, res) => {
      const text = await body(req);
      if (req.url === '/api/health') return res.end(JSON.stringify({ name: 'ai-harness' }));
      calls.push({ method: req.method!, url: req.url!, body: text, harness: req.headers['x-harness'] as string | undefined });
      if (req.url === '/api/dispatch/merge/ARC-0001') {
        res.statusCode = 422;
        return res.end(JSON.stringify({ code: 'merge-not-possible', message: 'ARC-0001 wartet auf keinen Merge.' }));
      }
      if (req.url === '/api/dispatch/merge/ARC-0002') {
        res.statusCode = 502;
        return res.end('<html>Bad Gateway</html>');
      }
      if (req.url === '/api/dispatch/merge/ARC-0003') return res.end('<html>ok</html>');
      res.end(JSON.stringify({ state: 'idle', message: 'bereit · 0/1' }));
    });
    await new Promise<void>((resolve) => server.listen(0, '127.0.0.1', resolve));
    port = (server.address() as { port: number }).port;
  });
  afterEach(() => new Promise<void>((resolve) => server.close(() => resolve())));

  it('spricht die Routen an, schreibende mit Pflichtkopf', async () => {
    const client = new DispatchClient(port, new ServerLauncher());
    expect((await client.status(3)).message).toBe('bereit · 0/1');
    await client.status();
    await client.run(3);
    await client.pause(15);
    await client.resume();
    await client.setEnabled(true);
    await client.setEnabled(false);
    expect(calls.map((c) => `${c.method} ${c.url} ${c.body}`)).toEqual([
      'GET /api/dispatch/status?boardId=3 ',
      'GET /api/dispatch/status ',
      'POST /api/dispatch/run {"boardId":3}',
      'POST /api/dispatch/pause {"minutes":15}',
      'POST /api/dispatch/resume {}',
      'POST /api/dispatch/enable {}',
      'POST /api/dispatch/disable {}',
    ]);
    expect(calls.filter((c) => c.method === 'POST').every((c) => c.harness === '1')).toBe(true);
  });

  it('startet eine Recherche mit kodierter Fragen-ID und Pflichtkopf', async () => {
    await new DispatchClient(port, new ServerLauncher()).startResearch('ARC-F0007');
    expect(calls).toEqual([{ method: 'POST', url: '/api/dispatch/research/ARC-F0007', body: '{}', harness: '1' }]);
  });

  it('macht Fehlerantworten zu HarnessError', async () => {
    await expect(new DispatchClient(port, new ServerLauncher()).merge('ARC-0001')).rejects.toMatchObject({ code: 'merge-not-possible', message: 'ARC-0001 wartet auf keinen Merge.' });
  });

  it('kodiert die Ticket-ID und sendet den Pflichtkopf beim Merge', async () => {
    expect((await new DispatchClient(port, new ServerLauncher()).merge('ARC 1')).message).toBe('bereit · 0/1');
    expect(calls).toEqual([{ method: 'POST', url: '/api/dispatch/merge/ARC%201', body: '{}', harness: '1' }]);
  });

  it('nimmt ein Ticket ab: kodierte ID, Name im Body, Pflichtkopf', async () => {
    await new DispatchClient(port, new ServerLauncher()).accept('ARC 4', 'Florian');
    expect(calls).toEqual([{ method: 'POST', url: '/api/dispatch/accept/ARC%204', body: '{"by":"Florian"}', harness: '1' }]);
  });

  it('meldet 502 mit HTML-Body als dispatch-failed', async () => {
    await expect(new DispatchClient(port, new ServerLauncher()).merge('ARC-0002')).rejects.toMatchObject({ code: 'dispatch-failed', message: 'Der Server antwortet mit 502.' });
  });

  it('meldet 2xx ohne JSON als dispatch-failed', async () => {
    await expect(new DispatchClient(port, new ServerLauncher()).merge('ARC-0003')).rejects.toMatchObject({ code: 'dispatch-failed', message: 'Der Server lieferte keine lesbare Antwort.' });
  });
});
