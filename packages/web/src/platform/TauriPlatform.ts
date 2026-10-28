import type { NotifyInput, NotifyPermission, OpenDocInput, Platform } from './Platform.js';

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

  // Jedes System der App kann Meldungen zeigen; die Erlaubnis dafür holt das Plugin
  notificationsSupported(): boolean {
    return true;
  }

  // Das Plugin kennt nur „erteilt“ oder „nicht erteilt“; ohne Erlaubnis gilt `default`, damit die Einstellung noch fragen darf
  async notificationPermission(): Promise<NotifyPermission> {
    const { isPermissionGranted } = await import('@tauri-apps/plugin-notification');
    try {
      return (await isPermissionGranted()) ? 'granted' : 'default';
    } catch (e) {
      throw failed('Der Stand der Erlaubnis für Meldungen ließ sich nicht lesen.', e);
    }
  }

  // Erst fragen, wenn es die Erlaubnis nicht schon gibt; der Dialog des Systems erscheint nur beim ersten Mal
  async requestNotificationPermission(): Promise<NotifyPermission> {
    const { isPermissionGranted, requestPermission } = await import('@tauri-apps/plugin-notification');
    try {
      if (await isPermissionGranted()) return 'granted';
      return await requestPermission();
    } catch (e) {
      throw failed('Die Erlaubnis für Meldungen ließ sich nicht einholen.', e);
    }
  }

  // Systemmeldung dieses Rechners, auch bei verstecktem Fenster.
  // Das Plugin nimmt nur Titel und Text; ein Klick holt die App nach vorn, meldet sich aber nicht zurück – `onClick` und `url` bleiben ungenutzt.
  // Zwei Meldungen mit gleichem `tag` ersetzen sich nicht, sie stehen nebeneinander.
  async notify(input: NotifyInput): Promise<void> {
    const { sendNotification } = await import('@tauri-apps/plugin-notification');
    try {
      sendNotification({ title: input.title, body: input.body });
    } catch (e) {
      throw failed('Die Meldung ließ sich nicht zeigen.', e);
    }
  }
}
