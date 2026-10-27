import { spawn } from 'node:child_process';
import { HarnessError } from '@harness/core';

/** Wo eine Datei geöffnet wird: im Explorer (markiert) oder in VS Code */
export type OpenTarget = 'explorer' | 'vscode';

// Öffnet eine Datei auf diesem Rechner in einem Programm
export interface FileOpener {
  open(file: string, target: OpenTarget): Promise<void>;
}

// Der Server läuft auf dem Rechner der Person: Explorer und VS Code erscheinen dort. Nur unter Windows.
// VS Code startet über den Befehl `code` (vom Installer in den PATH gelegt); fehlt er, kommt open-unavailable.
export class WindowsFileOpener implements FileOpener {
  constructor(private readonly platform: string = process.platform) {}

  open(file: string, target: OpenTarget): Promise<void> {
    if (this.platform !== 'win32') {
      return Promise.reject(new HarnessError('open-unavailable', 'Dateien öffnen gibt es nur unter Windows.'));
    }
    return target === 'explorer'
      // Explorer endet auch beim Erfolg oft mit Code 1, deshalb zählt nur, dass er startet
      ? this.start('explorer.exe', [`/select,${file}`], false, 'Der Explorer')
      : this.start('code', [`"${file}"`], true, 'VS Code (Befehl „code“)');
  }

  // Startet das Programm losgelöst; mit `checkExit` meldet ein Ende mit Fehlercode, dass es nicht ging
  private start(command: string, args: string[], checkExit: boolean, name: string): Promise<void> {
    return new Promise((resolve, reject) => {
      const unavailable = (detail: string) => new HarnessError('open-unavailable', `${name} ließ sich nicht öffnen (${detail}).`);
      const child = spawn(command, args, { shell: checkExit, windowsHide: true, detached: true, stdio: 'ignore', windowsVerbatimArguments: !checkExit });
      child.on('error', (error) => reject(unavailable(error.message)));
      if (!checkExit) {
        child.on('spawn', () => { child.unref(); resolve(); });
        return;
      }
      child.on('exit', (code) => (code === 0 ? resolve() : reject(unavailable(`Code ${code}`))));
    });
  }
}
