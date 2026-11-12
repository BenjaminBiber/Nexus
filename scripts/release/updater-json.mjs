// Schreibt latest.json für den Tauri-Updater aus den .sig-Dateien des Releases.
// Ersetzt `includeUpdaterJson` von tauri-action für Builds außerhalb von GitHub Actions (Gitea Actions).
// Aufruf: node scripts/release/updater-json.mjs --version <X.Y.Z> --tag <vX.Y.Z> --repo <owner/name> --sigs <ordner> --out <datei>
//   --sigs: Ordner mit den heruntergeladenen <datei>.sig des Releases. Die Notizen kommen aus CHANGELOG.md.
import { execFileSync } from 'node:child_process';
import { readdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { updaterJson } from './lib/UpdaterJson.mjs';

const HIER = dirname(fileURLToPath(import.meta.url));

function fail(message) {
  console.error(message);
  process.exit(1);
}

function options(argv) {
  const value = (name) => {
    const at = argv.indexOf(name);
    const raw = at < 0 ? undefined : argv[at + 1];
    if (!raw || raw.startsWith('--')) fail(`Es fehlt ${name} <wert>.`);
    return raw;
  };
  return { version: value('--version'), tag: value('--tag'), repo: value('--repo'), sigs: value('--sigs'), out: value('--out') };
}

const { version, tag, repo, sigs, out } = options(process.argv.slice(2));

// Dateiname am Release (ohne .sig) → Inhalt der Signatur
const signatures = Object.fromEntries(
  readdirSync(sigs)
    .filter((name) => name.endsWith('.sig'))
    .map((name) => [name.slice(0, -4), readFileSync(join(sigs, name), 'utf8')]),
);
try {
  // bump.mjs meldet einen fehlenden Abschnitt selbst auf stderr
  const notes = execFileSync(process.execPath, [resolve(HIER, 'bump.mjs'), 'notes', version], {
    encoding: 'utf8',
    stdio: ['ignore', 'pipe', 'inherit'],
  });
  const json = updaterJson({ version, tag, repo, notes, pubDate: new Date().toISOString(), signatures });
  writeFileSync(out, `${JSON.stringify(json, null, 2)}\n`);
  console.log(`${out}: ${Object.keys(json.platforms).length} Plattformen.`);
} catch (error) {
  fail(error.message);
}
