// Richtet den Harness auf diesem Rechner ein (einmal, wiederholbar zum Aktualisieren):
// baut alles, installiert das Plugin global für den Benutzer und macht `harness` im Terminal verfügbar.
import { spawnSync } from 'node:child_process';
import { existsSync, readFileSync } from 'node:fs';
import { homedir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const MARKETPLACE = 'ai-harness-local';
const PLUGIN = `ai-harness@${MARKETPLACE}`;
const BUNDLED_CLI = join(ROOT, 'packages/plugin/dist/ai-harness/lib/cli.mjs');
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

// Aktualisiert die Projekt-Einträge des Plugins (von `harness init` eingeschaltet), sonst bleiben sie auf dem alten Stand
function updateProjectInstalls() {
  let entries = [];
  try {
    const file = join(process.env.CLAUDE_CONFIG_DIR || join(homedir(), '.claude'), 'plugins', 'installed_plugins.json');
    entries = JSON.parse(readFileSync(file, 'utf8')).plugins?.[PLUGIN] ?? [];
  } catch {
    return;
  }
  for (const entry of entries) {
    if (entry.scope === 'user' || !entry.projectPath || !existsSync(entry.projectPath)) continue;
    const ok = run('claude', ['plugin', 'update', PLUGIN, '--scope', entry.scope], false, entry.projectPath).ok;
    console.log(`  ${ok ? 'aktualisiert' : 'nicht aktualisiert'}: ${entry.projectPath}`);
  }
}

// Gibt die Übersicht der Anpassungen aus (dieselbe Datei wie das Panel „Anpassungen“ im Board)
async function printCustomizations() {
  const file = join(ROOT, 'packages/plugin/dist/ai-harness/customizations/overview.json');
  if (!existsSync(file)) return console.warn('Keine Übersicht der Anpassungen gefunden (customizations/overview.json fehlt).');
  // Fehler hier dürfen setup nicht abbrechen (SC-002): die Installation ist dann schon erledigt
  try {
    const core = await import(pathToFileURL(join(ROOT, 'packages/core/dist/index.js')).href);
    console.log('\n▸ Anpassungen');
    console.log(new core.OverviewFormatter().format(JSON.parse(readFileSync(file, 'utf8'))));
  } catch (error) {
    console.warn(`Übersicht der Anpassungen nicht lesbar: ${error instanceof Error ? error.message : String(error)}`);
  }
}

// Meldet das Plugin über den lokalen Marktplatz bei Claude Code an (Benutzerebene, also in jedem Projekt)
function installPlugin() {
  console.log('\n▸ Plugin in Claude Code installieren (für alle Projekte)');
  if (!run('claude', ['--version'], false).ok) {
    console.warn('Claude Code (Befehl `claude`) wurde nicht gefunden. Das Plugin muss später selbst installiert werden:');
    console.warn(`  claude plugin marketplace add "${ROOT}"`);
    console.warn(`  claude plugin install ${PLUGIN}`);
    return false;
  }
  if (!run('claude', ['plugin', 'marketplace', 'list'], false).out.includes(MARKETPLACE)) {
    if (!run('claude', ['plugin', 'marketplace', 'add', ROOT]).ok) return false;
  }
  // Marktplatz neu einlesen, sonst sieht `plugin update` den frisch gebauten Stand nicht
  run('claude', ['plugin', 'marketplace', 'update', MARKETPLACE]);
  const installed = run('claude', ['plugin', 'list'], false).out.includes(PLUGIN);
  if (!run('claude', ['plugin', installed ? 'update' : 'install', PLUGIN]).ok) return false;
  updateProjectInstalls();
  // Auf Benutzerebene ausschalten: `harness init` schaltet es je Projekt ein, sonst kostet es überall Zeit und Kontext
  console.log('Plugin auf Benutzerebene ausschalten (wird je Projekt mit `harness init` eingeschaltet)');
  run('claude', ['plugin', 'disable', PLUGIN, '--scope', 'user']);
  return true;
}

updateSources();
step('Pakete bauen', 'npm', ['run', 'build']);
step('Plugin bauen', 'npm', ['run', 'build:plugin']);
if (!existsSync(BUNDLED_CLI)) {
  console.error(`Fehlt nach dem Bauen: ${BUNDLED_CLI}`);
  process.exit(1);
}
const pluginOk = installPlugin();
step('Laufenden Server neu starten (falls er lief; sonst startet er mit `harness board`)', process.execPath, [BUNDLED_CLI, 'stop', '--restart']);
step('harness im Terminal einrichten', process.execPath, [BUNDLED_CLI, 'install']);
await printCustomizations();

console.log('\nFertig.');
console.log('- Neues Terminal öffnen, dann funktioniert `harness` in jedem Ordner.');
console.log(pluginOk ? '- Das Plugin wird je Projekt durch `harness init` eingeschaltet; in anderen Projekten ist es aus und kostet nichts.' : '- Das Plugin ist noch nicht installiert (siehe Hinweis oben).');
console.log('- Im Projektordner: `harness init`, dann Claude Code neu starten (oder /reload-plugins), dann `harness board`.');
