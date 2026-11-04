// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { ApiProvider } from '../src/api/ApiContext.js';
import type { BotState, BotStatus, TelegramConfig, TelegramConfigInput, TelegramStatusDto } from '../src/api/telegramTypes.js';
import { TelegramPanel } from '../src/components/TelegramPanel.js';
import { PAIR_POLL_MS } from '../src/components/TelegramPairDialog.js';
import type { Platform } from '../src/platform/Platform.js';
import { PlatformProvider } from '../src/platform/PlatformContext.js';
import { fakeApi } from './helpers/fakeApi.js';
import { fakePlatform } from './helpers/fakePlatform.js';

afterEach(cleanup);

// Maske, wie der Server sie liefert: Bot-ID, Doppelpunkt, Auslassungszeichen, letzte vier Zeichen
const MASKED = '123456:…Dsaw';

const CONFIG: TelegramConfig = {
  enabled: true,
  botToken: MASKED,
  apiBase: 'https://api.telegram.org',
  chats: [4711, 815],
  notify: { review: true, blocked: true, question: true, merge: true, merged: false, paused: false },
  quietHours: null,
  profile: { applied: { botId: 42, photo: 'p', texts: 't' } },
  ideas: { defaultTarget: 'note', maxRounds: 3 },
  speech: { backend: 'off', baseUrl: 'https://api.openai.com/v1', apiKey: '', model: 'whisper-1', language: null, whisperCli: { binary: 'whisper-cli', model: '', ffmpeg: 'ffmpeg' } },
};

const RUNNING: BotStatus = { state: 'running', botName: 'nexus_bot', botId: 42, lastUpdateAt: null, lastError: null, profileError: null };

function statusDto(bot: Partial<BotStatus> & { state: BotState }): TelegramStatusDto {
  return { bot: { ...RUNNING, ...bot }, pairedChats: CONFIG.chats.length, openCodes: 0 };
}

// Rendert das Panel mit austauschbaren Aufrufen
function open(
  overrides: Record<string, unknown> = {},
  config: TelegramConfig = CONFIG,
  status: TelegramStatusDto = statusDto(RUNNING),
  platform: Platform = fakePlatform(),
) {
  render(
    <ApiProvider api={fakeApi({ getTelegramConfig: async () => config, getTelegramStatus: async () => status, ...overrides })}>
      <PlatformProvider value={platform}>
        <TelegramPanel />
      </PlatformProvider>
    </ApiProvider>,
  );
}

describe('TelegramPanel', () => {
  it('lädt die Einstellung und zeigt das Token nur als Maske', async () => {
    open();
    expect((await screen.findByLabelText('Gespeichertes Bot-Token')).textContent).toBe(MASKED);
    expect(screen.getByRole('button', { name: 'Bot aktiv' }).getAttribute('aria-pressed')).toBe('true');
    expect((screen.getByLabelText('Rückfragen pro Idee') as HTMLInputElement).value).toBe('3');
    expect((screen.getByLabelText('Ablegen als') as HTMLSelectElement).value).toBe('note');
  });

  it('speichert Änderungen ohne das Token im Klartext', async () => {
    const save = vi.fn(async (input: TelegramConfigInput) => ({ ...CONFIG, ...input, chats: CONFIG.chats, profile: CONFIG.profile }) as TelegramConfig);
    open({ saveTelegramConfig: save });
    await userEvent.click(await screen.findByLabelText('Gemergt'));
    await userEvent.click(screen.getByRole('button', { name: 'Speichern' }));
    const input = save.mock.calls[0]![0];
    expect(input).not.toHaveProperty('botToken');
    expect(input).not.toHaveProperty('chats');
    expect(input.notify).toEqual({ ...CONFIG.notify, merged: true });
    expect(input).not.toHaveProperty('profile');
    expect((await screen.findByRole('status')).textContent).toContain('Gespeichert');
  });

  it('schickt ein neu eingetipptes Token mit', async () => {
    const save = vi.fn(async (input: TelegramConfigInput) => ({ ...CONFIG, ...input, chats: CONFIG.chats, profile: CONFIG.profile }) as TelegramConfig);
    open({ saveTelegramConfig: save });
    await userEvent.click(await screen.findByRole('button', { name: 'Ändern' }));
    await userEvent.type(screen.getByLabelText('Neues Bot-Token'), '999:neuesToken');
    await userEvent.click(screen.getByRole('button', { name: 'Speichern' }));
    expect(save.mock.calls[0]![0].botToken).toBe('999:neuesToken');
  });

  it('speichert die Nachtruhe mit Zeiten und schaltet sie wieder ab', async () => {
    const save = vi.fn(async (input: TelegramConfigInput) => ({ ...CONFIG, ...input, chats: CONFIG.chats, profile: CONFIG.profile }) as TelegramConfig);
    open({ saveTelegramConfig: save });
    await userEvent.click(await screen.findByLabelText('Ruhezeit'));
    expect((screen.getByLabelText('von') as HTMLInputElement).value).toBe('22:00');
    await userEvent.clear(screen.getByLabelText('bis'));
    await userEvent.type(screen.getByLabelText('bis'), '06:30');
    await userEvent.click(screen.getByRole('button', { name: 'Speichern' }));
    expect(save.mock.calls[0]![0].quietHours).toEqual({ from: '22:00', to: '06:30' });
  });

  // Ablauf zehn Minuten in der Zukunft, damit der Code im Dialog gültig ist
  const later = () => new Date(Date.now() + 600_000).toISOString();

  it('öffnet beim Koppeln einen Dialog mit Code und QR-Code und öffnet den Link über die Plattform', async () => {
    const link = 'https://t.me/nexus_bot?start=ABC123';
    const pair = vi.fn(async () => ({ code: 'ABC123', expiresAt: later(), link }));
    const platform = fakePlatform();
    open({ pairTelegram: pair }, CONFIG, statusDto(RUNNING), platform);
    await userEvent.click(await screen.findByRole('button', { name: 'Weiteren Chat koppeln' }));
    const dialog = await screen.findByRole('dialog', { name: 'Chat koppeln' });
    const code = await within(dialog).findByRole('group', { name: 'Kopplungscode' });
    expect(code.textContent).toContain('ABC123');
    const qr = within(code).getByRole('img', { name: /QR-Code/ });
    expect(qr.querySelector('path')?.getAttribute('d')).toMatch(/^M\d+ \d+h1v1h-1z/);
    await userEvent.click(within(dialog).getByRole('button', { name: 'Chat mit dem Bot öffnen' }));
    expect(platform.openExternal).toHaveBeenCalledWith(link);
  });

  it('meldet im Dialog, wenn sich der Link zum Koppeln nicht öffnen lässt', async () => {
    const pair = vi.fn(async () => ({ code: 'ABC123', expiresAt: later(), link: 'https://t.me/nexus_bot?start=ABC123' }));
    const platform = fakePlatform({ openExternal: vi.fn(async () => { throw new Error('Die Adresse ließ sich nicht öffnen.'); }) });
    open({ pairTelegram: pair }, CONFIG, statusDto(RUNNING), platform);
    await userEvent.click(await screen.findByRole('button', { name: 'Weiteren Chat koppeln' }));
    await userEvent.click(await screen.findByRole('button', { name: 'Chat mit dem Bot öffnen' }));
    const dialog = screen.getByRole('dialog', { name: 'Chat koppeln' });
    expect((await within(dialog).findByRole('alert')).textContent).toContain('ließ sich nicht öffnen');
  });

  it('zeigt ohne Link weder Knopf noch QR-Code', async () => {
    const pair = vi.fn(async () => ({ code: 'ABC123', expiresAt: later(), link: '' }));
    open({ pairTelegram: pair });
    await userEvent.click(await screen.findByRole('button', { name: 'Weiteren Chat koppeln' }));
    const code = await screen.findByRole('group', { name: 'Kopplungscode' });
    expect(within(code).queryByRole('img')).toBeNull();
    expect(within(code).queryByRole('button')).toBeNull();
  });

  it('schließt den Dialog mit Abbrechen', async () => {
    const pair = vi.fn(async () => ({ code: 'ABC123', expiresAt: later(), link: 'https://t.me/nexus_bot?start=ABC123' }));
    open({ pairTelegram: pair });
    await userEvent.click(await screen.findByRole('button', { name: 'Weiteren Chat koppeln' }));
    const dialog = await screen.findByRole('dialog', { name: 'Chat koppeln' });
    await userEvent.click(within(dialog).getByRole('button', { name: 'Abbrechen' }));
    expect(screen.queryByRole('dialog')).toBeNull();
  });

  it('schließt den Dialog von selbst, sobald ein Chat gekoppelt ist', async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    try {
      const pair = vi.fn(async () => ({ code: 'ABC123', expiresAt: later(), link: 'https://t.me/nexus_bot?start=ABC123' }));
      let config = CONFIG;
      open({ pairTelegram: pair, getTelegramConfig: async () => config });
      const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
      await user.click(await screen.findByRole('button', { name: 'Weiteren Chat koppeln' }));
      await screen.findByRole('group', { name: 'Kopplungscode' });
      await vi.advanceTimersByTimeAsync(PAIR_POLL_MS);
      expect(screen.getByRole('dialog', { name: 'Chat koppeln' })).toBeTruthy();
      config = { ...CONFIG, chats: [...CONFIG.chats, 999] };
      await vi.advanceTimersByTimeAsync(PAIR_POLL_MS);
      expect(screen.queryByRole('dialog')).toBeNull();
      expect(screen.getByText('999')).toBeTruthy();
      expect(screen.getByRole('status').textContent).toContain('Chat 999 gekoppelt');
    } finally {
      vi.useRealTimers();
    }
  });

  it('entkoppelt einen Chat und lädt die Liste neu', async () => {
    const unpairTelegramChat = vi.fn(async () => ({ ok: true }));
    let config = CONFIG;
    open({ unpairTelegramChat, getTelegramConfig: async () => config });
    await screen.findByText('4711');
    config = { ...CONFIG, chats: [815] };
    await userEvent.click(screen.getByRole('button', { name: 'Chat 4711 entkoppeln' }));
    expect(unpairTelegramChat).toHaveBeenCalledWith(4711);
    expect(await screen.findByText('815')).toBeTruthy();
    expect(screen.queryByText('4711')).toBeNull();
  });

  it('zeigt, dass der Bot läuft', async () => {
    open();
    const head = await screen.findByRole('note', { name: 'Stand des Bots' });
    expect(head.textContent).toContain('@nexus_bot');
    expect(head.textContent).toContain('Läuft');
    expect(head.textContent).toContain('2 Chats gekoppelt · 4 von 6 Meldungen aktiv · Keine Ruhezeit');
    expect(screen.queryByRole('note', { name: 'Hinweis zum Bot' })).toBeNull();
  });

  it('zeigt einen Bot auf einem anderen Rechner', async () => {
    open({}, CONFIG, statusDto({ state: 'conflict' }));
    expect((await screen.findByRole('note', { name: 'Hinweis zum Bot' })).textContent).toContain('anderen Rechner');
  });

  it('zählt ungespeicherte Änderungen und verwirft sie', async () => {
    open();
    const save = await screen.findByRole('button', { name: 'Speichern' }) as HTMLButtonElement;
    expect(save.disabled).toBe(true);
    await userEvent.click(screen.getByLabelText('Gemergt'));
    await userEvent.click(screen.getByLabelText('Pause'));
    expect(screen.getByText('2 ungespeicherte Änderungen')).toBeTruthy();
    expect(save.disabled).toBe(false);
    await userEvent.click(screen.getByRole('button', { name: 'Verwerfen' }));
    expect((screen.getByLabelText('Gemergt') as HTMLInputElement).checked).toBe(false);
    expect(save.disabled).toBe(true);
  });

  it('behält die Zeiten der Ruhezeit beim Aus- und Einschalten', async () => {
    open();
    await userEvent.click(await screen.findByLabelText('Ruhezeit'));
    await userEvent.clear(screen.getByLabelText('von'));
    await userEvent.type(screen.getByLabelText('von'), '21:15');
    await userEvent.click(screen.getByLabelText('Ruhezeit'));
    expect((screen.getByLabelText('von') as HTMLInputElement).disabled).toBe(true);
    await userEvent.click(screen.getByLabelText('Ruhezeit'));
    expect((screen.getByLabelText('von') as HTMLInputElement).value).toBe('21:15');
  });

  it('zeigt ein ungültiges Token', async () => {
    open({}, CONFIG, statusDto({ state: 'tokenInvalid', botName: null, botId: null }));
    expect((await screen.findByRole('note', { name: 'Stand des Bots' })).textContent).toContain('Token ungültig');
  });

  it('zeigt einen Profilfehler als eigene Meldung', async () => {
    open({}, CONFIG, statusDto({ state: 'running', profileError: 'Das Profilbild wurde abgelehnt' }));
    const notes = await screen.findAllByRole('note');
    expect(notes.map((n) => n.textContent).join(' ')).toContain('Das Profilbild wurde abgelehnt');
  });

  it('bietet keine Schalter für das Profil des Bots an', async () => {
    open();
    await screen.findByAltText('Profilbild des Bots: Nexus-Logo');
    expect(screen.queryByRole('button', { name: 'Profil neu setzen' })).toBeNull();
    expect(screen.queryByLabelText('Profil des Bots von Nexus einrichten')).toBeNull();
  });

  it('zeigt die Vorschau des Bot-Profils', async () => {
    open();
    const image = await screen.findByAltText('Profilbild des Bots: Nexus-Logo');
    expect(image.getAttribute('src')).toBe('/favicon.svg');
  });

  it('schickt die Testnachricht und zeigt Erfolge und Fehler', async () => {
    const testTelegram = vi.fn(async () => ({ sent: 1, errors: [{ chatId: 815, error: 'chat not found' }] }));
    open({ testTelegram });
    await userEvent.click(await screen.findByRole('button', { name: 'Testnachricht senden' }));
    const notice = await screen.findByRole('note', { name: 'Ergebnis der Testnachricht' });
    expect(notice.textContent).toContain('An 1 Chat verschickt');
    expect(notice.textContent).toContain('815: chat not found');
  });
});
