import type { ApiClient } from '../api/ApiClient.js';
import type { OpenDocInput, Platform } from './Platform.js';

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
}
