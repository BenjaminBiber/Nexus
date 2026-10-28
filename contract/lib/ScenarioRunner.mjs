import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { GIT_IDENTITY_ENV } from './GitIdentity.mjs';
import { HomeFiles } from './HomeFiles.mjs';
import { Normalizer } from './Normalizer.mjs';
import { RepoFixture } from './RepoFixture.mjs';
import { ServerHandle } from './ServerHandle.mjs';
import { stepKinds } from './StepKinds.mjs';
import { Variables } from './Variables.mjs';

// Feste GUID des Vertrags-Benutzers in der Board-Konfiguration
const CONTRACT_USER_ID = '11111111-1111-1111-1111-111111111111';

// Führt ein Szenario aus: frisches Home, Fixture-Repos, Server, Schritte, Normalisierung.
export class ScenarioRunner {
  // `serverCmd`: Startbefehl mit `{port}`, `base`/`homeDir`: fremder Server, `cli`: Befehl der CLI als Liste
  constructor(options) {
    this.cwd = options.cwd;
    this.serverCmd = options.serverCmd;
    this.cli = options.cli;
    this.base = options.base ?? null;
    this.homeDir = options.homeDir ?? null;
    this.verbose = options.verbose ?? false;
    this.log = options.log ?? (() => {});
  }

  // Führt das Szenario aus und liefert die Einträge für die Golden-Datei
  async run(scenario) {
    const own = this.base === null;
    const home = this.homeDir ?? mkdtempSync(join(tmpdir(), 'harness-contract-home-'));
    const claudeConfig = mkdtempSync(join(tmpdir(), 'harness-contract-claude-'));
    const port = own ? await ServerHandle.freePort() : ScenarioRunner.portOf(this.base);
    const fixture = new RepoFixture({ home, claudeConfig, cli: this.cli });
    // Attrappen fremder Dienste der Schritt-Art `httpStub`, nach Namen; sie leben nur in diesem Szenario
    const stubs = new Map();
    let server = null;
    try {
      const repos = fixture.create(scenario.fixtures.repos);
      const variables = new Variables({ ...repos, HOME: home, PORT: String(port), NOW: new Date().toISOString() });
      // Dateien im Home liegen vor dem Start, damit der Server sie von der ersten Anfrage an sieht
      new HomeFiles(home).write(scenario.fixtures.homeFiles, variables);
      server = own
        ? await ServerHandle.start({ command: this.serverCmd, port, cwd: this.cwd, env: this.env(home, claudeConfig, port), verbose: this.verbose })
        : ServerHandle.external(this.base);
      if (!own) await server.waitForHealth();
      // Nur Werte aus `save` werden zu `<var:…>`; die Pfade der Fixtures haben eigene Platzhalter
      const saved = {};
      const normalizer = new Normalizer({
        paths: { ...repos, HOME: home, CLAUDE: claudeConfig, REPOS: fixture.base, TMP: tmpdir() },
        port,
        saved,
      });
      if (scenario.fixtures.boardConfig === 'sqlite') await this.writeBoardConfig(server.base, home);
      // `home` und `reposDir` brauchen die Schritt-Arten, die Dateien schreiben bzw. committen (homeFile, commit)
      return await this.steps(scenario, { base: server.base, variables, normalizer, saved, stubs, home, reposDir: fixture.base });
    } finally {
      if (server) await server.stop();
      for (const stub of stubs.values()) await stub.close();
      fixture.dispose();
      rmSync(claudeConfig, { recursive: true, force: true });
      // Ein von außen übergebenes Home gehört dem Aufrufer
      if (own) rmSync(home, { recursive: true, force: true });
    }
  }

  // Port einer Adresse (`--base`); ohne Angabe der Standardport des Schemas
  static portOf(base) {
    const url = new URL(base);
    if (url.port) return Number(url.port);
    return url.protocol === 'https:' ? 443 : 80;
  }

  // Umgebung des Servers; CONTRACT_HEARTBEAT_MS und CONTRACT_POLL_MS erbt er aus dem eigenen Prozess.
  // GIT_IDENTITY_ENV gibt seinen Commits (Merge eines Ticket-Branches) eine feste Identität und ein festes Datum
  env(home, claudeConfig, port) {
    const env = { ...process.env, ...GIT_IDENTITY_ENV, HARNESS_HOME: home, HARNESS_PORT: String(port), CLAUDE_CONFIG_DIR: claudeConfig };
    // Eine geerbte Session- oder Ticket-Zuordnung gilt nicht für den Vertrag
    delete env.HARNESS_SESSION;
    delete env.HARNESS_TICKET;
    return env;
  }

  // Fixture `boardConfig: "sqlite"`: Board-Datenbank im Home anmelden (nicht aufgezeichnet)
  async writeBoardConfig(base, home) {
    const body = { provider: 'sqlite', dbFile: join(home, 'board.db'), userId: CONTRACT_USER_ID, userName: 'Vertrag' };
    const response = await fetch(`${base}/api/board-config`, {
      method: 'PUT',
      headers: { 'x-harness': '1', 'content-type': 'application/json' },
      body: JSON.stringify(body),
    });
    const text = await response.text();
    if (!response.ok) throw new Error(`Fixture boardConfig: PUT /api/board-config meldet HTTP ${response.status}: ${text}`);
  }

  // Führt alle Schritte aus und sammelt die aufgezeichneten Einträge
  async steps(scenario, context) {
    const entries = [];
    for (const step of scenario.steps) {
      const kind = stepKinds[step.kind];
      if (!kind) throw new Error(`Schritt „${step.name}“: unbekannte Art ${step.kind}.`);
      let result;
      try {
        result = await kind.run(step, context);
      } catch (error) {
        throw new Error(`Schritt „${step.name}“: ${error.message}`);
      }
      for (const [name, path] of Object.entries(step.save)) {
        const value = Variables.pick(result.response, path);
        context.variables.set(name, value);
        context.saved[name] = value;
      }
      if (!step.record) continue;
      entries.push({ name: step.name, ...ScenarioRunner.compared(step, context.normalizer.value(result)) });
    }
    return entries;
  }

  // Wendet `compare` und `ignore` auf den normalisierten Eintrag an
  static compared(step, record) {
    const out = { request: record.request, response: record.response };
    for (const path of step.ignore) Variables.drop(out.response, path);
    if (step.compare === 'status') out.response = { status: out.response.status };
    if (step.compare === 'shape') {
      if ('json' in out.response) out.response.json = ScenarioRunner.shape(out.response.json);
      if ('text' in out.response) out.response.text = '<string>';
    }
    return out;
  }

  // Nur Schlüssel und JSON-Typen; gleiche Formen in einer Liste werden zu einer zusammengefasst
  static shape(value) {
    if (Array.isArray(value)) {
      const shapes = [];
      for (const entry of value) {
        const shape = ScenarioRunner.shape(entry);
        if (!shapes.some((known) => JSON.stringify(known) === JSON.stringify(shape))) shapes.push(shape);
      }
      return shapes;
    }
    if (value !== null && typeof value === 'object') {
      const out = {};
      for (const [key, entry] of Object.entries(value)) out[key] = ScenarioRunner.shape(entry);
      return out;
    }
    return `<${value === null ? 'null' : typeof value}>`;
  }
}
