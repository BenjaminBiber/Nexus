// Entwicklungsstart: baut core, startet Harness-Server (4710) und Vite (5173) in einem Terminal.
import { spawn, spawnSync } from 'node:child_process';

const SHELL = process.platform === 'win32';
const children = [];

// Startet einen Prozess mit Namenspräfix im Terminal und merkt ihn für das Beenden
function start(name, command, args) {
  const child = spawn(command, args, { stdio: 'inherit', shell: SHELL, windowsHide: true });
  child.on('exit', (code) => {
    console.log(`[${name}] beendet (Code ${code})`);
    stopAll(code ?? 0);
  });
  children.push(child);
}

// Beendet alle Prozesse und das Skript
function stopAll(code) {
  for (const child of children) {
    if (child.exitCode !== null || child.killed) continue;
    if (SHELL) spawnSync('taskkill', ['/pid', String(child.pid), '/T', '/F'], { windowsHide: true });
    else child.kill();
  }
  process.exit(code);
}

console.log('Baue core …');
const build = spawnSync('npx', ['tsc', '-b', 'packages/core'], { stdio: 'inherit', shell: SHELL, windowsHide: true });
if (build.status !== 0) process.exit(build.status ?? 1);

console.log('Starte Server (http://127.0.0.1:4710) und Oberfläche (http://localhost:5173) …');
start('server', 'npx', ['tsx', 'packages/server/src/main.ts']);
start('web', 'npm', ['run', 'dev', '-w', 'packages/web']);

process.on('SIGINT', () => stopAll(0));
process.on('SIGTERM', () => stopAll(0));
