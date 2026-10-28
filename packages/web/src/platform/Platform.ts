// Was ein Dokument zum Öffnen braucht: Projekt und Pfad für den Server, `root` für den vollen Pfad in der Desktop-App
export interface OpenDocInput {
  pid: string;
  path: string;
  root: string | null;
  board?: number;
  target: 'explorer' | 'vscode';
}

// Stand der Erlaubnis für Meldungen: erteilt, abgelehnt oder noch nicht gefragt
export type NotifyPermission = 'granted' | 'denied' | 'default';

// Was eine Meldung braucht: Titel und Text, `tag` ersetzt eine gleichnamige frühere Meldung,
// `url` und `onClick` gelten beim Klick (nur im Browser, die Desktop-App meldet keinen Klick zurück)
export interface NotifyInput {
  title: string;
  body: string;
  tag: string;
  url: string;
  onClick?: () => void;
}

// Systemfunktionen, die in der Desktop-App anders laufen als im Browser.
// Im Browser erledigt sie der Server (nur Windows), in Tauri die Plugins dieses Rechners.
export interface Platform {
  readonly kind: 'browser' | 'tauri';
  // Öffnet den Ordnerdialog; Abbruch ergibt null
  pickFolder(start?: string): Promise<string | null>;
  // Zeigt das Dokument im Dateimanager oder öffnet es in VS Code
  openDoc(input: OpenDocInput): Promise<void>;
  // Öffnet eine Adresse im Standardbrowser
  openExternal(url: string): Promise<void>;
  // Kann diese Umgebung überhaupt Meldungen zeigen?
  notificationsSupported(): boolean;
  // Stand der Erlaubnis, ohne zu fragen
  notificationPermission(): Promise<NotifyPermission>;
  // Fragt die Erlaubnis; mit erteilter Erlaubnis antwortet sie sofort `granted`
  requestNotificationPermission(): Promise<NotifyPermission>;
  // Zeigt eine Meldung; ohne Erlaubnis passiert nichts
  notify(input: NotifyInput): Promise<void>;
}
