// Richtet Nexus auf diesem Rechner ein (einmal, wiederholbar zum Aktualisieren):
// baut alles und ruft `nexus install` aus dem gebauten Plugin – das richtet PATH, Regeln und das
// Plugin in Claude Code ein.
import { spawnSync } from 'node:child_process';
import { existsSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const MARKETPLACE = 'nexus-local';
const PLUGIN = `nexus@${MARKETPLACE}`;
const PLUGIN_DIR = join(ROOT, 'packages/plugin/dist/nexus');
const BUNDLED_BIN = join(PLUGIN_DIR, 'bin', process.platform === 'win32' ? 'nexus.exe' : 'nexus');
// `npm run setup -- --no-pull`: nur bauen und Anpassungen anwenden, keine neue Version holen
const NO_PULL = process.argv.includes('--no-pull');

// Setzt Leerzeichen enthaltende Teile in Anführungszeichen (Befehle laufen über die Shell, damit npm.cmd und claude.cmd gefunden werden)
function quote(arg) {
  return /\s/.test(arg) && !arg.startsWith('"') ? `"${arg}"` : arg;
}

// Führt einen Befehl im Repository (oder in `cwd`) aus; `show` gibt die Ausgabe direkt aus, sonst wird sie zurückgegeben.
// Auch der Befehl selbst wird gequotet (z. B. `C:\Program Files\nodejs\node.exe`).
function run(command, args, show = true, cwd = ROOT) {
  const line = [command, ...args].map(quote).join(' ');
  const result = spawnSync(line, { cwd, shell: true, encoding: 'utf8', stdio: show ? 'inherit' : 'pipe' });
  return { ok: result.status === 0, out: `${result.stdout ?? ''}${result.stderr ?? ''}` };
}

// Gibt einen Schritt aus und bricht bei Fehlern ab
function step(title, command, args) {
  console.log(`\n▸ ${title}`);
  if (!run(command, args).ok) {
    console.error(`\nFehlgeschlagen: ${title}. Die Meldung steht oben.`);
    process.exit(1);
  }
}

// Holt neue Änderungen (git pull), aber nur bei sauberem Arbeitsordner und ohne Merge; sonst wird nur gemeldet
function updateSources() {
  console.log('\n▸ Quellen aktualisieren');
  if (NO_PULL) return console.log('Übersprungen (--no-pull): nur bauen und Anpassungen anwenden.');
  if (!existsSync(join(ROOT, '.git'))) return console.log('Kein Git-Repository, übersprungen.');
  if (run('git', ['status', '--porcelain'], false).out.trim()) {
    return console.log('Es gibt lokale Änderungen; git pull wird übersprungen (bitte bei Bedarf selbst ausführen).');
  }
  if (!run('git', ['pull', '--ff-only']).ok) console.warn('git pull ist nicht gelungen; es wird mit dem lokalen Stand weitergebaut.');
}

updateSources();
step('Pakete bauen', 'npm', ['run', 'build']);
step('Plugin bauen (inkl. Rust-Binary)', 'npm', ['run', 'build:plugin']);
if (!existsSync(BUNDLED_BIN)) {
  console.error(`Fehlt nach dem Bauen: ${BUNDLED_BIN}`);
  process.exit(1);
}
step('Laufenden Server neu starten (falls er lief; sonst startet er mit `nexus board`)', BUNDLED_BIN, ['stop', '--restart']);
// `nexus install` kopiert Binary, Regeln und Anpassungen nach ~/.nexus, ergänzt den PATH und
// meldet das Plugin über den Marktplatz nexus-local bei Claude Code an
step('nexus im Terminal und Plugin in Claude Code einrichten', BUNDLED_BIN, ['install', '--marketplace', ROOT]);
// Übersicht der Anpassungen (dieselbe Datei wie das Panel „Anpassungen“ im Board)
console.log('\n▸ Anpassungen');
run(BUNDLED_BIN, ['customize', 'overview', '--file', join(PLUGIN_DIR, 'customizations/overview.json')]);

console.log('\nFertig.');
console.log('- Neues Terminal öffnen, dann funktioniert `nexus` in jedem Ordner (`harness` bleibt als alter Name nutzbar).');
console.log(`- Das Plugin ${PLUGIN} wird je Projekt durch \`nexus init\` eingeschaltet; in anderen Projekten ist es aus und kostet nichts.`);
console.log('- Im Projektordner: `nexus init`, dann Claude Code neu starten (oder /reload-plugins), dann `nexus board`.');
