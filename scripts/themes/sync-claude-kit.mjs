#!/usr/bin/env node
// Gleicht die Claude-Themes unter themes/<id>/claude/ mit dem Kit ab: kopiert make-colors.js,
// make-colors.d.ts und nexus-look.tsx aus themes/_claude-kit/ nach claude/hooks/ und schreibt
// claude/hooks/nexus-settings.ts mit den Standardwerten aus dem settingsSchema des Manifests
// (im Slot schreibt Nexus dieselbe Datei mit den gewählten Werten, siehe claude_slot.rs).
//
// Gilt für jedes Theme mit `"claudeTheme": true` im Manifest.
// Optionen: --check (nichts schreiben, Exit-Code 1 bei Abweichung), --root <ordner> (Standard: Repo).

import { existsSync, mkdirSync, readFileSync, readdirSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

/** Dateien des Kits, die jedes Claude-Theme als Kopie in claude/hooks/ trägt */
export const KIT_FILES = ['make-colors.js', 'make-colors.d.ts', 'nexus-look.tsx'];

/** Kopfzeile von nexus-settings.ts; gleich in claude_slot.rs */
export const SETTINGS_HEADER =
  '// Von Nexus erzeugt – nicht bearbeiten. Im Quellordner des Themes stehen hier die Standardwerte.';

/** Standardwerte eines settingsSchema, Schlüssel sortiert */
export function defaultsOf(schema) {
  const values = {};
  for (const key of Object.keys(schema ?? {}).sort()) values[key] = schema[key].default;
  return values;
}

/** Inhalt von nexus-settings.ts für diese Werte (Schlüssel sortiert, JSON wie serde_json) */
export function settingsModule(values) {
  const sorted = {};
  for (const key of Object.keys(values).sort()) sorted[key] = values[key];
  return `${SETTINGS_HEADER}\nexport const settings: Readonly<Record<string, string | boolean>> = ${JSON.stringify(sorted)}\n`;
}

/** Ordner der Themes mit Claude-Theme unter `<root>/themes`, nach ID sortiert */
export function claudeThemes(root) {
  const themesDir = join(root, 'themes');
  return readdirSync(themesDir, { withFileTypes: true })
    .filter((entry) => entry.isDirectory() && !entry.name.startsWith('_') && !entry.name.startsWith('.'))
    .map((entry) => join(themesDir, entry.name))
    .filter((dir) => existsSync(join(dir, 'manifest.json')))
    .map((dir) => ({ dir, manifest: JSON.parse(readFileSync(join(dir, 'manifest.json'), 'utf8')) }))
    .filter(({ manifest }) => manifest.claudeTheme === true)
    .sort((a, b) => a.manifest.id.localeCompare(b.manifest.id));
}

/** Soll-Dateien eines Themes: Pfad unter claude/hooks/ → Inhalt */
export function expectedFiles(root, manifest) {
  const kit = join(root, 'themes', '_claude-kit');
  const files = new Map(KIT_FILES.map((name) => [name, readFileSync(join(kit, name), 'utf8')]));
  files.set('nexus-settings.ts', settingsModule(defaultsOf(manifest.settingsSchema)));
  return files;
}

/** Gleicht alle Themes ab; liefert die abweichenden Dateien (bei `check` ohne zu schreiben) */
export function syncClaudeKit(root, { check = false } = {}) {
  const changed = [];
  for (const { dir, manifest } of claudeThemes(root)) {
    for (const [name, content] of expectedFiles(root, manifest)) {
      const target = join(dir, 'claude', 'hooks', name);
      const current = existsSync(target) ? readFileSync(target, 'utf8') : null;
      if (current === content) continue;
      changed.push(`themes/${manifest.id}/claude/hooks/${name}`);
      if (check) continue;
      mkdirSync(dirname(target), { recursive: true });
      writeFileSync(target, content);
    }
  }
  return changed;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const args = process.argv.slice(2);
  const at = args.indexOf('--root');
  const root = at >= 0 ? resolve(args[at + 1]) : resolve(dirname(fileURLToPath(import.meta.url)), '..', '..');
  const check = args.includes('--check');
  const changed = syncClaudeKit(root, { check });
  for (const path of changed) console.log(`${check ? 'weicht ab' : 'geschrieben'}: ${path}`);
  if (check && changed.length > 0) process.exit(1);
}
