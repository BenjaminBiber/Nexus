import { readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { release, section } from './lib/Changelog.mjs';
import { KINDS, bump, parse } from './lib/SemVer.mjs';
import { highestTag, listTags, tagVersions } from './lib/Tags.mjs';
import { VersionFiles } from './lib/VersionFiles.mjs';

// Ohne --root gilt das Repo, in dem dieses Skript liegt – so klappt der Aufruf aus jedem Ordner
const DEFAULT_ROOT = fileURLToPath(new URL('../..', import.meta.url));
// Alle Befehle; `--check` wird als `check` gelesen, weil Release-Skripte ihn so schreiben
const COMMANDS = ['next', 'set', 'check', 'notes', 'current'];

const HELP = `Eine Version für das ganze Repo: berechnen, setzen, prüfen.

  node scripts/release/bump.mjs next <${KINDS.join('|')}> [--root <dir>]
  node scripts/release/bump.mjs set <X.Y.Z> [--changelog] [--date YYYY-MM-DD] [--root <dir>]
  node scripts/release/bump.mjs check [--root <dir>]
  node scripts/release/bump.mjs notes <X.Y.Z> [--root <dir>]
  node scripts/release/bump.mjs current [--root <dir>]

  next <stelle>    gibt die nächste Version aus: höchstes Tag „v*“ (ohne Vorabversionen),
                   sonst die Version der Quelle, um eine Stelle erhöht
  set <X.Y.Z>      setzt alle Versionsstellen und prüft danach selbst (Code 1 bei Abweichung)
  check            Code 0, wenn alle Stellen gleich sind, sonst Code 1 und eine Liste
  notes <X.Y.Z>    gibt den Abschnitt dieser Version aus CHANGELOG.md aus
  current          gibt die Version der Quelle aus

  --changelog      benennt „## Unveröffentlicht“ in „## X.Y.Z – <datum>“ um und setzt einen leeren
                   Abschnitt „Unveröffentlicht“ darüber
  --date <datum>   Datum der Überschrift (Standard: heute)
  --root <dir>     Wurzel des Repos (Standard: das Repo dieses Skripts)
  --help           zeigt diese Hilfe
`;

// Ergebnisse gehen auf stdout, Hinweise und Warnungen auf stderr – so bleibt „next“ in einer Variablen brauchbar
const out = (line) => process.stdout.write(`${line}\n`);
const warn = (line) => process.stderr.write(`${line}\n`);

// Heutiges Datum als YYYY-MM-DD (örtliche Zeit, denn das Datum steht für Menschen im Changelog)
export function today(now = new Date()) {
  const pad = (value) => String(value).padStart(2, '0');
  return `${now.getFullYear()}-${pad(now.getMonth() + 1)}-${pad(now.getDate())}`;
}

// Liest die Argumente
export function parseArgs(argv) {
  const options = { command: null, value: null, root: DEFAULT_ROOT, changelog: false, date: null, help: false };
  let index = 0;
  while (index < argv.length) {
    const arg = argv[index];
    const value = argv[index + 1];
    const needsValue = () => {
      if (value === undefined) throw new Error(`${arg} braucht einen Wert.`);
      index += 2;
      return value;
    };
    // `--check` ist derselbe Befehl wie `check`
    const asCommand = arg.startsWith('--') && COMMANDS.includes(arg.slice(2)) ? arg.slice(2) : arg;
    if (arg === '--help' || arg === '-h') {
      options.help = true;
      index += 1;
    } else if (arg === '--changelog') {
      options.changelog = true;
      index += 1;
    } else if (arg === '--root') options.root = needsValue();
    else if (arg === '--date') options.date = needsValue();
    else if (options.command === null) {
      if (!COMMANDS.includes(asCommand)) throw new Error(`Unbekannter Befehl „${arg}“. --help zeigt die Möglichkeiten.`);
      options.command = asCommand;
      index += 1;
    } else if (options.value === null && !arg.startsWith('-')) {
      options.value = arg;
      index += 1;
    } else throw new Error(`Unbekanntes Argument „${arg}“. --help zeigt die Möglichkeiten.`);
  }
  return options;
}

// Die nächste Version: höchstes Tag „v*“, sonst die Quelle – um `kind` erhöht
function nextVersion(files, root, kind) {
  const tags = listTags(root);
  if (tags === null) {
    warn('Kein Git-Repo gefunden – gerechnet wird ab der Version der Quelle.');
    return bump(files.current(), kind);
  }
  const { prerelease, skipped } = tagVersions(tags);
  for (const tag of skipped) warn(`Tag „${tag}“ ist keine Version – übersprungen.`);
  if (prerelease.length) warn(`Vorabversionen zählen nicht mit: ${prerelease.join(', ')}.`);
  const highest = highestTag(tags);
  if (highest === null) warn('Kein Tag „v*“ mit einer Release-Version – gerechnet wird ab der Version der Quelle.');
  return bump(highest ?? files.current(), kind);
}

// Zeigt die Stellen eines Vergleichs als Liste
function listPlaces(places) {
  const width = Math.max(...places.map((place) => place.file.length));
  for (const place of places) {
    out(`  ${place.file.padEnd(width)}  ${place.where}: ${place.version ?? 'fehlt'}`);
  }
}

// `check`: alle Stellen gegen die Quelle. Liefert true, wenn alles gleich ist
function runCheck(files) {
  const { source, places, deviations, removals, warnings } = files.check();
  for (const line of warnings) warn(line);
  out(`Quelle: ${source.file} (${source.where}) = ${source.version}`);
  if (deviations.length === 0 && removals.length === 0) {
    out(`${places.length} Stellen geprüft, alle auf ${source.version}.`);
    return true;
  }
  if (deviations.length) {
    out(`${deviations.length} von ${places.length} Stellen weichen ab:`);
    listPlaces(deviations);
  }
  if (removals.length) {
    out(`${removals.length} Stelle(n) gehören dort nicht hin:`);
    listPlaces(removals);
  }
  return false;
}

// Liest CHANGELOG.md der Wurzel
function readChangelog(root) {
  const file = join(root, 'CHANGELOG.md');
  try {
    return { file, text: readFileSync(file, 'utf8') };
  } catch {
    throw new Error(`${file} lässt sich nicht lesen.`);
  }
}

// `set`: alle Stellen setzen, auf Wunsch den Changelog umbenennen, danach selbst prüfen
function runSet(files, options) {
  const version = options.value;
  parse(version);
  const date = options.date ?? today();
  // Den neuen Changelog vor dem Schreiben berechnen: ein fehlender Abschnitt soll auffallen,
  // bevor die Version in zehn Dateien steht
  const changelog = options.changelog ? readChangelog(options.root) : null;
  const renamed = changelog ? release(changelog.text, version, date) : null;
  if (!options.changelog && options.date) warn('--date wirkt nur zusammen mit --changelog.');
  const { changed, warnings } = files.set(version);
  for (const line of warnings) warn(line);
  out(changed.length ? `${changed.length} Datei(en) auf ${version} gesetzt:` : `Alle Stellen stehen schon auf ${version}.`);
  for (const file of changed) out(`  ${file}`);
  if (changelog) {
    writeFileSync(changelog.file, renamed);
    out(`  CHANGELOG.md (Abschnitt „${version} – ${date}“)`);
  }
  return runCheck(files);
}

// `notes`: der Abschnitt dieser Version aus CHANGELOG.md
function runNotes(options) {
  const version = options.value;
  parse(version);
  const body = section(readChangelog(options.root).text, version);
  if (body === null) throw new Error(`In CHANGELOG.md gibt es keinen Abschnitt für ${version}.`);
  out(body);
  return true;
}

// Einstieg: Argumente lesen, Befehl ausführen, Code 1 bei Abweichung oder Fehler
function main(argv) {
  const options = parseArgs(argv);
  if (options.help || options.command === null) {
    process.stdout.write(HELP);
    return options.command === null && !options.help ? 1 : 0;
  }
  const files = new VersionFiles(options.root);
  const needsValue = (what) => {
    if (options.value === null) throw new Error(`„${options.command}“ braucht ${what}.`);
  };
  if (options.command === 'current') {
    out(files.current());
    return 0;
  }
  if (options.command === 'next') {
    needsValue(`eine Stelle (${KINDS.join(', ')})`);
    out(nextVersion(files, options.root, options.value));
    return 0;
  }
  if (options.command === 'notes') {
    needsValue('eine Version (X.Y.Z)');
    return runNotes(options) ? 0 : 1;
  }
  if (options.command === 'set') {
    needsValue('eine Version (X.Y.Z)');
    return runSet(files, options) ? 0 : 1;
  }
  return runCheck(files) ? 0 : 1;
}

// Als Programm aufgerufen? Beim Import (Tests) läuft nichts von allein
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  try {
    process.exitCode = main(process.argv.slice(2));
  } catch (error) {
    warn(`Abbruch: ${error instanceof Error ? error.message : error}`);
    process.exitCode = 1;
  }
}
