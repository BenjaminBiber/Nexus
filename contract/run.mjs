#!/usr/bin/env node
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { CliStep } from './lib/CliStep.mjs';
import { Coverage } from './lib/Coverage.mjs';
import { GoldenStore } from './lib/GoldenStore.mjs';
import { ScenarioLoader } from './lib/ScenarioLoader.mjs';
import { ScenarioRunner } from './lib/ScenarioRunner.mjs';
import { ServerHandle } from './lib/ServerHandle.mjs';

const CONTRACT_DIR = fileURLToPath(new URL('.', import.meta.url));
const REPO_ROOT = fileURLToPath(new URL('..', import.meta.url));
const DEFAULT_SERVER_CMD = 'node contract/servers/ts-server.mjs --port {port}';
const DEFAULT_CLI_CMD = 'node packages/cli/dist/main.js';

const HELP = `Vertragstest von HTTP-API, CLI und Hooks.

  node contract/run.mjs [Optionen]

  --update              Golden-Dateien neu aufzeichnen (nur geänderte werden geschrieben)
  --only <teilstring>   nur Szenarien, deren Pfad den Teilstring enthält
  --suite <name>        Ordner unter contract/ (Standard: api)
  --server-cmd "<cmd>"  eigener Startbefehl, {port} wird ersetzt
  --base <url>          fremder, laufender Server (braucht --home und genau ein Szenario)
  --home <ordner>       HARNESS_HOME des fremden Servers
  --cli-cmd "<cmd>"     Befehl der CLI für harnessInit und die cli-Schritte, auch als JSON-Array
                        (Standard: ${DEFAULT_CLI_CMD})
  --coverage            Routen aus contract/api/routes.json mit abgedeckt/offen auflisten
  --strict              mit --coverage: Exit 1, wenn eine Route offen ist
  --verbose             Ausgabe des Servers mitschreiben
  --help                diese Hilfe
`;

// Einstieg des Vertragstests: Argumente lesen, Szenarien abspielen, vergleichen oder aufzeichnen.
export class ContractRun {
  constructor(options) {
    this.options = options;
    this.golden = new GoldenStore();
    this.loader = new ScenarioLoader(CONTRACT_DIR);
  }

  // Liest die Argumente; wirft bei unbekannten Optionen
  static parse(argv) {
    const options = {
      update: false, only: null, suite: 'api', serverCmd: DEFAULT_SERVER_CMD, base: null, home: null,
      cliCmd: DEFAULT_CLI_CMD, coverage: false, strict: false, verbose: false, help: false,
    };
    const values = { '--only': 'only', '--suite': 'suite', '--server-cmd': 'serverCmd', '--base': 'base', '--home': 'home', '--cli-cmd': 'cliCmd' };
    const flags = { '--update': 'update', '--coverage': 'coverage', '--strict': 'strict', '--verbose': 'verbose', '--help': 'help', '-h': 'help' };
    for (let i = 0; i < argv.length; i += 1) {
      const arg = argv[i];
      if (flags[arg]) options[flags[arg]] = true;
      else if (values[arg]) {
        const value = argv[i + 1];
        if (value === undefined || value.startsWith('--')) throw new Error(`${arg} braucht einen Wert.`);
        options[values[arg]] = value;
        i += 1;
      } else throw new Error(`Unbekannte Option ${arg}. \`node contract/run.mjs --help\` zeigt alle.`);
    }
    return options;
  }

  // Führt den gewünschten Modus aus und liefert den Exit-Code
  async main() {
    if (this.options.help) {
      process.stdout.write(HELP);
      return 0;
    }
    const scenarios = this.scenarios();
    if (this.options.coverage) return this.reportCoverage(scenarios);
    if (!scenarios.length) {
      console.error(`Keine Szenarien gefunden (Suite ${this.options.suite}${this.options.only ? `, --only ${this.options.only}` : ''}).`);
      return 1;
    }
    return this.runAll(scenarios);
  }

  // Szenarien der Suite, gefiltert nach --only und Betriebssystem
  scenarios() {
    const all = this.loader.load(join(CONTRACT_DIR, this.options.suite));
    const only = this.options.only;
    return all.filter((scenario) => (!only || scenario.name.includes(only)) && ScenarioLoader.runsHere(scenario));
  }

  // Spielt alle Szenarien ab und vergleicht oder schreibt die Golden-Dateien
  async runAll(scenarios) {
    if (this.options.base !== null) this.checkExternal(scenarios);
    const runner = new ScenarioRunner({
      cwd: REPO_ROOT,
      serverCmd: this.options.serverCmd,
      cli: ServerHandle.resolve(CliStep.command(this.options.cliCmd), REPO_ROOT),
      base: this.options.base,
      homeDir: this.options.home,
      verbose: this.options.verbose,
    });
    const started = Date.now();
    let failed = 0;
    let steps = 0;
    console.log(`Vertragstest: ${scenarios.length} Szenario(s) in contract/${this.options.suite}${this.options.update ? ' (--update)' : ''}`);
    for (const scenario of scenarios) {
      const at = Date.now();
      let entries;
      try {
        entries = await runner.run(scenario);
      } catch (error) {
        failed += 1;
        console.log(`  FEHLER ${scenario.name}`);
        console.log(this.indent(error.message));
        continue;
      }
      steps += entries.length;
      const findings = this.settle(scenario, entries);
      const seconds = ((Date.now() - at) / 1000).toFixed(1);
      if (findings.length) {
        failed += 1;
        console.log(`  FEHLER ${scenario.name} (${seconds} s)`);
        for (const finding of findings) console.log(this.indent(finding));
      } else {
        console.log(`  ok     ${scenario.name} – ${entries.length} Schritt(e), ${seconds} s${this.written ? ', Golden-Datei geschrieben' : ''}`);
      }
    }
    const seconds = ((Date.now() - started) / 1000).toFixed(1);
    console.log(`\n${scenarios.length} Szenario(s), ${steps} Schritt(e), ${scenarios.length - failed} grün, ${failed} rot – ${seconds} s`);
    return failed ? 1 : 0;
  }

  // Schreibt die Golden-Datei (--update) oder vergleicht sie; liefert die Abweichungen
  settle(scenario, entries) {
    this.written = false;
    if (this.options.update) {
      this.written = this.golden.write(scenario.golden, entries);
      return [];
    }
    let expected;
    try {
      expected = this.golden.read(scenario.golden);
    } catch (error) {
      return [error.message];
    }
    if (expected === null) {
      return [`Es gibt noch keine Golden-Datei (${this.relative(scenario.golden)}). Mit \`node contract/run.mjs --update\` aufzeichnen.`];
    }
    return GoldenStore.compare(expected, entries);
  }

  // Ein fremder Server behält seinen Zustand: dann ist genau ein Szenario erlaubt
  checkExternal(scenarios) {
    if (!this.options.home) throw new Error('--base braucht --home <ordner> (das HARNESS_HOME des fremden Servers).');
    if (scenarios.length !== 1) {
      const names = scenarios.map((scenario) => scenario.name).join(', ') || 'keines';
      throw new Error(`--base kann den Zustand des Servers nicht zurücksetzen und erlaubt deshalb genau ein Szenario; --only passt auf: ${names}.`);
    }
  }

  // Listet alle Routen mit abgedeckt/offen (ohne Server, aus den Golden-Dateien)
  reportCoverage(scenarios) {
    const routes = JSON.parse(readFileSync(join(CONTRACT_DIR, 'api', 'routes.json'), 'utf8'));
    const coverage = new Coverage(routes);
    const missing = [];
    for (const scenario of scenarios) {
      const entries = this.golden.read(scenario.golden);
      if (entries === null) {
        missing.push(scenario.name);
        continue;
      }
      for (const entry of entries) {
        if (entry.request?.method && entry.request?.path) coverage.add(entry.request.method, entry.request.path);
      }
    }
    console.log(coverage.report());
    if (missing.length) console.log(`\nOhne Golden-Datei (erst \`node contract/run.mjs --update\`): ${missing.join(', ')}`);
    if (this.options.strict && coverage.open.length) {
      console.error(`\n${coverage.open.length} Route(n) sind offen (--strict).`);
      return 1;
    }
    return 0;
  }

  // Pfad relativ zum Projekt, mit Schrägstrichen
  relative(file) {
    return file.slice(REPO_ROOT.length).split('\\').join('/');
  }

  // Rückt einen mehrzeiligen Text für die Ausgabe ein
  indent(text) {
    return text
      .split('\n')
      .map((line) => `      ${line}`)
      .join('\n');
  }
}

const options = (() => {
  try {
    return ContractRun.parse(process.argv.slice(2));
  } catch (error) {
    console.error(error.message);
    process.exit(2);
  }
})();

new ContractRun(options).main().then(
  (code) => process.exit(code),
  (error) => {
    console.error(error instanceof Error ? error.message : error);
    process.exit(2);
  },
);
