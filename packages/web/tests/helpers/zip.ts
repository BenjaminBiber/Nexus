import type { ThemeManifest } from '../../src/api/generated/themes.js';

// Baut ein minimales ZIP (Methode 0 = gespeichert, 8 = deflate) für Tests des Paketlesers
export async function buildZip(entries: Record<string, string>, method: 0 | 8 = 0): Promise<Uint8Array<ArrayBuffer>> {
  const enc = new TextEncoder();
  const locals: Uint8Array[] = [];
  const centrals: Uint8Array[] = [];
  let offset = 0;
  for (const [name, content] of Object.entries(entries)) {
    const nameBytes = enc.encode(name);
    const raw = enc.encode(content);
    const data = method === 8 ? new Uint8Array(await new Response(new Blob([raw]).stream().pipeThrough(new CompressionStream('deflate-raw'))).arrayBuffer()) : raw;
    const local = new Uint8Array(30 + nameBytes.length + data.length);
    const lv = new DataView(local.buffer);
    lv.setUint32(0, 0x04034b50, true);
    lv.setUint16(8, method, true);
    lv.setUint32(18, data.length, true);
    lv.setUint32(22, raw.length, true);
    lv.setUint16(26, nameBytes.length, true);
    local.set(nameBytes, 30);
    local.set(data, 30 + nameBytes.length);
    const central = new Uint8Array(46 + nameBytes.length);
    const cv = new DataView(central.buffer);
    cv.setUint32(0, 0x02014b50, true);
    cv.setUint16(10, method, true);
    cv.setUint32(20, data.length, true);
    cv.setUint32(24, raw.length, true);
    cv.setUint16(28, nameBytes.length, true);
    cv.setUint32(42, offset, true);
    central.set(nameBytes, 46);
    locals.push(local);
    centrals.push(central);
    offset += local.length;
  }
  const cdSize = centrals.reduce((n, c) => n + c.length, 0);
  const eocd = new Uint8Array(22);
  const ev = new DataView(eocd.buffer);
  ev.setUint32(0, 0x06054b50, true);
  ev.setUint16(8, centrals.length, true);
  ev.setUint16(10, centrals.length, true);
  ev.setUint32(12, cdSize, true);
  ev.setUint32(16, offset, true);
  const out = new Uint8Array(offset + cdSize + 22);
  let pos = 0;
  for (const part of [...locals, ...centrals, eocd]) {
    out.set(part, pos);
    pos += part.length;
  }
  return out;
}

export const testManifest = (extra: Partial<ThemeManifest> = {}): ThemeManifest => ({
  id: 'ocean', name: 'Ocean', version: '1.2.0', author: 'Ada', sdkVersion: '^1.0', colorScheme: 'dark', startColor: '#123456', preview: null, slots: [], permissions: { network: [] }, ...extra,
});

// .nexus-Datei mit Manifest, theme.css und optional main.js
export async function themeFile(manifest: ThemeManifest, withCode = false, name = 'ocean.nexus'): Promise<File> {
  const entries: Record<string, string> = { 'manifest.json': JSON.stringify(manifest), 'theme.css': ':root{}' };
  if (withCode) entries['main.js'] = 'export default {}';
  return new File([await buildZip(entries)], name);
}
