// Baut den Store-Index `nexus-themes.json` aus fertigen Theme-Paketen (`.nexus` = ZIP mit manifest.json).
// Je Paket ein Eintrag `{ id, name, version, sdkVersion, downloadUrl, sha256, preview? }` im Format, das
// `parse_index` in crates/nexus-core/src/theme/store_index.rs prüft. Reine Funktionen auf Bytes, damit sich das
// ohne Build und ohne Netz testen lässt; Lesen und Schreiben der Dateien steckt in theme-index.mjs.

import { createHash } from 'node:crypto';
import { inflateRawSync } from 'node:zlib';
import { tryParse } from './SemVer.mjs';

// Wie `is_valid_theme_id` im Kern
const ID_PATTERN = /^[a-z0-9-]{2,40}$/;

// Liest eine Datei aus einem ZIP (gespeichert oder Deflate) über das zentrale Verzeichnis; undefined, wenn sie fehlt
export function readZipEntry(bytes, wanted) {
  const buf = Buffer.from(bytes);
  let end = -1;
  for (let at = buf.length - 22; at >= Math.max(0, buf.length - 22 - 0xffff); at--) {
    if (buf.readUInt32LE(at) === 0x06054b50) {
      end = at;
      break;
    }
  }
  if (end < 0) throw new Error('Das Paket ist kein ZIP-Archiv.');
  const count = buf.readUInt16LE(end + 10);
  let at = buf.readUInt32LE(end + 16);
  for (let i = 0; i < count; i++) {
    if (buf.readUInt32LE(at) !== 0x02014b50) throw new Error('Das ZIP-Verzeichnis des Pakets ist beschädigt.');
    const method = buf.readUInt16LE(at + 10);
    const size = buf.readUInt32LE(at + 20);
    const nameLength = buf.readUInt16LE(at + 28);
    const extraLength = buf.readUInt16LE(at + 30);
    const commentLength = buf.readUInt16LE(at + 32);
    const offset = buf.readUInt32LE(at + 42);
    const name = buf.toString('utf8', at + 46, at + 46 + nameLength);
    at += 46 + nameLength + extraLength + commentLength;
    if (name !== wanted) continue;
    const start = offset + 30 + buf.readUInt16LE(offset + 26) + buf.readUInt16LE(offset + 28);
    const data = buf.subarray(start, start + size);
    if (method === 0) return data.toString('utf8');
    if (method === 8) return inflateRawSync(data).toString('utf8');
    throw new Error(`${wanted} ist mit einer unbekannten Methode (${method}) gepackt.`);
  }
  return undefined;
}

// Dateiname eines Pakets am Release: `<id>-<version>.nexus`
export function packageName(manifest) {
  return `${manifest.id}-${manifest.version}.nexus`;
}

// Manifest aus den Paketbytes mit den Feldern, die der Index braucht; wirft bei Fehlern
export function packageManifest(bytes) {
  const text = readZipEntry(bytes, 'manifest.json');
  if (text === undefined) throw new Error('manifest.json fehlt im Paket.');
  let manifest;
  try {
    manifest = JSON.parse(text);
  } catch {
    throw new Error('manifest.json im Paket ist kein gültiges JSON.');
  }
  const { id, name, version, sdkVersion } = manifest;
  if (typeof id !== 'string' || !ID_PATTERN.test(id)) throw new Error(`Ungültige id „${id}“ (2–40 Zeichen aus a–z, 0–9 und -).`);
  if (typeof name !== 'string' || !name.trim()) throw new Error(`Theme ${id}: name fehlt.`);
  if (!tryParse(version)) throw new Error(`Theme ${id}: version „${version}“ ist keine semver-Version.`);
  if (typeof sdkVersion !== 'string' || !sdkVersion.trim()) throw new Error(`Theme ${id}: sdkVersion fehlt.`);
  return manifest;
}

// https, http nur für localhost/127.0.0.1 (lokale Handprobe) – wie `check_url` im Kern
function checkBaseUrl(baseUrl) {
  let url;
  try {
    url = new URL(baseUrl);
  } catch {
    throw new Error(`Basisadresse „${baseUrl}“ ist keine gültige Adresse.`);
  }
  const local = url.hostname === 'localhost' || url.hostname === '127.0.0.1';
  if (url.protocol !== 'https:' && !(url.protocol === 'http:' && local)) {
    throw new Error(`Basisadresse „${baseUrl}“ muss https sein (http nur für localhost).`);
  }
}

// Index als Array, sortiert nach id. `packages` sind die Bytes der Pakete, `baseUrl` die Adresse, unter der
// die Pakete als `<id>-<version>.nexus` liegen (am Release: …/releases/download/<tag>)
export function themeIndex(packages, { baseUrl }) {
  checkBaseUrl(baseUrl);
  const base = baseUrl.replace(/\/+$/, '');
  const seen = new Set();
  const entries = packages.map((bytes) => {
    const manifest = packageManifest(bytes);
    if (seen.has(manifest.id)) throw new Error(`Theme ${manifest.id} kommt doppelt vor.`);
    seen.add(manifest.id);
    const entry = {
      id: manifest.id,
      name: manifest.name,
      version: manifest.version,
      sdkVersion: manifest.sdkVersion,
      downloadUrl: `${base}/${packageName(manifest)}`,
      sha256: createHash('sha256').update(bytes).digest('hex'),
    };
    // Relative Vorschaubilder liegen im Paket und sind für den Store nicht erreichbar
    if (typeof manifest.preview === 'string' && manifest.preview.startsWith('https://')) entry.preview = manifest.preview;
    if (typeof manifest.description === 'string' && manifest.description.trim()) entry.description = manifest.description;
    return entry;
  });
  return entries.sort((a, b) => (a.id < b.id ? -1 : a.id > b.id ? 1 : 0));
}
