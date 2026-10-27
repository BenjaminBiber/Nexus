import { HarnessError, NOTIFICATION_MODES, NTFY_TOPIC_PATTERN, THEME_MODES, type UserSettings, type UserSettingsStore } from '@harness/core';
import type { FastifyInstance } from 'fastify';
import type { NotificationService } from '../notifications/NotificationService.js';

// /api/user-settings: benutzerweite Einstellungen (Benachrichtigung, Hero-Bilder, Farbschema, Nutzungslimits, kompakte Ticket-Kacheln, Board-Spalten, ntfy) und Testmeldungen.
export class UserSettingsRoutes {
  constructor(
    private readonly store: UserSettingsStore,
    private readonly notifications: NotificationService,
  ) {}

  // Meldet die Routen an
  register(app: FastifyInstance): void {
    app.get('/api/user-settings', async () => this.store.read());
    app.put('/api/user-settings', async (request) => {
      const merged = this.merge(await this.store.read(), (request.body ?? {}) as Record<string, unknown>);
      await this.store.write(merged);
      return merged;
    });
    app.post('/api/user-settings/test-notification', async () => {
      const error = await this.notifications.sendTest();
      return { ok: error === null, error };
    });
    app.post('/api/user-settings/test-ntfy', async () => {
      const error = await this.notifications.sendNtfyTest();
      return { ok: error === null, error };
    });
  }

  // Übernimmt die angegebenen Felder; ungültige Werte werden abgelehnt
  private merge(current: UserSettings, body: Record<string, unknown>): UserSettings {
    const next = { ...current };
    if (body.notifications !== undefined) {
      const mode = NOTIFICATION_MODES.find((m) => m === body.notifications);
      if (!mode) throw new HarnessError('settings-invalid', 'notifications muss off, browser oder windows sein.');
      next.notifications = mode;
    }
    if (body.heroImages !== undefined) {
      if (typeof body.heroImages !== 'boolean') throw new HarnessError('settings-invalid', 'heroImages muss true oder false sein.');
      next.heroImages = body.heroImages;
    }
    if (body.theme !== undefined) {
      const theme = THEME_MODES.find((t) => t === body.theme);
      if (!theme) throw new HarnessError('settings-invalid', 'theme muss system, light, dark, signal, pastel oder win95 sein.');
      next.theme = theme;
    }
    if (body.claudeUsage !== undefined) {
      if (typeof body.claudeUsage !== 'boolean') throw new HarnessError('settings-invalid', 'claudeUsage muss true oder false sein.');
      next.claudeUsage = body.claudeUsage;
    }
    if (body.compactCards !== undefined) {
      if (typeof body.compactCards !== 'boolean') throw new HarnessError('settings-invalid', 'compactCards muss true oder false sein.');
      next.compactCards = body.compactCards;
    }
    if (body.hideNotNeeded !== undefined) {
      if (typeof body.hideNotNeeded !== 'boolean') throw new HarnessError('settings-invalid', 'hideNotNeeded muss true oder false sein.');
      next.hideNotNeeded = body.hideNotNeeded;
    }
    if (body.hideInPlanning !== undefined) {
      if (typeof body.hideInPlanning !== 'boolean') throw new HarnessError('settings-invalid', 'hideInPlanning muss true oder false sein.');
      next.hideInPlanning = body.hideInPlanning;
    }
    if (body.ntfy !== undefined) {
      if (typeof body.ntfy !== 'boolean') throw new HarnessError('settings-invalid', 'ntfy muss true oder false sein.');
      next.ntfy = body.ntfy;
    }
    if (body.ntfyServer !== undefined) {
      const server = typeof body.ntfyServer === 'string' ? body.ntfyServer.trim() : '';
      if (!/^https?:\/\/\S+$/.test(server)) throw new HarnessError('settings-invalid', 'ntfyServer muss eine http- oder https-Adresse sein.');
      next.ntfyServer = server;
    }
    if (body.ntfyTopic !== undefined) {
      const topic = typeof body.ntfyTopic === 'string' ? body.ntfyTopic.trim() : null;
      if (topic === null || (topic !== '' && !NTFY_TOPIC_PATTERN.test(topic))) throw new HarnessError('settings-invalid', 'ntfyTopic darf nur Buchstaben, Ziffern, - und _ enthalten (höchstens 64 Zeichen).');
      next.ntfyTopic = topic;
    }
    if (body.ntfyToken !== undefined) {
      if (typeof body.ntfyToken !== 'string') throw new HarnessError('settings-invalid', 'ntfyToken muss ein Text sein.');
      next.ntfyToken = body.ntfyToken.trim();
    }
    return next;
  }
}
