import { spawn } from 'node:child_process';
import { existsSync, readFileSync } from 'node:fs';
import { ServerHandle } from './ServerHandle.mjs';

// Reißleine, damit ein hängender Aufruf den Lauf nicht blockiert; kein Vertragsbestandteil
const TIMEOUT_MS = 120000;

// Schritt-Art `cli`: ruft die CLI als eigenen Prozess auf (Argumente, Arbeitsordner, Standardeingabe,
// Umgebung) und zeichnet Rückgabecode, stdout und stderr auf.
export class CliStep {
  // Eigene Felder der Art; der Lader meldet damit Tippfehler im Szenario
  get keys() {
    return ['args', 'cwd', 'stdin', 'env'];
  }

  // Prüft die Felder des Schritts: Argumente des Programms, Arbeitsordner, Standardeingabe und Umgebung
  parse(value, index, fail) {
    if (!Array.isArray(value.args) || value.args.length === 0 || value.args.some((arg) => typeof arg !== 'string')) {
      fail(`steps[${index}].args muss eine nicht leere Liste aus Texten sein.`);
    }
    if (value.cwd !== undefined && (typeof value.cwd !== 'string' || !value.cwd)) fail(`steps[${index}].cwd muss ein Text sein.`);
    if (value.stdin !== undefined && (typeof value.stdin !== 'string' && (value.stdin === null || typeof value.stdin !== 'object' || Array.isArray(value.stdin)))) {
      fail(`steps[${index}].stdin muss ein Objekt (als JSON) oder ein Text (roh) sein.`);
    }
    const env = value.env ?? {};
    if (env === null || typeof env !== 'object' || Array.isArray(env)) fail(`steps[${index}].env muss ein Objekt sein.`);
    for (const [name, entry] of Object.entries(env)) {
      if (typeof entry !== 'string') fail(`steps[${index}].env["${name}"] muss ein Text sein.`);
    }
    return { args: [...value.args], cwd: value.cwd, stdin: value.stdin, env };
  }

  // Führt den Schritt aus; `context.cli` ist das Programm als Liste, `context.home`/`port`/`claudeConfig`
  // die Umgebung des Szenarios und `context.envFile` die Datei der Fixture `claudeEnvFile`.
  // Ein Schritt mit `record: false` baut nur den Zustand auf und nimmt dafür `context.setupCli`.
  async run(step, context) {
    const args = context.variables.apply(step.args);
    const cwd = step.cwd === undefined ? context.home : context.variables.apply(step.cwd);
    const stdin = step.stdin === undefined ? null : context.variables.apply(step.stdin);
    const declared = context.variables.apply(step.env ?? {});
    const [command, ...prefix] = step.record === false ? context.setupCli ?? context.cli : context.cli;
    const result = await CliStep.execute(command, [...prefix, ...args], {
      cwd,
      env: CliStep.env(context, declared),
      input: stdin === null ? null : CliStep.input(stdin),
    });
    const request = { args, cwd };
    if (stdin !== null) request.stdin = stdin;
    // Nur die im Szenario genannten Variablen werden aufgezeichnet; die des Rahmens stehen im README
    if (Object.keys(declared).length) request.env = declared;
    const response = { code: result.code, ...CliStep.stdout(result.stdout), stderr: CliStep.clean(result.stderr) };
    if (context.envFile) response.envFile = CliStep.envFileContent(context.envFile);
    return { request, response };
  }

  // Zerlegt `--cli-cmd` in Programm und Argumente: ein JSON-Array wird gelesen, sonst wird am Leerzeichen
  // getrennt (Anführungszeichen halten zusammen). Das Array ist der sichere Weg für Pfade mit Leerzeichen.
  static command(value) {
    const text = value.trim();
    if (text.startsWith('[')) {
      let parsed;
      try {
        parsed = JSON.parse(text);
      } catch (error) {
        throw new Error(`--cli-cmd ist kein gültiges JSON-Array (${error.message}).`);
      }
      if (!Array.isArray(parsed) || !parsed.length || parsed.some((part) => typeof part !== 'string')) {
        throw new Error('--cli-cmd als JSON braucht eine nicht leere Liste aus Texten, z. B. \'["node","dist/main.js"]\'.');
      }
      return parsed;
    }
    const parts = ServerHandle.split(text);
    if (!parts.length) throw new Error('--cli-cmd ist leer.');
    return parts;
  }

  // Umgebung des Aufrufs: die geerbte ohne Session- und Ticket-Zuordnung, dazu Home, Port und eine leere
  // Claude-Konfiguration, feste Ausgabe ohne Farben – zuletzt die Variablen des Schritts
  static env(context, declared) {
    const env = {
      ...process.env,
      HARNESS_HOME: context.home,
      HARNESS_PORT: String(context.port),
      CLAUDE_CONFIG_DIR: context.claudeConfig,
      NO_COLOR: '1',
      LANG: 'C.UTF-8',
    };
    // Eine geerbte Zuordnung oder Umgebungsdatei gilt nicht für den Vertrag
    delete env.HARNESS_SESSION;
    delete env.HARNESS_TICKET;
    delete env.CLAUDE_ENV_FILE;
    if (context.envFile) env.CLAUDE_ENV_FILE = context.envFile;
    return { ...env, ...declared };
  }

  // Standardeingabe: ein Objekt geht als JSON hinein, ein Text roh
  static input(stdin) {
    return typeof stdin === 'string' ? stdin : JSON.stringify(stdin);
  }

  // stdout als `json`, wenn es ein JSON-Objekt oder eine Liste ist, sonst als `stdout`
  static stdout(text) {
    const out = CliStep.clean(text);
    if (out === '') return { stdout: '' };
    try {
      const json = JSON.parse(out);
      if (json !== null && typeof json === 'object') return { json };
    } catch {
      // kein JSON – bleibt Text
    }
    return { stdout: out };
  }

  // Zeilenenden vereinheitlichen und abschließende Leerzeilen weglassen, damit die Golden-Datei
  // auf allen Plattformen dieselbe ist
  static clean(text) {
    return text.replace(/\r\n/g, '\n').replace(/\n+$/, '');
  }

  // Inhalt der Umgebungsdatei nach dem Schritt; sie fehlt, solange kein Hook etwas geschrieben hat.
  // Der abschließende Zeilenumbruch bleibt: Er gehört zum Format der Datei.
  static envFileContent(file) {
    if (!existsSync(file)) return '';
    return readFileSync(file, 'utf8').replace(/\r\n/g, '\n');
  }

  // Startet das Programm ohne Shell (Leerzeichen in Pfaden bleiben sicher) und wartet auf sein Ende
  static execute(command, args, options) {
    return new Promise((done, fail) => {
      const child = spawn(command, args, {
        cwd: options.cwd,
        env: options.env,
        stdio: ['pipe', 'pipe', 'pipe'],
        windowsHide: true,
      });
      const out = [];
      const err = [];
      child.stdout.on('data', (chunk) => out.push(chunk));
      child.stderr.on('data', (chunk) => err.push(chunk));
      let expired = false;
      const timer = setTimeout(() => {
        expired = true;
        child.kill('SIGKILL');
      }, TIMEOUT_MS);
      child.on('error', (error) => {
        clearTimeout(timer);
        fail(new Error(`\`${command}\` lässt sich nicht starten (${error.message}).`));
      });
      child.on('close', (code, signal) => {
        clearTimeout(timer);
        const text = (chunks) => Buffer.concat(chunks).toString('utf8');
        if (expired) {
          fail(new Error(`\`${[command, ...args].join(' ')}\` lief länger als ${TIMEOUT_MS} ms.\n${text(err)}`));
          return;
        }
        if (code === null) {
          fail(new Error(`\`${[command, ...args].join(' ')}\` wurde durch ${signal} beendet.\n${text(err)}`));
          return;
        }
        done({ code, stdout: text(out), stderr: text(err) });
      });
      // Ein Programm, das die Eingabe nicht liest, lässt den Kanal zerbrechen; das ist kein Fehler des Schritts
      child.stdin.on('error', () => undefined);
      child.stdin.end(options.input ?? undefined);
    });
  }
}
