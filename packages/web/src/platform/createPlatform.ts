import type { ApiClient } from '../api/ApiClient.js';
import { BrowserPlatform } from './BrowserPlatform.js';
import type { Platform } from './Platform.js';
import { TauriPlatform } from './TauriPlatform.js';

// Wahr, wenn die Oberfläche im Fenster der Desktop-App läuft; Tauri legt dieses Feld vor dem Laden der Seite an
export function isTauri(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}

// Die Plattform dieser Umgebung: in der Desktop-App die Tauri-Plugins, sonst der Server
export function createPlatform(api: ApiClient): Platform {
  return isTauri() ? new TauriPlatform() : new BrowserPlatform(api);
}
