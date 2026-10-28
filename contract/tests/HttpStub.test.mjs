import { describe, expect, it } from 'vitest';
import { HttpStub } from '../lib/HttpStub.mjs';

// Schickt eine Anfrage an die Attrappe und liefert den Status
const post = (stub, body, headers = { 'content-type': 'application/json' }) =>
  fetch(stub.url, { method: 'POST', headers, body }).then((response) => response.status);

describe('HttpStub', () => {
  it('zeichnet Methode, Pfad, die festen Köpfe und den JSON-Körper auf', async () => {
    const stub = await HttpStub.start();
    try {
      expect(await post(stub, '{"topic":"harness"}', { 'content-type': 'application/json', authorization: 'Bearer tk_1' })).toBe(200);
      expect(stub.requests).toEqual([
        {
          method: 'POST',
          path: '/',
          headers: { 'content-type': 'application/json', authorization: 'Bearer tk_1' },
          json: { topic: 'harness' },
        },
      ]);
    } finally {
      await stub.close();
    }
  });

  it('antwortet mit dem angegebenen Status und merkt einen Körper ohne JSON als Text', async () => {
    const stub = await HttpStub.start(500);
    try {
      expect(await post(stub, 'kein json')).toBe(500);
      expect(stub.requests).toEqual([{ method: 'POST', path: '/', headers: { 'content-type': 'application/json' }, text: 'kein json' }]);
    } finally {
      await stub.close();
    }
  });

  it('nimmt nach dem Beenden keine Anfragen mehr an', async () => {
    const stub = await HttpStub.start();
    const url = stub.url;
    await stub.close();
    await expect(fetch(url)).rejects.toThrow();
  });
});
