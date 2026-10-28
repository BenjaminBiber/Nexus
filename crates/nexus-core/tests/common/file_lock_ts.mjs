// Gegenprobe zur Rust-Sperre: nutzt die FileLock aus dem gebauten TypeScript-Harness.
// Aufruf: node file_lock_ts.mjs <modus> <FileLock.js> <sperrdatei> [<bereit> <freigabe>]
//   halten    – holt die Sperre, legt die Bereit-Datei an und hält sie, bis die Freigabe-Datei auftaucht
//   versuchen – versucht die Sperre kurz zu holen; „belegt“ (Rückgabecode 3) heißt: jemand anderes hält sie
import { existsSync, writeFileSync } from 'node:fs';
import { pathToFileURL } from 'node:url';

const [mode, modulePath, lockPath, readyFile, releaseFile] = process.argv.slice(2);
const { FileLock } = await import(pathToFileURL(modulePath).href);

// Wartezeit großzügig, damit nicht die Zeit, sondern die Sperre das Ergebnis bestimmt;
// staleMs hoch, damit keine Seite die frische Sperre der anderen für verwaist hält
const options = mode === 'halten' ? { timeoutMs: 30000, staleMs: 60000 } : { timeoutMs: 300, staleMs: 60000 };
const lock = new FileLock(lockPath, options);

if (mode === 'halten') {
  await lock.withLock(async () => {
    writeFileSync(readyFile, 'bereit');
    while (!existsSync(releaseFile)) await new Promise((resolve) => setTimeout(resolve, 20));
  });
  console.log('freigegeben');
} else {
  try {
    await lock.withLock(() => undefined);
    console.log('bekommen');
  } catch (error) {
    if (error?.code !== 'lock-timeout') throw error;
    console.log('belegt');
    process.exit(3);
  }
}
