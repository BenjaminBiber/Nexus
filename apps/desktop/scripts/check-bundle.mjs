// Prüft das gebaute macOS-Bundle: Programm, Ressourcen und Symbol im .app, dazu die Größe des .dmg.
// Sucht unter <target>/[<rust-ziel>/]release/bundle/{macos,dmg}/ und nimmt den jüngsten Treffer; als target
// kommen das gemeinsame target/ des Cargo-Workspace und ein einzeln gebautes apps/desktop/src-tauri/target/ infrage.
// Aufruf: node apps/desktop/scripts/check-bundle.mjs [rust-ziel]
import { accessSync, constants, existsSync, readFileSync, readdirSync, statSync } from 'node:fs';
import { basename, dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const HIER = dirname(fileURLToPath(import.meta.url));
const TARGETS = [resolve(HIER, '../../../target'), resolve(HIER, '../src-tauri/target')].filter((pfad) => existsSync(pfad));
const ZIEL = process.argv[2];

const fehler = [];

// Hält einen Mangel fest
function bemaengeln(text) {
  fehler.push(text);
}

// Alle Bundle-Ordner, die es gibt: mit Rust-Ziel genau dieses, sonst der Standardpfad und alle Ziel-Unterordner
function bundleOrdner() {
  const kandidaten = [];
  for (const target of TARGETS) {
    if (ZIEL) {
      kandidaten.push(join(target, ZIEL, 'release/bundle'));
      continue;
    }
    kandidaten.push(join(target, 'release/bundle'));
    for (const eintrag of readdirSync(target)) {
      if (eintrag !== 'release') kandidaten.push(join(target, eintrag, 'release/bundle'));
    }
  }
  return kandidaten.filter((pfad) => existsSync(pfad));
}

// Jüngste Datei bzw. jüngster Ordner mit passender Endung unter den Bundle-Ordnern
function juengstes(unterordner, endung) {
  const treffer = [];
  for (const basis of bundleOrdner()) {
    const ordner = join(basis, unterordner);
    if (!existsSync(ordner)) continue;
    for (const eintrag of readdirSync(ordner)) {
      if (!eintrag.endsWith(endung)) continue;
      const pfad = join(ordner, eintrag);
      treffer.push({ pfad, zeit: statSync(pfad).mtimeMs });
    }
  }
  treffer.sort((a, b) => b.zeit - a.zeit);
  return treffer[0]?.pfad;
}

// Prüft, dass es die Datei gibt, und bei exec zusätzlich, dass sie ausführbar ist
function pruefeDatei(wurzel, relativ, exec = false) {
  const pfad = join(wurzel, relativ);
  if (!existsSync(pfad)) return bemaengeln(`Fehlt im Bundle: ${relativ}`);
  if (exec) {
    try {
      accessSync(pfad, constants.X_OK);
    } catch {
      bemaengeln(`Nicht ausführbar: ${relativ}`);
    }
  }
}

// Name des Hauptprogramms aus CFBundleExecutable der Info.plist; ohne Angabe der Ordnername ohne „.app“
function hauptprogramm(wurzel) {
  const plist = join(wurzel, 'Contents/Info.plist');
  const name = existsSync(plist) ? readFileSync(plist, 'utf8').match(/<key>CFBundleExecutable<\/key>\s*<string>([^<]+)<\/string>/)?.[1] : undefined;
  return name ?? basename(wurzel, '.app');
}

const app = juengstes('macos', '.app');
if (!app) {
  console.error('Kein .app gefunden. Bitte zuerst "npm run tauri -w apps/desktop -- build --bundles app,dmg" ausführen.');
  process.exit(1);
}
console.log(`App: ${app}`);

pruefeDatei(app, 'Contents/Info.plist');
pruefeDatei(app, `Contents/MacOS/${hauptprogramm(app)}`, true);
pruefeDatei(app, 'Contents/Resources/web/index.html');
pruefeDatei(app, 'Contents/Resources/toast/logo-app.png');
pruefeDatei(app, 'Contents/Resources/plugin/nexus/.claude-plugin/plugin.json');
pruefeDatei(app, 'Contents/Resources/icon.icns');

const dmg = juengstes('dmg', '.dmg');
if (dmg) {
  const mb = (statSync(dmg).size / 1024 / 1024).toFixed(1);
  console.log(`Installer: ${dmg} (${mb} MB)`);
} else {
  bemaengeln('Kein .dmg gefunden.');
}

if (fehler.length > 0) {
  for (const text of fehler) console.error(`FEHLER: ${text}`);
  process.exit(1);
}
console.log('Bundle vollständig.');
