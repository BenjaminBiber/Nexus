// Inhalts-Hash eines gebauten Plugins: gleicher Inhalt ergibt denselben Hash, auch wenn sich die
// Build-Version (Zeitstempel) unterscheidet. Die App vergleicht ihn beim Start mit dem installierten
// Plugin und bietet ein Update nur an, wenn sich der Inhalt wirklich geändert hat.
import { createHash } from 'node:crypto';
import { readdirSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

// Datei mit dem Hash im Plugin-Ordner; zählt selbst nicht mit
export const CONTENT_HASH_FILE = 'content-hash';

// Felder, die sich bei jedem Build ändern, ohne dass sich das Plugin ändert
const VOLATILE_FIELDS = {
  '.claude-plugin/plugin.json': ['version'],
  'customizations/overview.json': ['createdAt', 'harnessVersion', 'cloneDir', 'customDir'],
};

// Alle Dateien unter `dir`, relativ und mit `/` getrennt, sortiert
function listFiles(dir, prefix = '') {
  const files = [];
  for (const entry of readdirSync(join(dir, prefix), { withFileTypes: true })) {
    const rel = prefix ? `${prefix}/${entry.name}` : entry.name;
    if (entry.isDirectory()) files.push(...listFiles(dir, rel));
    else if (entry.isFile()) files.push(rel);
  }
  return files.sort();
}

// Inhalt einer Datei für den Hash; bei JSON-Dateien mit wechselnden Feldern ohne diese
function stableContent(dir, rel) {
  const bytes = readFileSync(join(dir, rel));
  const volatile = VOLATILE_FIELDS[rel];
  if (!volatile) return bytes;
  try {
    const value = JSON.parse(bytes.toString('utf8'));
    for (const field of volatile) delete value[field];
    return Buffer.from(JSON.stringify(value));
  } catch {
    return bytes;
  }
}

// SHA-256 über Pfade und Inhalte aller Dateien des Plugins
export function contentHash(dir) {
  const hash = createHash('sha256');
  for (const rel of listFiles(dir)) {
    if (rel === CONTENT_HASH_FILE) continue;
    const content = stableContent(dir, rel);
    hash.update(`${rel}\0${content.length}\0`);
    hash.update(content);
  }
  return hash.digest('hex');
}

// Berechnet den Hash und legt ihn als `content-hash` ins Plugin
export function writeContentHash(dir) {
  const value = contentHash(dir);
  writeFileSync(join(dir, CONTENT_HASH_FILE), `${value}\n`);
  return value;
}
