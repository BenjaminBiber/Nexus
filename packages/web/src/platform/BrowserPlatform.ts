import type { ApiClient } from '../api/ApiClient.js';
import type { NotifyInput, NotifyPermission, OpenDocInput, Platform } from './Platform.js';

// Plattform im Browser: alles läuft wie bisher über den Server dieses Projekts.
export class BrowserPlatform implements Platform {
  readonly kind = 'browser' as const;

  constructor(private readonly api: ApiClient) {}

  // Ordnerdialog des Servers (nur Windows); Abbruch liefert null
  async pickFolder(start?: string): Promise<string | null> {
    const { path } = await this.api.pickDbFolder(start);
    return path;
  }

  // Der Server startet Explorer oder VS Code auf dem Rechner, auf dem er läuft
  async openDoc(input: OpenDocInput): Promise<void> {
    await this.api.openDoc(input.pid, input.path, input.target, input.board);
  }

  // Ein neuer Tab des Browsers; ohne Zugriff auf das Fenster dieser Seite
  async openExternal(url: string): Promise<void> {
    window.open(url, '_blank', 'noopener');
  }

  // Alte Browser kennen `Notification` nicht; dann gibt es hier keine Meldungen
  notificationsSupported(): boolean {
    return typeof Notification !== 'undefined';
  }

  // Der Stand, den der Browser für diese Seite gespeichert hat
  async notificationPermission(): Promise<NotifyPermission> {
    return this.notificationsSupported() ? Notification.permission : 'denied';
  }

  // Der Browser zeigt seinen eigenen Dialog; nur eine Nutzergeste darf ihn auslösen
  async requestNotificationPermission(): Promise<NotifyPermission> {
    if (!this.notificationsSupported()) return 'denied';
    return await Notification.requestPermission();
  }

  // Meldung des Browsers; der Klick holt diesen Tab nach vorn, statt einen neuen zu öffnen.
  // Die Prüfung bleibt synchron, damit die Meldung im selben Durchlauf erscheint wie das Ereignis.
  async notify(input: NotifyInput): Promise<void> {
    if (!this.notificationsSupported() || Notification.permission !== 'granted') return;
    const notification = new Notification(input.title, { body: input.body, tag: input.tag });
    notification.onclick = (event) => {
      event.preventDefault();
      window.focus();
      input.onClick?.();
      notification.close();
    };
  }
}
