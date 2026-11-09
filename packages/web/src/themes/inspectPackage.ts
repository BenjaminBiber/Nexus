import type { ThemeManifest } from '../api/generated/themes.js';

// Lesefehler eines .nexus-Pakets im Browser; die Meldung ist für den Nutzer gedacht
export class ThemePackageReadError extends Error {
  constructor(message: string) {
    super(message);
    this.name = 'ThemePackageReadError';
  }
}

type Entry = { name: string; method: number; compressedSize: number; localOffset: number };

const NOT_ZIP = 'Die Datei ist kein gültiges Theme-Paket (kein ZIP-Archiv).';

// Sucht das End-of-Central-Directory vom Dateiende her (Kommentar bis 64 KiB)
function readCentralDirectory(bytes: Uint8Array): Entry[] {
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const min = Math.max(0, bytes.length - 22 - 0xffff);
  let eocd = -1;
  for (let i = bytes.length - 22; i >= min; i--) {
    if (view.getUint32(i, true) === 0x06054b50) { eocd = i; break; }
  }
  if (eocd < 0) throw new ThemePackageReadError(NOT_ZIP);
  const count = view.getUint16(eocd + 10, true);
  let pos = view.getUint32(eocd + 16, true);
  const decoder = new TextDecoder();
  const entries: Entry[] = [];
  for (let i = 0; i < count; i++) {
    if (pos + 46 > bytes.length || view.getUint32(pos, true) !== 0x02014b50) throw new ThemePackageReadError(NOT_ZIP);
    const nameLength = view.getUint16(pos + 28, true);
    entries.push({
      name: decoder.decode(bytes.subarray(pos + 46, pos + 46 + nameLength)),
      method: view.getUint16(pos + 10, true),
      compressedSize: view.getUint32(pos + 20, true),
      localOffset: view.getUint32(pos + 42, true),
    });
    pos += 46 + nameLength + view.getUint16(pos + 30, true) + view.getUint16(pos + 32, true);
  }
  return entries;
}

// Liest die Daten eines Eintrags über den Local Header; Methode 0 (gespeichert) oder 8 (deflate)
async function readEntry(bytes: Uint8Array, entry: Entry): Promise<Uint8Array> {
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const at = entry.localOffset;
  if (at + 30 > bytes.length || view.getUint32(at, true) !== 0x04034b50) throw new ThemePackageReadError(NOT_ZIP);
  const start = at + 30 + view.getUint16(at + 26, true) + view.getUint16(at + 28, true);
  const data = bytes.subarray(start, start + entry.compressedSize);
  if (entry.method === 0) return data;
  if (entry.method !== 8) throw new ThemePackageReadError('Die manifest.json ist mit einem nicht unterstützten Verfahren gepackt.');
  try {
    // Stream über Response statt Blob.stream(): ältere Umgebungen (z. B. jsdom) kennen Blob.stream nicht
    const stream = new Response(data.slice()).body!.pipeThrough(new DecompressionStream('deflate-raw'));
    return new Uint8Array(await new Response(stream).arrayBuffer());
  } catch {
    throw new ThemePackageReadError('Die manifest.json konnte nicht entpackt werden.');
  }
}

async function blobBytes(file: Blob): Promise<Uint8Array> {
  if (typeof file.arrayBuffer === 'function') return new Uint8Array(await file.arrayBuffer());
  // Ältere Umgebungen (z. B. jsdom) kennen Blob.arrayBuffer nicht
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(new Uint8Array(reader.result as ArrayBuffer));
    reader.onerror = () => reject(new ThemePackageReadError('Die Datei konnte nicht gelesen werden.'));
    reader.readAsArrayBuffer(file);
  });
}

// Liest Manifest und Code-Hinweis eines .nexus-Pakets im Browser, ohne zu installieren.
// Nur defensives Lesen für den Installationsdialog – die volle Prüfung bleibt beim Server.
export async function inspectThemePackage(file: Blob): Promise<{ manifest: ThemeManifest; hasCode: boolean }> {
  const bytes = await blobBytes(file);
  const entries = readCentralDirectory(bytes);
  const manifestEntry = entries.find((e) => e.name === 'manifest.json');
  if (!manifestEntry) throw new ThemePackageReadError('Die Datei ist kein gültiges Theme-Paket: manifest.json fehlt.');
  let raw: unknown;
  try {
    raw = JSON.parse(new TextDecoder().decode(await readEntry(bytes, manifestEntry)));
  } catch (err) {
    if (err instanceof ThemePackageReadError) throw err;
    throw new ThemePackageReadError('Die manifest.json des Themes ist kein gültiges JSON.');
  }
  if (!raw || typeof raw !== 'object') throw new ThemePackageReadError('Die manifest.json des Themes ist kein gültiges JSON.');
  const m = raw as Partial<ThemeManifest>;
  const network = Array.isArray(m.permissions?.network) ? m.permissions.network.filter((d): d is string => typeof d === 'string') : [];
  const text = (value: unknown) => (typeof value === 'string' ? value : '');
  const manifest = { ...m, name: text(m.name) || text(m.id), author: text(m.author), version: text(m.version), permissions: { ...m.permissions, network } } as ThemeManifest;
  return { manifest, hasCode: entries.some((e) => e.name === 'main.js') };
}
