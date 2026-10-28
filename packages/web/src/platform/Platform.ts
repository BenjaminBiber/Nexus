// Was ein Dokument zum Öffnen braucht: Projekt und Pfad für den Server, `root` für den vollen Pfad in der Desktop-App
export interface OpenDocInput {
  pid: string;
  path: string;
  root: string | null;
  board?: number;
  target: 'explorer' | 'vscode';
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
}
