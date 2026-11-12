import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { download, prefetch, sha1 } from '../prefetch-nsis.mjs';

// Antwort wie von fetch: nur das, was download() liest
const response = (status, body = '') => ({
  ok: status >= 200 && status < 300,
  status,
  arrayBuffer: async () => new TextEncoder().encode(body).buffer,
  json: async () => JSON.parse(body),
});

// fetch, das die Antworten der Reihe nach liefert und die Aufrufe zählt
function scripted(...answers) {
  const calls = [];
  const fetchImpl = async (url, init) => {
    calls.push(init ? { url, headers: init.headers } : url);
    const next = answers.shift();
    if (next instanceof Error) throw next;
    return next;
  };
  return { fetchImpl, calls };
}

const quiet = { wait: async () => {}, log: () => {} };

describe('prefetch-nsis: download', () => {
  it('versucht es nach einem 504 erneut und liefert die Datei', async () => {
    const { fetchImpl, calls } = scripted(response(504), response(200, 'inhalt'));
    const data = await download('https://example.test/a.zip', sha1('inhalt'), { fetchImpl, ...quiet });
    expect(data.toString()).toBe('inhalt');
    expect(calls).toHaveLength(2);
  });

  it('versucht es nach einem Netzwerkfehler erneut', async () => {
    const { fetchImpl, calls } = scripted(new Error('ECONNRESET'), response(200, 'inhalt'));
    await download('https://example.test/a.zip', sha1('inhalt'), { fetchImpl, ...quiet });
    expect(calls).toHaveLength(2);
  });

  it('gibt nach der letzten Wiederholung mit dem Grund auf', async () => {
    const { fetchImpl, calls } = scripted(response(504), response(504), response(504));
    await expect(download('https://example.test/a.zip', sha1('x'), { fetchImpl, attempts: 3, ...quiet })).rejects.toThrow(
      /HTTP 504.*3 Versuche/,
    );
    expect(calls).toHaveLength(3);
  });

  it('nimmt eine Datei mit falscher Prüfsumme nicht an', async () => {
    const { fetchImpl } = scripted(response(200, 'kaputt'));
    await expect(download('https://example.test/a.zip', sha1('inhalt'), { fetchImpl, attempts: 1, ...quiet })).rejects.toThrow(
      /SHA1/,
    );
  });

  it('lädt mit Token angemeldet über die Release-API statt anonym über github.com', async () => {
    const release = { assets: [{ name: 'nsis-3.13.zip', url: 'https://api.github.com/repos/o/r/releases/assets/7' }] };
    const { fetchImpl, calls } = scripted(response(200, JSON.stringify(release)), response(200, 'inhalt'));
    const url = 'https://github.com/o/r/releases/download/nsis-3.13/nsis-3.13.zip';
    const data = await download(url, sha1('inhalt'), { token: 't0k', fetchImpl, ...quiet });
    expect(data.toString()).toBe('inhalt');
    expect(calls[0].url).toBe('https://api.github.com/repos/o/r/releases/tags/nsis-3.13');
    expect(calls[0].headers.authorization).toBe('Bearer t0k');
    expect(calls[1].url).toBe('https://api.github.com/repos/o/r/releases/assets/7');
    expect(calls[1].headers).toMatchObject({ authorization: 'Bearer t0k', accept: 'application/octet-stream' });
  });

  it('meldet eine fehlende Datei im Release', async () => {
    const { fetchImpl } = scripted(response(200, JSON.stringify({ assets: [] })));
    const url = 'https://github.com/o/r/releases/download/v1/a.zip';
    await expect(download(url, sha1('x'), { token: 't0k', fetchImpl, attempts: 1, ...quiet })).rejects.toThrow(/keine Datei a\.zip/);
  });

  it('vergleicht die Prüfsumme ohne Groß- und Kleinschreibung', async () => {
    const { fetchImpl } = scripted(response(200, 'inhalt'));
    await expect(download('https://example.test/a.zip', sha1('inhalt').toLowerCase(), { fetchImpl, ...quiet })).resolves.toBeDefined();
  });
});

describe('prefetch-nsis: prefetch', () => {
  let toolsDir;
  beforeEach(() => {
    toolsDir = mkdtempSync(join(tmpdir(), 'nexus-nsis-'));
  });
  afterEach(() => {
    rmSync(toolsDir, { recursive: true, force: true });
  });

  // Kleine Stellvertreter für NSIS und das Plugin, damit kein Test ins Netz geht
  const nsis = { url: 'https://example.test/nsis.zip', sha1: sha1('zip'), folder: 'nsis-x', exeSha1: sha1('exe') };
  const utils = { url: 'https://example.test/utils.dll', sha1: sha1('dll'), path: 'Plugins/x86-unicode/additional/nsis_tauri_utils.dll' };
  const files = { [nsis.url]: 'zip', [utils.url]: 'dll' };
  const fakeDownload = async (url) => Buffer.from(files[url]);
  // Entpackt „wie das Archiv“: legt den versionierten Ordner mit NSIS.exe an
  const fakeExtract = (zipPath, target) => {
    expect(readFileSync(zipPath, 'utf8')).toBe('zip');
    mkdirSync(join(target, nsis.folder), { recursive: true });
    writeFileSync(join(target, nsis.folder, 'NSIS.exe'), 'exe');
  };

  it('legt NSIS und das Plugin dort ab, wo Tauri sie sucht', async () => {
    const result = await prefetch({ toolsDir, nsis, utils, download: fakeDownload, extract: fakeExtract, log: () => {} });
    expect(result).toBe('geladen');
    expect(readFileSync(join(toolsDir, 'NSIS', 'NSIS.exe'), 'utf8')).toBe('exe');
    expect(readFileSync(join(toolsDir, 'NSIS', utils.path), 'utf8')).toBe('dll');
    // Weder das Archiv noch der versionierte Ordner bleiben liegen
    expect(existsSync(join(toolsDir, nsis.folder))).toBe(false);
    expect(existsSync(join(toolsDir, 'nsis.zip'))).toBe(false);
  });

  it('lässt einen passenden Stand liegen und lädt nichts', async () => {
    mkdirSync(join(toolsDir, 'NSIS', 'Plugins', 'x86-unicode', 'additional'), { recursive: true });
    writeFileSync(join(toolsDir, 'NSIS', 'NSIS.exe'), 'exe');
    writeFileSync(join(toolsDir, 'NSIS', utils.path), 'dll');
    const download = async () => {
      throw new Error('darf nicht laden');
    };
    const result = await prefetch({ toolsDir, nsis, utils, download, extract: fakeExtract, log: () => {} });
    expect(result).toBe('vorhanden');
  });

  it('ersetzt einen veralteten Stand', async () => {
    mkdirSync(join(toolsDir, 'NSIS'), { recursive: true });
    writeFileSync(join(toolsDir, 'NSIS', 'NSIS.exe'), 'alt');
    writeFileSync(join(toolsDir, 'NSIS', 'übrig.txt'), 'alt');
    const result = await prefetch({ toolsDir, nsis, utils, download: fakeDownload, extract: fakeExtract, log: () => {} });
    expect(result).toBe('geladen');
    expect(readFileSync(join(toolsDir, 'NSIS', 'NSIS.exe'), 'utf8')).toBe('exe');
    expect(existsSync(join(toolsDir, 'NSIS', 'übrig.txt'))).toBe(false);
  });
});
