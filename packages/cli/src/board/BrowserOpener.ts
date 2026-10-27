import { spawn } from 'node:child_process';

// Öffnet eine Adresse im Standardbrowser; scheitert das, bleibt die Adresse im Terminal stehen.
export class BrowserOpener {
  // Startet den passenden Systembefehl abgelöst; liefert, ob er gestartet werden konnte
  open(url: string): Promise<boolean> {
    const [command, args] = this.commandFor(url);
    return new Promise((resolve) => {
      try {
        const child = spawn(command, args, { detached: true, stdio: 'ignore', windowsHide: true });
        child.on('error', () => resolve(false));
        child.on('spawn', () => {
          child.unref();
          resolve(true);
        });
      } catch {
        resolve(false);
      }
    });
  }

  // Befehl je Betriebssystem (unter Windows ohne Shell, damit keine Anführungszeichen nötig sind)
  private commandFor(url: string): [string, string[]] {
    if (process.platform === 'win32') return ['rundll32', ['url.dll,FileProtocolHandler', url]];
    if (process.platform === 'darwin') return ['open', [url]];
    return ['xdg-open', [url]];
  }
}
