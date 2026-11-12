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

  it('ersetzt das Bot-Token im aufgezeichneten Pfad', async () => {
    const stub = await HttpStub.start();
    try {
      await fetch(`${stub.url}/bot123456:AAHdqTcvCH1vGWJxfSeofSAs0K5PALDsaw/getMe`);
      expect(stub.requests[0].path).toBe('/bot<TOKEN>/getMe');
    } finally {
      await stub.close();
    }
  });

  it('antwortet je Aufruf eigens und wiederholt die letzte Antwort einer Liste', async () => {
    const stub = await HttpStub.start(200, { ok: true, result: true }, {
      getMe: { body: { ok: true, result: { id: 7, username: 'nexus_bot', first_name: 'Nexus' } } },
      getUpdates: { bodies: [{ ok: true, result: [{ update_id: 1 }] }, { ok: true, result: [] }] },
      sendMessage: { status: 403, body: { ok: false, error_code: 403, description: 'Forbidden' } },
    });
    try {
      const call = (name) => fetch(`${stub.url}/bot7:geheim/${name}`).then(async (response) => [response.status, await response.json()]);

      expect(await call('getMe')).toEqual([200, { ok: true, result: { id: 7, username: 'nexus_bot', first_name: 'Nexus' } }]);
      expect(await call('getUpdates')).toEqual([200, { ok: true, result: [{ update_id: 1 }] }]);
      expect(await call('getUpdates')).toEqual([200, { ok: true, result: [] }]);
      expect(await call('getUpdates')).toEqual([200, { ok: true, result: [] }]);
      expect(await call('sendMessage')).toEqual([403, { ok: false, error_code: 403, description: 'Forbidden' }]);
      // Ein Aufruf ohne eigene Antwort bekommt die Standardantwort
      expect(await call('setMyCommands')).toEqual([200, { ok: true, result: true }]);
    } finally {
      await stub.close();
    }
  });

  it('wartet auf die Anfragen an einen Aufruf und meldet sich nach dem Zeitlimit', async () => {
    const stub = await HttpStub.start();
    try {
      setTimeout(() => fetch(`${stub.url}/bot7:geheim/getMe`), 30);
      expect(await stub.waitFor('getMe', 1)).toBe(1);
      await expect(stub.waitFor('sendMessage', 1, 50)).rejects.toThrow('sendMessage');
    } finally {
      await stub.close();
    }
  });

  it('lehnt widersprüchliche Antworten eines Aufrufs ab', async () => {
    await expect(HttpStub.start(200, undefined, { getMe: { body: {}, bodies: [{}] } })).rejects.toThrow('bodies');
    await expect(HttpStub.start(200, undefined, { getMe: { bodies: [] } })).rejects.toThrow('nicht leere Liste');
    await expect(HttpStub.start(200, undefined, { getMe: 'ok' })).rejects.toThrow('muss ein Objekt sein');
  });

  it('nimmt nach dem Beenden keine Anfragen mehr an', async () => {
    const stub = await HttpStub.start();
    const url = stub.url;
    await stub.close();
    await expect(fetch(url)).rejects.toThrow();
  });
});
