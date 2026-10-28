import { readFileSync, readdirSync, statSync } from 'node:fs';
import { join, relative, sep } from 'node:path';
import { stepKinds } from './StepKinds.mjs';

const SCENARIO_KEYS = new Set(['title', 'platforms', 'fixtures', 'steps']);
const FIXTURE_KEYS = new Set(['repos', 'boardConfig', 'homeFiles']);
const REPO_KEYS = new Set(['name', 'git', 'harnessInit', 'files', 'branches']);
const BRANCH_KEYS = new Set(['name', 'message', 'files']);
// Felder, die jede Schritt-Art hat; die Art nennt ihre eigenen in `keys`
const STEP_KEYS = new Set(['name', 'kind', 'save', 'compare', 'ignore', 'record']);
const PLATFORMS = new Set(['darwin', 'linux', 'win32']);
const METHODS = new Set(['GET', 'HEAD', 'POST', 'PUT', 'PATCH', 'DELETE']);
const COMPARE = new Set(['full', 'status', 'shape']);
const BOARD_CONFIGS = new Set(['sqlite']);
const REPO_NAME = /^[A-Z][A-Z0-9_]*$/;
// Branch-Name eines Fixture-Repos: wie bei Git, aber ohne Sonderfälle – Abschnitte aus Buchstaben, Zahlen, . _ -
const BRANCH_NAME = /^[A-Za-z0-9][A-Za-z0-9._-]*(?:\/[A-Za-z0-9][A-Za-z0-9._-]*)*$/;
// Pfad einer Datei im Home: relativ, mit `/` getrennt, Abschnitte ohne Schrägstriche und Rückwärtsschritte
const HOME_FILE = /^[^/\\]+(?:\/[^/\\]+)*$/;

// Liest die Szenario-Dateien eines Verzeichnisbaums und prüft ihre Felder.
export class ScenarioLoader {
  // `root`: Ordner, auf den sich die Namen der Szenarien beziehen (`contract/`)
  constructor(root) {
    this.root = root;
  }

  // Alle Szenarien unter `dir`, nach Namen sortiert
  load(dir) {
    return this.files(dir)
      .map((file) => this.parse(file))
      .sort((a, b) => a.name.localeCompare(b.name));
  }

  // Alle `*.scenario.json` unterhalb von `dir`
  files(dir) {
    if (!this.isDirectory(dir)) return [];
    const out = [];
    for (const entry of readdirSync(dir, { withFileTypes: true })) {
      const path = join(dir, entry.name);
      if (entry.isDirectory()) out.push(...this.files(path));
      else if (entry.name.endsWith('.scenario.json')) out.push(path);
    }
    return out;
  }

  // Liest und prüft eine Datei; liefert das Szenario mit Namen und Pfad der Golden-Datei
  parse(file) {
    const name = relative(this.root, file).split(sep).join('/').replace(/\.scenario\.json$/, '');
    const fail = (message) => {
      throw new Error(`${name}.scenario.json: ${message}`);
    };
    let raw;
    try {
      raw = JSON.parse(readFileSync(file, 'utf8'));
    } catch (error) {
      fail(`kein gültiges JSON (${error.message}).`);
    }
    if (raw === null || typeof raw !== 'object' || Array.isArray(raw)) fail('die Datei muss ein JSON-Objekt enthalten.');
    this.unknown(raw, SCENARIO_KEYS, '', fail);
    if (typeof raw.title !== 'string' || !raw.title.trim()) fail('title fehlt oder ist leer.');
    const platforms = this.platforms(raw.platforms, fail);
    const fixtures = this.fixtures(raw.fixtures, fail);
    const steps = this.steps(raw.steps, fail);
    return {
      name,
      file,
      golden: file.replace(/\.scenario\.json$/, '.golden.json'),
      title: raw.title,
      platforms,
      fixtures,
      steps,
    };
  }

  // Läuft das Szenario auf diesem Betriebssystem?
  static runsHere(scenario, platform = process.platform) {
    return scenario.platforms.length === 0 || scenario.platforms.includes(platform);
  }

  // Meldet Felder, die es im Format nicht gibt (Tippfehler fallen sofort auf)
  unknown(value, allowed, where, fail) {
    const extra = Object.keys(value).filter((key) => !allowed.has(key));
    if (extra.length) fail(`unbekannte Felder${where}: ${extra.join(', ')}.`);
  }

  // Betriebssysteme, auf denen das Szenario läuft (leer = alle)
  platforms(value, fail) {
    if (value === undefined) return [];
    if (!Array.isArray(value) || value.some((entry) => !PLATFORMS.has(entry))) {
      fail(`platforms muss eine Liste aus ${[...PLATFORMS].join(', ')} sein.`);
    }
    return [...value];
  }

  // Vorbereitungen vor den Schritten: Repos, Dateien im Home und Board-Konfiguration
  fixtures(value, fail) {
    if (value === undefined) return { repos: [], boardConfig: null, homeFiles: {} };
    if (value === null || typeof value !== 'object' || Array.isArray(value)) fail('fixtures muss ein Objekt sein.');
    this.unknown(value, FIXTURE_KEYS, ' in fixtures', fail);
    if (value.boardConfig !== undefined && !BOARD_CONFIGS.has(value.boardConfig)) {
      fail(`fixtures.boardConfig erlaubt nur ${[...BOARD_CONFIGS].join(', ')}.`);
    }
    if (value.repos !== undefined && !Array.isArray(value.repos)) fail('fixtures.repos muss eine Liste sein.');
    const repos = (value.repos ?? []).map((repo, index) => this.repo(repo, index, fail));
    const names = new Set();
    for (const repo of repos) {
      if (names.has(repo.name)) fail(`fixtures.repos enthält den Namen ${repo.name} zweimal.`);
      names.add(repo.name);
    }
    return { repos, boardConfig: value.boardConfig ?? null, homeFiles: this.homeFiles(value.homeFiles, fail) };
  }

  // Dateien, die vor dem Start des Servers im HARNESS_HOME liegen (Zustand, Läufe, Protokolle des Dispatchers)
  homeFiles(value, fail) {
    if (value === undefined) return {};
    if (value === null || typeof value !== 'object' || Array.isArray(value)) fail('fixtures.homeFiles muss ein Objekt sein.');
    for (const [path, content] of Object.entries(value)) {
      const where = `fixtures.homeFiles["${path}"]`;
      if (!HOME_FILE.test(path) || path.split('/').includes('..')) {
        fail(`${where}: der Pfad muss relativ zum Home sein, mit / getrennt und ohne „..“ (z. B. dispatch/state.json).`);
      }
      if (content === null || (typeof content !== 'string' && typeof content !== 'object')) {
        fail(`${where} muss ein Text oder ein JSON-Wert (Objekt oder Liste) sein.`);
      }
    }
    return { ...value };
  }

  // Ein Repo der Fixtures
  repo(value, index, fail) {
    const where = ` in fixtures.repos[${index}]`;
    if (value === null || typeof value !== 'object' || Array.isArray(value)) fail(`fixtures.repos[${index}] muss ein Objekt sein.`);
    this.unknown(value, REPO_KEYS, where, fail);
    if (typeof value.name !== 'string' || !REPO_NAME.test(value.name)) {
      fail(`fixtures.repos[${index}].name muss ein Name in Großbuchstaben sein (z. B. REPO1).`);
    }
    for (const flag of ['git', 'harnessInit']) {
      if (value[flag] !== undefined && typeof value[flag] !== 'boolean') fail(`fixtures.repos[${index}].${flag} muss true oder false sein.`);
    }
    const files = this.repoFiles(value.files, `fixtures.repos[${index}].files`, fail);
    // Ohne Git kein eingerichteter Harness und keine Branches: beides braucht ein Git-Repo
    const git = value.git ?? true;
    const harnessInit = value.harnessInit ?? false;
    if (harnessInit && !git) fail(`fixtures.repos[${index}]: harnessInit braucht git: true.`);
    if (value.branches !== undefined && !Array.isArray(value.branches)) fail(`fixtures.repos[${index}].branches muss eine Liste sein.`);
    const branches = (value.branches ?? []).map((branch, at) => this.branch(branch, index, at, fail));
    if (branches.length && !git) fail(`fixtures.repos[${index}]: branches braucht git: true.`);
    const names = new Set();
    for (const branch of branches) {
      if (names.has(branch.name)) fail(`fixtures.repos[${index}].branches enthält den Namen ${branch.name} zweimal.`);
      names.add(branch.name);
    }
    return { name: value.name, git, harnessInit, files, branches };
  }

  // Ein Arbeitsbranch des Repos: Name, Dateien des Commits und seine Nachricht
  branch(value, index, at, fail) {
    const where = `fixtures.repos[${index}].branches[${at}]`;
    if (value === null || typeof value !== 'object' || Array.isArray(value)) fail(`${where} muss ein Objekt sein.`);
    this.unknown(value, BRANCH_KEYS, ` in ${where}`, fail);
    if (typeof value.name !== 'string' || !BRANCH_NAME.test(value.name)) {
      fail(`${where}.name muss ein Branch-Name sein (z. B. harness/BEN-0001).`);
    }
    // Der Hauptbranch entsteht schon mit dem Repo; das Fixture wechselt nach jedem Branch dorthin zurück
    if (value.name === 'main') fail(`${where}.name darf nicht main sein – das ist der Hauptbranch des Repos.`);
    if (value.message !== undefined && (typeof value.message !== 'string' || !value.message.trim())) {
      fail(`${where}.message muss eine Commit-Nachricht als Text sein.`);
    }
    const files = this.repoFiles(value.files, `${where}.files`, fail);
    return { name: value.name, message: value.message ?? `Arbeit auf ${value.name}`, files };
  }

  // Dateien eines Repos oder eines Branches: Pfad → Text
  repoFiles(value, where, fail) {
    if (value === undefined) return {};
    if (value === null || typeof value !== 'object' || Array.isArray(value)) fail(`${where} muss ein Objekt sein.`);
    for (const [path, content] of Object.entries(value)) {
      if (typeof content !== 'string') fail(`${where}["${path}"] muss ein Text sein.`);
    }
    return { ...value };
  }

  // Die Schritte des Szenarios
  steps(value, fail) {
    if (!Array.isArray(value) || value.length === 0) fail('steps muss eine nicht leere Liste sein.');
    const names = new Set();
    return value.map((step, index) => {
      const parsed = this.step(step, index, fail);
      if (names.has(parsed.name)) fail(`steps enthält den Namen „${parsed.name}“ zweimal.`);
      names.add(parsed.name);
      return parsed;
    });
  }

  // Ein Schritt; `kind` entscheidet, welche Felder gelten (Standard `http`)
  step(value, index, fail) {
    const where = ` in steps[${index}]`;
    if (value === null || typeof value !== 'object' || Array.isArray(value)) fail(`steps[${index}] muss ein Objekt sein.`);
    if (typeof value.name !== 'string' || !value.name.trim()) fail(`steps[${index}].name fehlt oder ist leer.`);
    const kind = value.kind ?? 'http';
    if (typeof kind !== 'string' || !stepKinds[kind]) {
      fail(`steps[${index}].kind muss eine bekannte Art sein (${Object.keys(stepKinds).sort().join(', ')}).`);
    }
    this.unknown(value, new Set([...STEP_KEYS, ...stepKinds[kind].keys]), where, fail);
    if (value.compare !== undefined && !COMPARE.has(value.compare)) {
      fail(`steps[${index}].compare erlaubt nur ${[...COMPARE].join(', ')}.`);
    }
    if (value.record !== undefined && typeof value.record !== 'boolean') fail(`steps[${index}].record muss true oder false sein.`);
    if (value.ignore !== undefined && (!Array.isArray(value.ignore) || value.ignore.some((entry) => typeof entry !== 'string'))) {
      fail(`steps[${index}].ignore muss eine Liste aus Texten sein.`);
    }
    const save = value.save ?? {};
    if (save === null || typeof save !== 'object' || Array.isArray(save)) fail(`steps[${index}].save muss ein Objekt sein.`);
    for (const [name, path] of Object.entries(save)) {
      if (typeof path !== 'string' || !/^(json|headers)\./.test(path)) {
        fail(`steps[${index}].save["${name}"] muss mit json. oder headers. beginnen.`);
      }
    }
    const step = {
      name: value.name,
      kind,
      compare: value.compare ?? 'full',
      ignore: value.ignore ?? [],
      record: value.record ?? true,
      save,
    };
    if (kind === 'http') return { ...step, ...this.httpStep(value, index, fail) };
    return { ...step, raw: value };
  }

  // Felder eines HTTP-Schritts
  httpStep(value, index, fail) {
    if (typeof value.method !== 'string' || !METHODS.has(value.method)) {
      fail(`steps[${index}].method muss eine von ${[...METHODS].join(', ')} sein.`);
    }
    if (typeof value.path !== 'string' || !value.path.startsWith('/')) fail(`steps[${index}].path muss mit / beginnen.`);
    const headers = value.headers ?? {};
    if (headers === null || typeof headers !== 'object' || Array.isArray(headers)) fail(`steps[${index}].headers muss ein Objekt sein.`);
    for (const [name, header] of Object.entries(headers)) {
      if (header !== null && typeof header !== 'string') fail(`steps[${index}].headers["${name}"] muss ein Text oder null sein.`);
    }
    return { method: value.method, path: value.path, body: value.body, headers };
  }

  // Gibt es den Ordner?
  isDirectory(dir) {
    return statSync(dir, { throwIfNoEntry: false })?.isDirectory() ?? false;
  }
}
