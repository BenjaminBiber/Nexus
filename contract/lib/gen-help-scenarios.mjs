#!/usr/bin/env node
import { spawnSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { CliStep } from './CliStep.mjs';
import { ServerHandle } from './ServerHandle.mjs';

const CONTRACT_DIR = fileURLToPath(new URL('..', import.meta.url));
const REPO_ROOT = fileURLToPath(new URL('../..', import.meta.url));
const DEFAULT_CLI_CMD = 'node packages/cli/dist/main.js';
const TARGET = join(CONTRACT_DIR, 'cli', 'help', 'help.scenario.json');

// Überschrift der Unterbefehle (deutsch aus GermanHelp.ts, englisch als Rückfall)
const COMMAND_HEADING = /^(?:Befehle|Commands):$/;
// Eintrag eines Unterbefehls: genau zwei Leerzeichen, dann der Name. Fortsetzungszeilen stehen weiter rechts.
const COMMAND_LINE = /^ {2}(\S+)/;
// Der eingebaute Befehl `help` zeigt die Hilfe des übergeordneten Befehls – kein eigener Hilfetext
const BUILTIN_HELP = 'help';

// Fehlerfälle von commander, übersetzt in GermanErrors.ts; sie gehören genauso zum Vertrag wie die Hilfe.
// „Zu viele Argumente“ fehlt hier, weil die CLI überzählige Argumente nicht ablehnt (kein allowExcessArguments(false)).
const ERROR_STEPS = [
  { name: 'unbekannter Befehl', args: ['quatsch'] },
  { name: 'unbekannter Befehl mit Vorschlag', args: ['bord'] },
  { name: 'unbekannter Unterbefehl', args: ['board', 'quatsch'] },
  { name: 'fehlendes Pflichtargument', args: ['board', 'show'] },
  { name: 'unbekannte Option des Programms', args: ['--quatsch'] },
  { name: 'unbekannte Option eines Befehls', args: ['board', 'list', '--quatsch'] },
  { name: 'Option ohne Wert', args: ['board', 'set', 'ARC-0001', '--title'] },
  { name: 'Hilfe über den Befehl help', args: ['help', 'board'] },
];

// Erzeugt das Szenario der Hilfetexte: liest die Befehlsliste rekursiv aus `<cli> --help` und schreibt je
// Befehl einen Schritt `--help`. Wird von Hand ausgeführt; das Ergebnis ist eingecheckt.
export class HelpScenarios {
  // `cli`: Befehl der CLI als Liste (z. B. ['node', '…/main.js']), `home`: Ordner für HARNESS_HOME und
  // CLAUDE_CONFIG_DIR der Aufrufe. `--help` führt keinen Befehl aus, trotzdem bekommt es nie den echten Ordner.
  constructor(cli, home = null) {
    this.cli = cli;
    this.home = home;
  }

  // Alle Befehlspfade in der Reihenfolge der Hilfe, beginnend mit dem Programm selbst (leerer Pfad)
  paths() {
    const out = [[]];
    const walk = (path) => {
      for (const name of this.subcommands(path)) {
        const child = [...path, name];
        out.push(child);
        walk(child);
      }
    };
    walk([]);
    return out;
  }

  // Namen der Unterbefehle eines Befehls, aus dem Abschnitt „Befehle“ seiner Hilfe
  subcommands(path) {
    const lines = this.help(path).split('\n');
    const start = lines.findIndex((line) => COMMAND_HEADING.test(line));
    if (start === -1) return [];
    const names = [];
    for (const line of lines.slice(start + 1)) {
      if (!line.trim()) break; // der Abschnitt endet mit einer Leerzeile (danach folgen Beispiele)
      const match = COMMAND_LINE.exec(line);
      // Erster Teil vor Argumenten (`show [options] <id>`) und vor einem Alias (`rm|remove`)
      if (match) names.push(match[1].split('|')[0]);
    }
    return names.filter((name) => name !== BUILTIN_HELP);
  }

  // Hilfetext eines Befehls; ohne Shell, damit Leerzeichen in Pfaden sicher sind
  help(path) {
    const [command, ...prefix] = this.cli;
    const args = [...prefix, ...path, '--help'];
    const env = { ...process.env, NO_COLOR: '1', LANG: 'C.UTF-8' };
    if (this.home) Object.assign(env, { HARNESS_HOME: this.home, CLAUDE_CONFIG_DIR: this.home });
    const result = spawnSync(command, args, { cwd: REPO_ROOT, encoding: 'utf8', windowsHide: true, env });
    if (result.error) throw new Error(`\`${[command, ...args].join(' ')}\` lässt sich nicht starten (${result.error.message}).`);
    if (result.status !== 0) {
      throw new Error(`\`${[command, ...args].join(' ')}\` endet mit ${result.status}:\n${result.stderr}`);
    }
    return CliStep.clean(result.stdout);
  }

  // Das vollständige Szenario: ein Schritt je Befehl, danach die Fehlerfälle
  scenario() {
    const steps = this.paths().map((path) => ({
      name: ['harness', ...path].join(' '),
      kind: 'cli',
      args: [...path, '--help'],
    }));
    for (const error of ERROR_STEPS) steps.push({ name: error.name, kind: 'cli', args: error.args });
    return {
      title: 'Hilfetexte aller Befehle und die Fehlermeldungen von commander',
      steps,
    };
  }
}

// Schreibt das Szenario und meldet, was gefunden wurde
function main(argv) {
  const index = argv.indexOf('--cli-cmd');
  if (index !== -1 && argv[index + 1] === undefined) throw new Error('--cli-cmd braucht einen Wert.');
  const value = index === -1 ? DEFAULT_CLI_CMD : argv[index + 1];
  const cli = ServerHandle.resolve(CliStep.command(value), REPO_ROOT);
  const home = mkdtempSync(join(tmpdir(), 'harness-contract-help-'));
  let scenario;
  try {
    scenario = new HelpScenarios(cli, home).scenario();
  } finally {
    rmSync(home, { recursive: true, force: true });
  }
  mkdirSync(join(CONTRACT_DIR, 'cli', 'help'), { recursive: true });
  // Reihenfolge der Felder wie geschrieben (nicht sortiert wie in den Golden-Dateien): so liest sich das Szenario
  writeFileSync(TARGET, `${JSON.stringify(scenario, null, 2)}\n`);
  const commands = scenario.steps.length - ERROR_STEPS.length;
  console.log(`${TARGET.slice(REPO_ROOT.length).split('\\').join('/')}: ${commands} Befehl(e) und ${ERROR_STEPS.length} Fehlerfall/-fälle.`);
  console.log('Golden-Datei aufzeichnen: node contract/run.mjs --suite cli --only cli/help --update');
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  try {
    main(process.argv.slice(2));
  } catch (error) {
    console.error(error instanceof Error ? error.message : error);
    process.exit(2);
  }
}
