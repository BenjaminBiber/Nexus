#!/usr/bin/env node
// Packt jeden Theme-Ordner `themes/<id>/` mit `nexus theme pack` nach `dist/themes/<id>.nexus`.
// Code-Themes mit package.json und Skript `build` werden vorher gebaut; gepackt wird dann ihr `dist/`.
// Die Pakete sind für Release und Theme-Store; mitgeliefert werden sie nicht (siehe themes/README.md).
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

/**
 * Code-Themes (package.json mit Skript `build`, z. B. themes/win95) erst bauen; gepackt wird dann `dist/`.
 * Liefert den zu packenden Ordner oder null, wenn der Build fehlschlägt.
 */
function buildCodeTheme(dir) {
  const pkgPath = join(dir, 'package.json');
  if (!existsSync(pkgPath)) return dir;
  let scripts;
  try {
    scripts = JSON.parse(readFileSync(pkgPath, 'utf8')).scripts;
  } catch {
    return null;
  }
  if (!scripts?.build) return dir;
  // npm findet vite & Co. über die node_modules/.bin der übergeordneten Ordner (Repo-Wurzel)
  // Fester Befehl ohne Argumente; die Shell braucht es unter Windows für npm.cmd
  const result = spawnSync('npm run build', { cwd: dir, encoding: 'utf8', shell: true });
  if (result.status !== 0) {
    console.error(result.error?.message ?? `${result.stdout ?? ''}${result.stderr ?? ''}`.trim());
    return null;
  }
  return join(dir, 'dist');
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
        // Punkt-Ordner sind versteckt, Unterstrich-Ordner Vorlagen (themes/_template)
        .filter((entry) => entry.isDirectory() && !/^[._]/.test(entry.name))
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
    const packDir = buildCodeTheme(dir);
    if (packDir === null) {
      console.error(`Theme ${id} konnte nicht gebaut werden (npm run build in ${dir}).`);
      return 1;
    }
    const result = spawnSync(nexus.command, [...nexus.args, 'theme', 'pack', packDir, '--out', target], {
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
