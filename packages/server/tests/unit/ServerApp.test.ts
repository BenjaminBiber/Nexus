import { describe, it, expect, beforeAll, afterAll } from 'vitest';
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { gzipSync } from 'node:zlib';
import { TestServer } from '../helpers/TestServer.js';

describe('ServerApp', () => {
  let web: string;
  let server: TestServer;

  beforeAll(async () => {
    web = mkdtempSync(join(tmpdir(), 'harness-web-'));
    mkdirSync(join(web, 'assets'));
    writeFileSync(join(web, 'index.html'), '<html><body>Oberfläche</body></html>');
    writeFileSync(join(web, 'assets', 'app.js'), 'console.log(1)');
    writeFileSync(join(web, 'assets', 'big.js'), 'console.log(2)');
    writeFileSync(join(web, 'assets', 'big.js.gz'), gzipSync('console.log(2)'));
    server = await TestServer.start({ webDir: web });
  });
  afterAll(async () => {
    await server.dispose();
    rmSync(web, { recursive: true, force: true });
  });

  it('beantwortet /api/health mit Namen, Version und Prozess', async () => {
    const res = await server.get('/api/health');
    expect(res.status).toBe(200);
    expect(res.json).toMatchObject({ name: 'ai-harness', version: 'test' });
    expect(res.json.pid).toBe(process.pid);
  });

  it('liefert statische Dateien aus', async () => {
    const res = await server.get('/assets/app.js');
    expect(res.status).toBe(200);
    expect(res.text).toBe('console.log(1)');
  });

  it('lässt Dateien unter /assets dauerhaft zwischenspeichern, index.html nie ungeprüft', async () => {
    expect((await server.get('/assets/app.js')).headers['cache-control']).toBe('public, max-age=31536000, immutable');
    expect((await server.get('/')).headers['cache-control']).toBe('no-cache');
    expect((await server.get('/p/abc/board')).headers['cache-control']).toBe('no-cache');
  });

  it('liefert vorkomprimierte Dateien, wenn der Browser sie annimmt', async () => {
    const zipped = await server.get('/assets/big.js', { 'accept-encoding': 'gzip' });
    expect(zipped.headers['content-encoding']).toBe('gzip');
    const plain = await server.get('/assets/big.js');
    expect(plain.headers['content-encoding']).toBeUndefined();
    expect(plain.text).toBe('console.log(2)');
  });

  it('liefert index.html für Seitenadressen (Seitenrouting)', async () => {
    for (const url of ['/', '/p/abc/board', '/p/abc/tickets/T-0001', '/docs', '/p/abc/docs/docs/specs/plan.md?board=15']) {
      const res = await server.get(url);
      expect(res.status, url).toBe(200);
      expect(res.text).toContain('Oberfläche');
    }
  });

  it('antwortet auf unbekannte API-Pfade und fehlende Dateien mit 404 als JSON', async () => {
    const api = await server.get('/api/gibtesnicht');
    expect(api.status).toBe(404);
    expect(api.json.code).toBe('not-found');
    expect((await server.get('/fehlt.js')).status).toBe(404);
  });

  it('lehnt fremde Anfragen ab, bevor eine Route sie sieht', async () => {
    const evil = await server.get('/api/health', { host: 'evil.example' });
    expect(evil.status).toBe(403);
    expect(evil.json.code).toBe('host-denied');
    const noHeader = await server.app.inject({ method: 'POST', url: '/api/projects', headers: { host: '127.0.0.1:4710' }, payload: '{}' });
    expect(noHeader.statusCode).toBe(403);
  });

  it('weist ohne gebaute Oberfläche freundlich darauf hin', async () => {
    const bare = await TestServer.start();
    try {
      const res = await bare.get('/');
      expect(res.status).toBe(200);
      expect(res.text).toContain('npm run build');
    } finally {
      await bare.dispose();
    }
  });
});
