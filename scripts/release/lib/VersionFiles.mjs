// Findet alle Stellen, an denen die Version des Repos steht, liest sie und setzt sie. Der Wurzelordner ist ein
// Parameter, damit Tests auf einer Kopie im Temp-Ordner laufen und nie im echten Repo.
//
// Quelle der Version ist `[workspace.package] version` der Workspace-`Cargo.toml`. Gibt es die noch nicht, ist es
// die `[package] version` des einzigen Crates. Alle anderen Stellen müssen ihr gleichen.

import { existsSync, readFileSync, readdirSync, writeFileSync } from 'node:fs';
import { dirname, join, relative, sep } from 'node:path';
import { JsonText } from './JsonText.mjs';
import { parse } from './SemVer.mjs';
import { TomlText } from './TomlText.mjs';

// Plugin-Manifest und Marktplatz-Eintrag liegen an festen Stellen
const PLUGIN_MANIFEST = join('packages', 'plugin', 'manifest', '.claude-plugin', 'plugin.json');
const MARKETPLACE = join('.claude-plugin', 'marketplace.json');
// Name des Plugins, falls das Manifest fehlt
const DEFAULT_PLUGIN = 'ai-harness';
// npm-Workspaces, falls die Wurzel-`package.json` keine nennt
const DEFAULT_WORKSPACES = ['packages/*', 'apps/*'];
// Abhängigkeiten, in denen Geschwisterpakete exakt gepinnt sein können
const DEP_GROUPS = ['dependencies', 'devDependencies', 'peerDependencies', 'optionalDependencies'];
// Eine exakt gepinnte Version (`0.1.0`). `^0.1.0`, `~0.1.0`, `*` und `workspace:*` bleiben unberührt
const EXACT_PIN = /^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/;

// Liest eine Datei; `null`, wenn es sie nicht gibt
function read(file) {
  try {
    return readFileSync(file, 'utf8');
  } catch {
    return null;
  }
}

// Unterordner eines Ordners (ohne `node_modules`); leer, wenn es den Ordner nicht gibt
function subdirs(dir) {
  try {
    return readdirSync(dir, { withFileTypes: true })
      .filter((entry) => entry.isDirectory() && entry.name !== 'node_modules')
      .map((entry) => entry.name)
      .sort();
  } catch {
    return [];
  }
}

// Löst ein Muster mit `*` in vorhandene Ordner auf: `packages/*`, `apps/*/src-tauri`, `crates/core`
function expand(root, pattern) {
  let found = [root];
  for (const part of String(pattern).split('/').filter((piece) => piece && piece !== '.')) {
    found = part === '*'
      ? found.flatMap((dir) => subdirs(dir).map((name) => join(dir, name)))
      : found.map((dir) => join(dir, part)).filter((dir) => existsSync(dir));
  }
  return found;
}

// Alle Versionsstellen eines Repos: lesen, vergleichen, setzen.
export class VersionFiles {
  constructor(root) {
    this.root = root;
  }

  // Pfad einer Datei, wie er in Meldungen steht (immer mit `/`)
  label(file) {
    return relative(this.root, file).split(sep).join('/');
  }

  // Eine Versionsstelle. `version` ist `null`, wenn die Stelle fehlt und beim Setzen angelegt wird.
  place(file, where, version, format, apply) {
    return { file: this.label(file), path: file, where, version, format, apply };
  }

  // Die Version der Quelle
  current() {
    return this.collect().source.version;
  }

  // Vergleicht alle Stellen mit der Quelle
  check() {
    const found = this.collect();
    return { ...found, version: found.source.version, deviations: found.places.filter((entry) => entry.version !== found.source.version) };
  }

  // Setzt alle Stellen auf `version` und liefert die geänderten Dateien
  set(version) {
    // Erst prüfen, dann schreiben: ein Tippfehler soll nicht in zehn Dateien landen
    parse(version);
    const { places, removals, warnings } = this.collect();
    const byFile = new Map();
    for (const edit of [...places, ...removals]) {
      if (!byFile.has(edit.path)) byFile.set(edit.path, { file: edit.file, format: edit.format, edits: [] });
      byFile.get(edit.path).edits.push(edit);
    }
    const changed = [];
    for (const [path, entry] of byFile) {
      const before = readFileSync(path, 'utf8');
      const doc = entry.format === 'json' ? new JsonText(before) : new TomlText(before);
      for (const edit of entry.edits) edit.apply(doc, version);
      // Unverändert heißt: nicht anfassen – so bleibt der Zeitstempel der Datei stehen
      if (doc.text === before) continue;
      writeFileSync(path, doc.text);
      changed.push(entry.file);
    }
    return { changed, warnings };
  }

  // Sucht alle Stellen zusammen: { source, places, removals, warnings }. `places` enthält die Quelle.
  collect() {
    const places = [];
    const removals = [];
    const warnings = [];
    const source = this.cargo(places, removals, warnings);
    const pluginName = this.plugin(places);
    this.marketplace(places, warnings, pluginName);
    this.npm(places, warnings);
    return { source, places, removals, warnings };
  }

  // Cargo: Quelle, Versionen der Crates, Einträge in Cargo.lock und eine Version, die nicht in tauri.conf.json gehört
  cargo(places, removals, warnings) {
    const manifests = new Map();
    const addManifest = (file) => {
      if (manifests.has(file)) return;
      const text = read(file);
      if (text !== null) manifests.set(file, new TomlText(text));
    };
    const rootFile = join(this.root, 'Cargo.toml');
    addManifest(rootFile);
    const rootToml = manifests.get(rootFile) ?? null;
    // Mitglieder des Workspace, so wie sie dort stehen
    for (const member of rootToml?.array('workspace', 'members') ?? []) {
      for (const dir of expand(this.root, member)) addManifest(join(dir, 'Cargo.toml'));
    }
    // Auch ohne Workspace-Datei: die Crates der Apps (apps/<name>/src-tauri)
    for (const dir of [...expand(this.root, 'apps/*'), ...expand(this.root, 'apps/*/src-tauri')]) {
      addManifest(join(dir, 'Cargo.toml'));
    }

    // Namen aller Crates – nur deren Einträge in Cargo.lock gehören uns
    const crates = new Set();
    for (const toml of manifests.values()) {
      const name = toml.string('package', 'name');
      if (name) crates.add(name);
    }

    // Die Quelle: `[workspace.package] version`, sonst die eigene Version des einzigen Crates
    let source = null;
    if (rootToml?.string('workspace.package', 'version') !== undefined) {
      source = this.place(rootFile, '[workspace.package] version', rootToml.string('workspace.package', 'version'), 'toml',
        (doc, version) => doc.setString('workspace.package', 'version', version));
      places.push(source);
    }
    const own = [];
    for (const [file, toml] of manifests) {
      // `version.workspace = true` zieht die Version des Workspace – diese Datei bleibt unverändert
      if (toml.raw('package', 'version.workspace') === 'true') continue;
      const version = toml.string('package', 'version');
      if (version === undefined) continue;
      own.push(this.place(file, '[package] version', version, 'toml',
        (doc, next) => doc.setString('package', 'version', next)));
    }
    if (source) places.push(...own);
    else if (own.length === 1) {
      source = own[0];
      places.push(source);
    } else if (own.length === 0) {
      throw new Error('Keine Cargo.toml mit einer Version gefunden – ohne Quelle lässt sich nichts setzen.');
    } else {
      throw new Error(`Ohne „[workspace.package] version“ ist die Quelle nicht eindeutig: ${own.map((entry) => entry.file).join(', ')}.`);
    }

    this.cargoLocks(places, manifests, crates);
    this.tauriConf(removals, warnings, manifests);
    return source;
  }

  // Cargo.lock: nur die Einträge der eigenen Crates, nicht die der Abhängigkeiten
  cargoLocks(places, manifests, crates) {
    const files = new Set([join(this.root, 'Cargo.lock'), ...[...manifests.keys()].map((file) => join(dirname(file), 'Cargo.lock'))]);
    for (const file of [...files].sort()) {
      const text = read(file);
      if (text === null) continue;
      const toml = new TomlText(text);
      for (const table of toml.tables('package')) {
        if (!crates.has(table.name)) continue;
        const version = toml.string('package', 'version', table);
        if (version === undefined) continue;
        const name = table.name;
        places.push(this.place(file, `[[package]] ${name} → version`, version, 'toml', (doc, next) => {
          // Die Stelle im frischen Text selbst suchen; die Zeilennummern von vorher gelten dort nicht
          const found = doc.tables('package').find((entry) => entry.name === name);
          if (found) doc.setString('package', 'version', next, found);
        }));
      }
    }
  }

  // tauri.conf.json darf keine Version tragen – Tauri liest sie aus dem Crate
  tauriConf(removals, warnings, manifests) {
    for (const dir of new Set([...manifests.keys()].map((file) => dirname(file)))) {
      const file = join(dir, 'tauri.conf.json');
      const text = read(file);
      if (text === null) continue;
      if (!new JsonText(text).has(['version'])) continue;
      removals.push(this.place(file, 'version', null, 'json', (doc) => doc.remove(['version'])));
      warnings.push(`${this.label(file)}: „version“ gehört nicht in tauri.conf.json (Tauri liest sie aus dem Crate) – wird entfernt.`);
    }
  }

  // Plugin-Manifest; liefert den Namen des Plugins für den Marktplatz-Eintrag
  plugin(places) {
    const file = join(this.root, PLUGIN_MANIFEST);
    const text = read(file);
    if (text === null) return DEFAULT_PLUGIN;
    const doc = new JsonText(text);
    if (doc.has(['version'])) {
      places.push(this.place(file, 'version', doc.value(['version']), 'json',
        (target, version) => target.setString(['version'], version)));
    }
    return typeof doc.data.name === 'string' ? doc.data.name : DEFAULT_PLUGIN;
  }

  // Marktplatz-Eintrag des Plugins; die Version wird angelegt, falls sie dort noch fehlt
  marketplace(places, warnings, pluginName) {
    const file = join(this.root, MARKETPLACE);
    const text = read(file);
    if (text === null) return;
    const doc = new JsonText(text);
    const plugins = Array.isArray(doc.data.plugins) ? doc.data.plugins : [];
    const index = plugins.findIndex((entry) => entry && entry.name === pluginName);
    if (index === -1) {
      warnings.push(`${this.label(file)}: kein Eintrag „${pluginName}“ unter „plugins“ – dort steht keine Version.`);
      return;
    }
    const where = `plugins[${pluginName}] → version`;
    if (doc.has(['plugins', index, 'version'])) {
      places.push(this.place(file, where, doc.value(['plugins', index, 'version']), 'json',
        (target, version) => target.setString(['plugins', index, 'version'], version)));
    } else {
      // Fehlt die Version, gilt sie als Abweichung und wird beim Setzen hinter den Namen eingefügt
      places.push(this.place(file, where, null, 'json',
        (target, version) => target.insertStringAfter(['plugins', index, 'name'], 'version', version)));
    }
  }

  // npm: Version je Workspace-Paket, dieselben Stellen in package-lock.json und exakt gepinnte Geschwister
  npm(places, warnings) {
    const rootFile = join(this.root, 'package.json');
    const rootText = read(rootFile);
    if (rootText === null) return;
    const rootJson = new JsonText(rootText);
    const workspaces = rootJson.data.workspaces;
    const patterns = Array.isArray(workspaces) ? workspaces
      : Array.isArray(workspaces?.packages) ? workspaces.packages
        : DEFAULT_WORKSPACES;
    const dirs = [...new Set(patterns.flatMap((pattern) => expand(this.root, pattern)))]
      .filter((dir) => existsSync(join(dir, 'package.json')))
      .sort();
    const packages = dirs.map((dir) => {
      const file = join(dir, 'package.json');
      return { file, dir: this.label(dir), json: new JsonText(read(file)) };
    });
    const names = new Set(packages.map((entry) => entry.json.data.name).filter((name) => typeof name === 'string'));

    for (const entry of packages) {
      // Ein Paket ohne `version` bekommt keine (etwa ein rein privates Paket)
      if (entry.json.has(['version'])) {
        places.push(this.place(entry.file, 'version', entry.json.value(['version']), 'json',
          (doc, version) => doc.setString(['version'], version)));
      }
      this.pins(places, entry.file, entry.json, [], names);
    }
    // Die Wurzel hat keine Version, kann aber Geschwister pinnen
    this.pins(places, rootFile, rootJson, [], names);

    const lockFile = join(this.root, 'package-lock.json');
    const lockText = read(lockFile);
    if (lockText === null) {
      warnings.push('package-lock.json fehlt – die Versionen der Pakete stehen dort nicht.');
      return;
    }
    const lock = new JsonText(lockText);
    for (const entry of [{ dir: '', json: null }, ...packages]) {
      const base = ['packages', entry.dir];
      if (!lock.has(base)) continue;
      // `packages[""]` ist die Wurzel und hat keine Version
      if (entry.dir !== '' && lock.has([...base, 'version'])) {
        places.push(this.place(lockFile, `packages["${entry.dir}"] → version`, lock.value([...base, 'version']), 'json',
          (doc, version) => doc.setString([...base, 'version'], version)));
      }
      this.pins(places, lockFile, lock, base, names);
    }
  }

  // Exakt gepinnte Abhängigkeiten auf Geschwisterpakete: ohne sie fände `npm ci` die alte Version nicht mehr
  pins(places, file, doc, base, names) {
    const prefix = base.length ? `packages["${base[1]}"] → ` : '';
    for (const group of DEP_GROUPS) {
      for (const name of names) {
        const path = [...base, group, name];
        if (!doc.has(path)) continue;
        const pinned = doc.value(path);
        if (typeof pinned !== 'string' || !EXACT_PIN.test(pinned)) continue;
        places.push(this.place(file, `${prefix}${group}["${name}"]`, pinned, 'json',
          (target, version) => target.setString(path, version)));
      }
    }
  }
}
