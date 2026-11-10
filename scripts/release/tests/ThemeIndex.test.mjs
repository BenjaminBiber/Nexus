import { createHash } from 'node:crypto';
import { readFileSync, readdirSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { deflateRawSync } from 'node:zlib';
import { describe, expect, it } from 'vitest';
import { packageName, packagePreview, readZipBytes, readZipEntry, themeIndex } from '../lib/ThemeIndex.mjs';

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '../../..');
const REFERENCE = resolve(ROOT, 'scripts/release/tests/fixtures/nexus-themes.reference.json');
const BASE = 'https://github.com/BenjaminBiber/Nexus/releases/download/v1.2.3';

// CRC-32 für die lokalen Kopfzeilen des Mini-ZIPs
function crc32(bytes) {
  let crc = ~0;
  for (const byte of bytes) {
    crc ^= byte;
    for (let k = 0; k < 8; k++) crc = (crc >>> 1) ^ (0xedb88320 & -(crc & 1));
  }
  return ~crc >>> 0;
}

// Baut ein kleines ZIP; `deflate` packt die Dateien komprimiert (Methode 8), sonst gespeichert (0)
function zip(files, { deflate = false } = {}) {
  const locals = [];
  const centrals = [];
  let offset = 0;
  for (const [name, text] of Object.entries(files)) {
    const nameBytes = Buffer.from(name);
    const raw = Buffer.from(text);
    const data = deflate ? deflateRawSync(raw) : raw;
    const method = deflate ? 8 : 0;
    const local = Buffer.alloc(30);
    local.writeUInt32LE(0x04034b50, 0);
    local.writeUInt16LE(20, 4);
    local.writeUInt16LE(method, 8);
    local.writeUInt32LE(crc32(raw), 14);
    local.writeUInt32LE(data.length, 18);
    local.writeUInt32LE(raw.length, 22);
    local.writeUInt16LE(nameBytes.length, 26);
    const central = Buffer.alloc(46);
    central.writeUInt32LE(0x02014b50, 0);
    central.writeUInt16LE(20, 4);
    central.writeUInt16LE(20, 6);
    central.writeUInt16LE(method, 10);
    central.writeUInt32LE(crc32(raw), 16);
    central.writeUInt32LE(data.length, 20);
    central.writeUInt32LE(raw.length, 24);
    central.writeUInt16LE(nameBytes.length, 28);
    central.writeUInt32LE(offset, 42);
    locals.push(local, nameBytes, data);
    centrals.push(central, nameBytes);
    offset += 30 + nameBytes.length + data.length;
  }
  const dir = Buffer.concat(centrals);
  const end = Buffer.alloc(22);
  end.writeUInt32LE(0x06054b50, 0);
  end.writeUInt16LE(centrals.length / 2, 8);
  end.writeUInt16LE(centrals.length / 2, 10);
  end.writeUInt32LE(dir.length, 12);
  end.writeUInt32LE(offset, 16);
  return Buffer.concat([...locals, dir, end]);
}

function paket(manifest, options) {
  return zip({ 'theme.css': ':root{}', 'manifest.json': JSON.stringify(manifest) }, options);
}

const OZEAN = { id: 'ozean', name: 'Ozean', version: '1.0.0', sdkVersion: '^1.1', colorScheme: 'dark' };
const WALD = { id: 'wald', name: 'Wald', version: '2.1.0', sdkVersion: '^1.0', preview: 'https://example.org/wald.png' };

describe('ThemeIndex', () => {
  it('liest manifest.json aus gespeicherten und komprimierten Paketen', () => {
    expect(JSON.parse(readZipEntry(paket(OZEAN), 'manifest.json')).id).toBe('ozean');
    expect(JSON.parse(readZipEntry(paket(OZEAN, { deflate: true }), 'manifest.json')).id).toBe('ozean');
    expect(readZipEntry(paket(OZEAN), 'main.js')).toBeUndefined();
  });

  it('baut je Paket einen Eintrag mit Prüfsumme und Adresse am Tag, sortiert nach id', () => {
    const wald = paket(WALD, { deflate: true });
    const ozean = paket(OZEAN);
    const index = themeIndex([wald, ozean], { baseUrl: `${BASE}/` });
    expect(index).toEqual([
      {
        id: 'ozean', name: 'Ozean', version: '1.0.0', sdkVersion: '^1.1',
        downloadUrl: `${BASE}/ozean-1.0.0.nexus`,
        sha256: createHash('sha256').update(ozean).digest('hex'),
      },
      {
        id: 'wald', name: 'Wald', version: '2.1.0', sdkVersion: '^1.0',
        downloadUrl: `${BASE}/wald-2.1.0.nexus`,
        sha256: createHash('sha256').update(wald).digest('hex'),
        preview: 'https://example.org/wald.png',
      },
    ]);
    expect(index[0].sha256).toMatch(/^[0-9a-f]{64}$/);
  });

  it('verlinkt ein Vorschaubild aus dem Paket als Datei neben dem Paket am Release', () => {
    const png = Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0xff]);
    const manifest = { ...OZEAN, preview: 'assets/preview.png' };
    for (const deflate of [false, true]) {
      const bytes = zip({ 'manifest.json': JSON.stringify(manifest), 'assets/preview.png': png }, { deflate });
      expect(readZipBytes(bytes, 'assets/preview.png')).toEqual(png);
      expect(packagePreview(bytes, manifest)).toEqual({ name: 'ozean-1.0.0-preview.png', data: png });
      const [eintrag] = themeIndex([bytes], { baseUrl: BASE });
      expect(eintrag.preview).toBe(`${BASE}/ozean-1.0.0-preview.png`);
    }
  });

  it('lässt relative Vorschaubilder weg, die fehlen, kein Bild sind oder aus dem Paket zeigen', () => {
    const mit = (preview, files = {}) => zip({ 'manifest.json': JSON.stringify({ ...OZEAN, preview }), ...files });
    expect(themeIndex([paket({ ...OZEAN, preview: 'assets/preview.png' })], { baseUrl: BASE })[0]).not.toHaveProperty('preview');
    expect(themeIndex([mit('theme.css', { 'theme.css': ':root{}' })], { baseUrl: BASE })[0]).not.toHaveProperty('preview');
    expect(themeIndex([mit('../preview.png', { '../preview.png': 'x' })], { baseUrl: BASE })[0]).not.toHaveProperty('preview');
    expect(themeIndex([mit('http://example.org/a.png')], { baseUrl: BASE })[0]).not.toHaveProperty('preview');
  });

  it('benennt Pakete nach id und Version', () => {
    expect(packageName(OZEAN)).toBe('ozean-1.0.0.nexus');
  });

  it('bricht bei doppelter id, fehlendem Manifest, ungültiger id oder Version ab', () => {
    expect(() => themeIndex([paket(OZEAN), paket(OZEAN)], { baseUrl: BASE })).toThrow(/ozean.*doppelt/);
    expect(() => themeIndex([zip({ 'theme.css': '' })], { baseUrl: BASE })).toThrow(/manifest\.json fehlt/);
    expect(() => themeIndex([paket({ ...OZEAN, id: 'Ozean!' })], { baseUrl: BASE })).toThrow(/id/);
    expect(() => themeIndex([paket({ ...OZEAN, version: '1.0' })], { baseUrl: BASE })).toThrow(/version/);
    expect(() => themeIndex([paket({ ...OZEAN, sdkVersion: '' })], { baseUrl: BASE })).toThrow(/sdkVersion/);
    expect(() => themeIndex([Buffer.from('kein zip')], { baseUrl: BASE })).toThrow(/ZIP/);
  });

  it('verlangt eine https-Basisadresse', () => {
    expect(() => themeIndex([paket(OZEAN)], { baseUrl: 'http://example.org/x' })).toThrow(/https/);
    expect(() => themeIndex([paket(OZEAN)], { baseUrl: 'http://127.0.0.1:8080' })).not.toThrow();
  });

  it('die Referenz-Index-Datei passt zu den Manifesten unter themes/', () => {
    const ids = readdirSync(resolve(ROOT, 'themes'), { withFileTypes: true })
      .filter((e) => e.isDirectory() && !/^[._]/.test(e.name))
      .map((e) => e.name)
      .sort();
    const reference = JSON.parse(readFileSync(REFERENCE, 'utf8'));
    expect(reference.map((e) => e.id)).toEqual(ids);
    for (const eintrag of reference) {
      const manifest = JSON.parse(readFileSync(join(ROOT, 'themes', eintrag.id, 'manifest.json'), 'utf8'));
      expect(eintrag).toMatchObject({ id: manifest.id, name: manifest.name, version: manifest.version, sdkVersion: manifest.sdkVersion });
      expect(eintrag.downloadUrl).toBe(`https://github.com/BenjaminBiber/Nexus/releases/download/v0.0.0/${packageName(manifest)}`);
      // Jedes Theme bringt ein Vorschaubild mit, das der Store neben dem Paket findet – außer Themes mit fremden
      // Ressourcen (Minecraft): ein Bild mit deren Texturen darf nicht mitverteilt werden, eins ohne zeigt nur die Rückfall-Ansicht
      if ((manifest.permissions?.network ?? []).length > 0) {
        expect(manifest.preview, eintrag.id).toBeUndefined();
        expect(eintrag, eintrag.id).not.toHaveProperty('preview');
        continue;
      }
      expect(manifest.preview, eintrag.id).toBe('assets/preview.png');
      expect(eintrag.preview).toBe(`https://github.com/BenjaminBiber/Nexus/releases/download/v0.0.0/${manifest.id}-${manifest.version}-preview.png`);
    }
  });
});
