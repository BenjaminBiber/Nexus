import { chmod, mkdir, writeFile } from 'node:fs/promises';
import { join } from 'node:path';

// Schreibt die Startdateien `harness.cmd` (cmd und PowerShell) und `harness` (Bash, Git Bash, macOS, Linux).
// Beide rufen das kopierte CLI unter `<home>/app/lib/cli.mjs` mit dem `node` aus dem PATH auf.
export class LauncherWriter {
  // Legt beide Startdateien im Ordner an und liefert ihre Pfade
  async write(binDir: string): Promise<string[]> {
    await mkdir(binDir, { recursive: true });
    const cmd = join(binDir, 'harness.cmd');
    const sh = join(binDir, 'harness');
    await writeFile(cmd, '@echo off\r\nnode "%~dp0..\\app\\lib\\cli.mjs" %*\r\n', 'utf8');
    await writeFile(sh, '#!/bin/sh\nexec node "$(dirname "$0")/../app/lib/cli.mjs" "$@"\n', 'utf8');
    await chmod(sh, 0o755);
    return [cmd, sh];
  }
}
