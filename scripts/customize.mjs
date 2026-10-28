// Anpassungen im Clone prüfen, Standard kopieren oder zurücksetzen: npm run customize -- check | copy … | reset …
// Ruft nexus customize im Rust-Binary auf und gibt Ausgabe und Rückgabecode weiter.
// Ordner der Anpassungen für Tests über HARNESS_CUSTOM_DIR.
import { spawnSync } from 'node:child_process';
import { existsSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const CUSTOM = process.env.HARNESS_CUSTOM_DIR || join(ROOT, 'custom');
// In Fehlermeldungen steht der Aufruf, mit dem der Dev gearbeitet hat, nicht der des Binaries
const AUFRUF = 'npm run customize --';
const OPTIONS = ['--clone', ROOT, '--custom', CUSTOM, '--aufruf', AUFRUF];

// Ein gebautes Binary ist schneller; ohne eines baut und startet cargo es
function befehl(args) {
  const binary = join(ROOT, 'target/release/nexus' + (process.platform === 'win32' ? '.exe' : ''));
  if (existsSync(binary)) return { command: binary, args: ['customize', ...OPTIONS, ...args] };
  return { command: 'cargo', args: ['run', '-q', '-p', 'nexus-cli', '--', 'customize', ...OPTIONS, ...args] };
}

const { command, args } = befehl(process.argv.slice(2));
const result = spawnSync(command, args, { cwd: ROOT, stdio: 'inherit', windowsHide: true });
if (result.error) {
  console.error(`${command} ließ sich nicht starten: ${result.error.message}`);
  console.error('Für die Anpassungen braucht es Rust (cargo) oder ein gebautes target/release/nexus.');
  process.exit(1);
}
process.exit(result.status ?? 1);
