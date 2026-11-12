import { execFileSync, spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, realpathSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { BenchApi } from './BenchApi.mjs';

// Programmdatei des CLI, mit der das Testprojekt eingerichtet wird (aus `npm run build`)
const CLI_ENTRY = join('packages', 'cli', 'dist', 'main.js');
// Skript, das das Plugin-Bündel baut
const PLUGIN_BUILD = join('packages', 'plugin', 'scripts', 'build-plugin.mjs');

// Testprojekt für die Hook-Messung: baut das Plugin-Bündel in einen temporären Ordner, legt ein Git-Projekt an,
// richtet es mit `nexus init` ein, meldet es am gemessenen Server an und ordnet ihm das Bench-Board zu.
// Gemessen wird damit genau das, was Claude Code startet: `node <plugin>/lib/cli.mjs hook <ereignis>`.
export class BenchProject {
  constructor({ cloneDir, home, hookCmd = null, log = () => {} }) {
    this.cloneDir = cloneDir;
    this.home = home;
    this.hookCmd = hookCmd;
    this.log = log;
    // realpathSync.native löst auch 8.3-Kurznamen unter Windows auf; git meldet den Langpfad, sonst passt die
    // Repo-Zuordnung des Boards nicht zum Ordner
    this.base = realpathSync.native(mkdtempSync(join(tmpdir(), 'harness-bench-hook-')));
    this.repo = join(this.base, 'projekt');
    this.pluginDir = join(this.base, 'plugin');
    this.claudeDir = join(this.base, 'claude');
    // build-plugin.mjs legt die Anpassungen aus custom/ über den Standard; für die Messung gilt der Standard
    this.customDir = join(this.base, 'custom');
    this.projectId = null;
    this.token = '';
  }

  // Prüft, dass die gebauten Programmdateien vorhanden sind (sonst fehlt `npm run build`).
  // Mit eigenem Hook-Befehl wird das Testprojekt mit dessen Programm eingerichtet; dann braucht es
  // weder das gebaute TypeScript-CLI noch das Plugin-Bündel.
  assertBuilt() {
    for (const file of [this.hookCmd ? null : CLI_ENTRY, this.hookCmd ? null : PLUGIN_BUILD].filter(Boolean)) {
      if (!existsSync(join(this.cloneDir, file))) {
        throw new Error(`${file} fehlt – bitte zuerst „npm run build“ im Hauptordner des Repos ausführen.`);
      }
    }
  }

  // Richtet alles ein: Plugin-Bündel, Git-Projekt, `nexus init`, Projekt am Server, Board-Zuordnung
  async prepare(serverUrl, boardId) {
    this.assertBuilt();
    mkdirSync(this.customDir, { recursive: true });
    mkdirSync(this.claudeDir, { recursive: true });
    if (!this.hookCmd) this.buildPlugin();
    this.createRepo();
    this.init();
    this.token = readFileSync(join(this.gitDir(), 'harness', 'token'), 'utf8').trim();
    const api = new BenchApi(serverUrl);
    const project = await api.send('POST', '/api/projects', { path: this.repo });
    if (typeof project?.id !== 'string') throw new Error(`Das Projekt wurde ohne ID angemeldet: ${JSON.stringify(project)}`);
    this.projectId = project.id;
    // Ohne Repo-Zuordnung lädt der Hook keinen Board-Kontext und die Messung wäre zu günstig
    await api.send('PATCH', `/api/db/boards/${boardId}`, { repoPath: this.repo });
    this.assertBoard(boardId);
    this.log(`  Testprojekt ${this.repo} (Projekt ${this.projectId}, Board ${boardId})`);
  }

  // Prüft, dass das CLI im Testprojekt das Bench-Board findet: sonst lädt der Hook keinen Board-Kontext
  // und die gemessene Laufzeit wäre zu günstig. `init` ist wiederholbar und nennt das Board des Repos.
  assertBoard(boardId) {
    const found = this.init().board;
    if (found?.id !== boardId) {
      throw new Error(`Das Testprojekt ${this.repo} ist nicht dem Bench-Board ${boardId} zugeordnet (gefunden: ${JSON.stringify(found)}).`);
    }
  }

  // Befehl und Umgebung eines Hook-Aufrufs; `port` ist der laufende Bench-Server, damit kein Hook einen Server startet
  hookCommand(port) {
    const [command, ...args] = this.hookCmd ? splitCommand(this.hookCmd) : [process.execPath, join(this.pluginDir, 'lib', 'cli.mjs'), 'hook'];
    return { command, args, cwd: this.repo, env: this.hookEnv(port) };
  }

  // Gemessener Hook-Befehl für die Ergebnisdatei; der temporäre Ordner wechselt je Lauf und steht deshalb nicht darin
  hookDescription() {
    return this.hookCmd ?? 'node <plugin>/lib/cli.mjs hook';
  }

  // Umgebung der Hook-Aufrufe: eigenes HARNESS_HOME, der Port des Bench-Servers und ein eigener Claude-Ordner.
  // CLAUDE_ENV_FILE muss weg, sonst schreibt der Hook in die Env-Datei einer laufenden Claude-Session;
  // HARNESS_SESSION und HARNESS_TICKET einer laufenden Session gelten für die Messung nicht.
  hookEnv(port) {
    const env = { ...process.env, HARNESS_HOME: this.home, CLAUDE_CONFIG_DIR: this.claudeDir };
    if (port) env.HARNESS_PORT = String(port);
    delete env.HARNESS_SESSION;
    delete env.HARNESS_TICKET;
    delete env.CLAUDE_ENV_FILE;
    return env;
  }

  // Räumt den temporären Ordner des Testprojekts
  dispose() {
    rmSync(this.base, { recursive: true, force: true });
  }

  // Baut das Plugin-Bündel in den temporären Ordner; nie nach packages/plugin/dist/ai-harness (das nutzen laufende Sessions)
  buildPlugin() {
    this.log(`  Baue das Plugin-Bündel nach ${this.pluginDir} …`);
    const result = spawnSync(process.execPath, [join(this.cloneDir, PLUGIN_BUILD)], {
      cwd: this.cloneDir,
      encoding: 'utf8',
      windowsHide: true,
      env: { ...process.env, HARNESS_PLUGIN_OUT: this.pluginDir, HARNESS_CUSTOM_DIR: this.customDir },
    });
    if (result.error) throw new Error(`Das Plugin-Bündel lässt sich nicht bauen: ${result.error.message}`);
    if (result.status !== 0) {
      throw new Error(`Das Plugin-Bündel lässt sich nicht bauen (Code ${result.status}): ${(result.stderr || result.stdout || '').slice(-500)}`);
    }
  }

  // Leeres Git-Projekt mit erstem Commit (wie TempProject in den Prozess-Tests)
  createRepo() {
    mkdirSync(this.repo, { recursive: true });
    this.git(['init', '-q']);
    this.git(['-c', 'user.name=t', '-c', 'user.email=t@t', 'commit', '--allow-empty', '-q', '-m', 'init']);
  }

  // `nexus init`: Harness-Ordner, Token und Zähler im Testprojekt; wiederholbar. Liefert die Ausgabe als Objekt.
  init() {
    // Ohne das angehängte Ereignis: der Hook-Befehl endet auf „hook“, das Einrichten nutzt dasselbe Programm
    const ohneUnterbefehl = this.hookCmd ? splitCommand(this.hookCmd).slice(0, -1) : null;
    if (ohneUnterbefehl && ohneUnterbefehl.length === 0) {
      throw new Error(`Der Hook-Befehl „${this.hookCmd}“ nennt nur ein Programm; erwartet wird er samt Unterbefehl, z. B. „nexus hook“.`);
    }
    const [command, ...vorher] = ohneUnterbefehl ?? [process.execPath, join(this.cloneDir, CLI_ENTRY)];
    const result = spawnSync(command, [...vorher, '--json', 'init'], {
      cwd: this.repo,
      encoding: 'utf8',
      windowsHide: true,
      env: this.hookEnv(null),
    });
    if (result.error) throw new Error(`„nexus init“ lässt sich nicht starten: ${result.error.message}`);
    if (result.status !== 0) throw new Error(`„nexus init“ endet mit Code ${result.status}: ${(result.stderr || result.stdout || '').slice(-500)}`);
    try {
      return JSON.parse(result.stdout);
    } catch (error) {
      throw new Error(`Die Ausgabe von „nexus init“ ist kein JSON (${error.message}): ${result.stdout.slice(-300)}`);
    }
  }

  // Git-Ordner des Testprojekts (dort liegt der Harness-Ordner mit dem Token)
  gitDir() {
    return this.git(['rev-parse', '--absolute-git-dir']);
  }

  // Führt Git im Testprojekt aus
  git(args) {
    return execFileSync('git', args, { cwd: this.repo, encoding: 'utf8', windowsHide: true }).trim();
  }
}

// Zerlegt einen Befehl in Programm und Argumente; Teile in doppelten Anführungszeichen bleiben zusammen
export function splitCommand(text) {
  const parts = text.match(/"[^"]*"|\S+/g) ?? [];
  const command = parts.map((part) => (part.startsWith('"') && part.endsWith('"') ? part.slice(1, -1) : part));
  if (command.length === 0) throw new Error('Der Hook-Befehl ist leer.');
  return command;
}
