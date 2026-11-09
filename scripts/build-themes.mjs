#!/usr/bin/env node
// Packt jeden Theme-Ordner `themes/<id>/` mit `nexus theme pack` nach `dist/themes/<id>.nexus`.
// Die Pakete liefern Server und Desktop-App vorinstalliert mit (siehe themes/README.md).
//
// Optionen: --src <ordner> (Standard themes), --out <ordner> (Standard dist/themes),
// --nexus "<befehl>" (Standard: NEXUS_BIN, sonst `cargo run -q -p nexus-cli --bin nexus --`).
// Ein ungültiges Theme beendet das Skript mit Exit-Code 1.

import { spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, readFileSync, readdirSync, rmSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');

/** Wert eines Schalters mit Wert (`--src themes`) */
function option(args, name) {
  const at = args.indexOf(name);
  return at >= 0 ? args[at + 1] : undefined;
}

/** Befehl und Argumente des CLI */
function nexusCommand(args) {
  const given = option(args, '--nexus');
  if (given) {
    const [command, ...rest] = given.split(/\s+/).filter(Boolean);
    return { command, args: rest };
  }
  if (process.env.NEXUS_BIN) return { command: process.env.NEXUS_BIN, args: [] };
  return { command: 'cargo', args: ['run', '-q', '-p', 'nexus-cli', '--bin', 'nexus', '--'] };
}

function main(args) {
  const src = resolve(root, option(args, '--src') ?? 'themes');
  const out = resolve(root, option(args, '--out') ?? 'dist/themes');
  const nexus = nexusCommand(args);

  // Der Ausgabeordner existiert immer, damit die Ressource der Desktop-App da ist
  rmSync(out, { recursive: true, force: true });
  mkdirSync(out, { recursive: true });

  const dirs = existsSync(src)
    ? readdirSync(src, { withFileTypes: true })
        .filter((entry) => entry.isDirectory() && !entry.name.startsWith('.'))
        .map((entry) => entry.name)
        .filter((name) => existsSync(join(src, name, 'manifest.json')))
        .sort()
    : [];
  if (dirs.length === 0) {
    console.log(`Keine Themes in ${src} – ${out} bleibt leer.`);
    return 0;
  }

  for (const id of dirs) {
    const dir = join(src, id);
    let manifestId;
    try {
      manifestId = JSON.parse(readFileSync(join(dir, 'manifest.json'), 'utf8')).id;
    } catch (error) {
      console.error(`Theme ${id}: manifest.json ist kein gültiges JSON (${error.message}).`);
      return 1;
    }
    if (manifestId !== id) {
      console.error(`Theme ${id}: Ordnername und Manifest-ID (${manifestId}) weichen ab.`);
      return 1;
    }
    const target = join(out, `${id}.nexus`);
    const result = spawnSync(nexus.command, [...nexus.args, 'theme', 'pack', dir, '--out', target], {
      cwd: root,
      encoding: 'utf8',
    });
    if (result.status !== 0) {
      console.error(`Theme ${id} konnte nicht gepackt werden:`);
      console.error(result.error?.message ?? `${result.stdout ?? ''}${result.stderr ?? ''}`.trim());
      rmSync(target, { force: true });
      return 1;
    }
    console.log(`Theme ${id} → ${target}`);
  }
  return 0;
}

process.exitCode = main(process.argv.slice(2));
