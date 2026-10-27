import { existsSync } from 'node:fs';
import { mkdir, readFile } from 'node:fs/promises';
import { AtomicWriter } from '../locking/AtomicWriter.js';
import type { HarnessHome } from '../registry/HarnessHome.js';
import { DEFAULT_NTFY_SERVER, DEFAULT_USER_SETTINGS, NOTIFICATION_MODES, NTFY_TOPIC_PATTERN, THEME_MODES, type UserSettings } from './UserSettings.js';

// Speichert die benutzerweiten Einstellungen in `~/.harness/settings.json` (atomar).
export class UserSettingsStore {
  private readonly writer = new AtomicWriter();

  constructor(private readonly home: HarnessHome) {}

  // Liest die Einstellungen; fehlende oder beschädigte Datei ergibt die Standardwerte
  async read(): Promise<UserSettings> {
    if (!existsSync(this.home.settingsFile)) return { ...DEFAULT_USER_SETTINGS };
    try {
      return this.normalize(JSON.parse(await readFile(this.home.settingsFile, 'utf8')));
    } catch {
      return { ...DEFAULT_USER_SETTINGS };
    }
  }

  // Schreibt die Einstellungen; der Ordner wird bei Bedarf angelegt
  async write(settings: UserSettings): Promise<void> {
    await mkdir(this.home.dir, { recursive: true });
    await this.writer.write(this.home.settingsFile, JSON.stringify(settings, null, 2));
  }

  // Übernimmt nur gültige Werte; der frühere Schalter `notifyOnWaiting` wird zu „browser“
  private normalize(raw: unknown): UserSettings {
    const data = (raw ?? {}) as Record<string, unknown>;
    const legacy = data.notifyOnWaiting === true ? 'browser' : DEFAULT_USER_SETTINGS.notifications;
    const mode = NOTIFICATION_MODES.find((m) => m === data.notifications) ?? legacy;
    return { notifications: mode, heroImages: data.heroImages === true, theme: THEME_MODES.find((t) => t === data.theme) ?? 'system', claudeUsage: data.claudeUsage === true, compactCards: data.compactCards === true, hideNotNeeded: data.hideNotNeeded === true, hideInPlanning: data.hideInPlanning === true, ...this.ntfy(data) };
  }

  // ntfy-Felder: Server als http(s)-Adresse, Thema nur mit erlaubten Zeichen, sonst Standard
  private ntfy(data: Record<string, unknown>): Pick<UserSettings, 'ntfy' | 'ntfyServer' | 'ntfyTopic' | 'ntfyToken'> {
    const server = typeof data.ntfyServer === 'string' && /^https?:\/\/\S+$/.test(data.ntfyServer) ? data.ntfyServer : DEFAULT_NTFY_SERVER;
    const topic = typeof data.ntfyTopic === 'string' && NTFY_TOPIC_PATTERN.test(data.ntfyTopic) ? data.ntfyTopic : '';
    return { ntfy: data.ntfy === true, ntfyServer: server, ntfyTopic: topic, ntfyToken: typeof data.ntfyToken === 'string' ? data.ntfyToken : '' };
  }
}
