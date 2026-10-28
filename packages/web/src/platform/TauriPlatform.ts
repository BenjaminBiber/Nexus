import type { OpenDocInput, Platform } from './Platform.js';

// Setzt Wurzel und Dokumentpfad mit dem Trennzeichen der Wurzel zusammen (Windows-Wurzeln tragen `\`)
export function joinDocPath(root: string, path: string): string {
  const windows = root.includes('\\');
  const sep = windows ? '\\' : '/';
  const base = root.replace(/[\\/]+$/, '');
  const rest = path.replace(/^[\\/]+/, '');
  return `${base}${sep}${windows ? rest.replace(/\//g, '\\') : rest}`;
}

// Adresse, mit der VS Code eine Datei öffnet: immer Schrägstriche, auch unter Windows (`vscode://file/C:/repo/…`)
export function vscodeUrl(full: string): string {
  const forward = full.replace(/\\/g, '/');
  return `vscode://file${encodeURI(forward.startsWith('/') ? forward : `/${forward}`)}`;
}

// Macht aus einem Fehler der Tauri-Plugins einen Fehler mit verständlichem deutschem Text
function failed(what: string, e: unknown): Error {
  const detail = e instanceof Error ? e.message : String(e);
  return new Error(`${what} (${detail})`);
}

// Plattform in der Desktop-App: Ordnerdialog und Öffnen laufen über die Tauri-Plugins dieses Rechners.
// Die Plugin-Pakete werden erst beim Aufruf geladen, damit das Browser-Bundle sie nicht enthält.
export class TauriPlatform implements Platform {
  readonly kind = 'tauri' as const;

  // Nativer Ordnerdialog; Abbruch liefert null
  async pickFolder(start?: string): Promise<string | null> {
    const { open } = await import('@tauri-apps/plugin-dialog');
    try {
      const picked = await open({ directory: true, multiple: false, defaultPath: start });
      return typeof picked === 'string' ? picked : null;
    } catch (e) {
      throw failed('Die Ordnerauswahl ließ sich nicht öffnen.', e);
    }
  }

  // Zeigt die Datei im Dateimanager oder öffnet sie in VS Code; beides braucht den vollen Pfad
  async openDoc(input: OpenDocInput): Promise<void> {
    if (!input.root) throw new Error('Der Ordner des Dokuments ist unbekannt.');
    const full = joinDocPath(input.root, input.path);
    const { openUrl, revealItemInDir } = await import('@tauri-apps/plugin-opener');
    if (input.target === 'explorer') {
      try {
        await revealItemInDir(full);
      } catch (e) {
        throw failed('Die Datei ließ sich im Dateimanager nicht zeigen.', e);
      }
      return;
    }
    try {
      await openUrl(vscodeUrl(full));
    } catch (e) {
      throw failed('VS Code ließ sich nicht öffnen.', e);
    }
  }

  // Standardbrowser dieses Rechners statt des App-Fensters
  async openExternal(url: string): Promise<void> {
    const { openUrl } = await import('@tauri-apps/plugin-opener');
    try {
      await openUrl(url);
    } catch (e) {
      throw failed('Die Adresse ließ sich nicht öffnen.', e);
    }
  }
}
